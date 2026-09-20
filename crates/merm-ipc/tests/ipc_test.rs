use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::channel;
use std::time::Duration;

use merm_ipc::{
    verify_peer, EditorCommand, IpcServer, JsonRpcRequest, JsonRpcResponse,
};

#[test]
fn test_jsonrpc_parsing() {
    let raw = r#"{"jsonrpc":"2.0","id":1,"method":"cursor_moved","params":{"file":"src/main.rs","line":42,"column":10,"symbol":"ProcessNode"}}"#;
    let req: JsonRpcRequest = serde_json::from_str(raw).expect("Must parse");
    let cmd = req.parse_command();

    match cmd {
        EditorCommand::CursorMoved(params) => {
            assert_eq!(params.file, "src/main.rs");
            assert_eq!(params.line, 42);
            assert_eq!(params.column, 10);
            assert_eq!(params.symbol.as_deref(), Some("ProcessNode"));
        }
        _ => panic!("Expected CursorMoved command"),
    }
}

#[test]
fn test_ipc_server_lifecycle_and_peercred() {
    let temp_dir = std::env::temp_dir().join(format!("merm-test-{}", std::process::id()));
    let socket_path = temp_dir.join("test.sock");

    let (tx, rx) = channel();
    let server = IpcServer::bind(&socket_path, tx).expect("Must bind IPC server");
    assert!(socket_path.exists());

    // Connect as client
    let mut client = UnixStream::connect(&socket_path).expect("Must connect to socket");

    // Verify peer credentials directly
    let creds = verify_peer(&client).expect("Self-connect should pass SO_PEERCRED");
    assert_eq!(creds.uid, unsafe { libc::getuid() });

    // Send ping
    let ping_msg = r#"{"jsonrpc":"2.0","id":"ping-1","method":"ping"}"#;
    writeln!(client, "{}", ping_msg).expect("Must write");
    client.flush().expect("Must flush");

    let mut reader = BufReader::new(client.try_clone().unwrap());
    let mut resp_line = String::new();
    reader.read_line(&mut resp_line).expect("Must read ping response");

    let resp: JsonRpcResponse = serde_json::from_str(resp_line.trim()).expect("Must parse response");
    assert_eq!(resp.id, Some(serde_json::Value::String("ping-1".to_string())));
    assert_eq!(resp.result, Some(serde_json::Value::String("pong".to_string())));

    // Send cursor_moved
    let cursor_msg = r#"{"jsonrpc":"2.0","id":2,"method":"cursor_moved","params":{"file":"test.md","line":10,"column":5,"symbol":null}}"#;
    writeln!(client, "{}", cursor_msg).expect("Must write");
    client.flush().expect("Must flush");

    let received = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("Must receive cursor command on channel");

    match received {
        EditorCommand::CursorMoved(p) => {
            assert_eq!(p.file, "test.md");
            assert_eq!(p.line, 10);
        }
        _ => panic!("Expected CursorMoved"),
    }

    // Drop server, socket must be cleaned up
    drop(server);
    assert!(!socket_path.exists(), "Socket file should be removed on drop");
}

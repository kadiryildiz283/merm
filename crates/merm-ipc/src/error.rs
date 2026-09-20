use thiserror::Error;

#[derive(Error, Debug)]
pub enum IpcError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization/Deserialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Peer credential check failed: peer UID {peer_uid} does not match expected UID {expected_uid}")]
    UnauthorizedPeer { peer_uid: u32, expected_uid: u32 },

    #[error("Failed to retrieve peer credentials from socket: OS error code {0}")]
    CredentialRetrievalFailed(i32),

    #[error("Socket path already exists or cannot be bound: {0}")]
    BindError(String),

    #[error("Invalid JSON-RPC frame: {0}")]
    ProtocolError(String),
}

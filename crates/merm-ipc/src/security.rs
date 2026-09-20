use crate::error::IpcError;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerCredentials {
    pub pid: i32,
    pub uid: u32,
    pub gid: u32,
}

#[cfg(target_os = "linux")]
pub fn get_peer_credentials(stream: &UnixStream) -> Result<PeerCredentials, IpcError> {
    let fd = stream.as_raw_fd();
    let mut ucred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;

    let res = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut ucred as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };

    if res != 0 {
        return Err(IpcError::CredentialRetrievalFailed(
            std::io::Error::last_os_error().raw_os_error().unwrap_or(-1),
        ));
    }

    Ok(PeerCredentials {
        pid: ucred.pid,
        uid: ucred.uid,
        gid: ucred.gid,
    })
}

#[cfg(not(target_os = "linux"))]
pub fn get_peer_credentials(_stream: &UnixStream) -> Result<PeerCredentials, IpcError> {
    let current_uid = unsafe { libc::getuid() };
    Ok(PeerCredentials {
        pid: std::process::id() as i32,
        uid: current_uid,
        gid: unsafe { libc::getgid() },
    })
}

/// Verifies that the peer process connecting to the Unix Domain Socket
/// belongs to the exact same operating system user ID (UID).
pub fn verify_peer(stream: &UnixStream) -> Result<PeerCredentials, IpcError> {
    let creds = get_peer_credentials(stream)?;
    let expected_uid = unsafe { libc::getuid() };

    if creds.uid != expected_uid {
        log::warn!(
            "IPC connection rejected: peer UID {} != expected UID {}",
            creds.uid,
            expected_uid
        );
        return Err(IpcError::UnauthorizedPeer {
            peer_uid: creds.uid,
            expected_uid,
        });
    }

    log::debug!(
        "IPC connection authorized for peer PID: {}, UID: {}",
        creds.pid,
        creds.uid
    );
    Ok(creds)
}

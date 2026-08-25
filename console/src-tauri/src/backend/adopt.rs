//! Reuse a healthy leftover sidecar instead of paying another cold start.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use super::paths;

const PROBE_TIMEOUT: Duration = Duration::from_millis(400);

/// If a previous desktop backend is still serving `/api/version`, adopt it.
pub(crate) fn try_adopt_running_backend() -> Option<AdoptedBackend> {
    let port = paths::read_desktop_port()?;
    if !probe_version(port) {
        return None;
    }
    Some(AdoptedBackend {
        port,
        pid: paths::read_desktop_pid(),
    })
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AdoptedBackend {
    pub port: u16,
    pub pid: Option<u32>,
}

pub(crate) fn probe_version(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = match TcpStream::connect_timeout(&addr, PROBE_TIMEOUT) {
        Ok(stream) => stream,
        Err(_) => return false,
    };
    let _ = stream.set_read_timeout(Some(PROBE_TIMEOUT));
    let _ = stream.set_write_timeout(Some(PROBE_TIMEOUT));
    if stream
        .write_all(b"GET /api/version HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut body = String::new();
    let _ = stream.read_to_string(&mut body);
    body.starts_with("HTTP/1.0 200") || body.starts_with("HTTP/1.1 200")
}

pub(crate) fn terminate_pid(pid: u32) {
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T"])
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn probe_version_accepts_http_200() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = thread::spawn(move || {
            if let Ok((mut socket, _)) = listener.accept() {
                let mut buf = [0_u8; 128];
                let _ = socket.read(&mut buf);
                let _ = socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
            }
        });
        assert!(probe_version(port));
        let _ = server.join();
    }

    #[test]
    fn probe_version_rejects_closed_port() {
        assert!(!probe_version(1));
    }
}

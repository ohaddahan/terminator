//! Bounded local MessagePack requests to an existing Neovim process.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::Value;
use std::{
    io::{self, Read, Write},
    os::{fd::AsRawFd, unix::net::UnixStream},
    path::Path,
    time::{Duration, Instant},
};

pub struct Connection {
    stream: UnixStream,
    deadline: Instant,
    next: u64,
}
impl Connection {
    pub fn connect(path: &Path, timeout: Duration) -> Result<Self> {
        Ok(Self {
            stream: UnixStream::connect(path).context("Connect to Neovim")?,
            deadline: Instant::now() + timeout,
            next: 0,
        })
    }
    pub fn call(&mut self, method: &str, args: Value, limit: usize) -> Result<Value> {
        self.next += 1;
        self.stream
            .set_write_timeout(Some(remaining(self.deadline)?))?;
        self.stream
            .write_all(&rmp_serde::to_vec(&(0, self.next, method, args))?)?;
        let reader = Limited {
            stream: &self.stream,
            deadline: self.deadline,
            remaining: limit,
        };
        let mut decoder = rmp_serde::Deserializer::new(reader);
        decoder.set_max_depth(32);
        let response = Value::deserialize(&mut decoder).context("Read Neovim response")?;
        let parts = response.as_array().context("Invalid Neovim response")?;
        ensure!(
            parts.len() == 4 && parts[0] == 1 && parts[1] == self.next,
            "Unexpected Neovim response identity"
        );
        ensure!(parts[2].is_null(), "Neovim request failed: {}", parts[2]);
        Ok(parts[3].clone())
    }
}
fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "Neovim request deadline exceeded"))
}
struct Limited<'a> {
    stream: &'a UnixStream,
    deadline: Instant,
    remaining: usize,
}
impl Read for Limited<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Neovim response exceeds preview limit",
            ));
        }
        // poll enforces one deadline for the complete response, including slow
        // partial frames. It also permits draining a peer that already closed;
        // updating SO_RCVTIMEO on such a socket can fail with EINVAL on macOS.
        loop {
            let wait = remaining(self.deadline)?
                .as_millis()
                .clamp(1, i32::MAX as u128) as i32;
            let mut descriptor = libc::pollfd {
                fd: self.stream.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let ready = unsafe { libc::poll(&mut descriptor, 1, wait) };
            if ready > 0 {
                break;
            }
            if ready == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Neovim request deadline exceeded",
                ));
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
        let limit = bytes.len().min(self.remaining);
        let read = self.stream.read(&mut bytes[..limit])?;
        self.remaining -= read;
        Ok(read)
    }
}
pub fn timed_out(error: &anyhow::Error) -> bool {
    error.chain().any(|error| {
        error.downcast_ref::<io::Error>().is_some_and(|e| {
            matches!(
                e.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            )
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    #[test]
    fn mismatched_rpc_response_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rpc.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut peer, _) = listener.accept().unwrap();
            let _: Value = rmp_serde::from_read(&mut peer).unwrap();
            peer.write_all(&rmp_serde::to_vec(&(1, 99, Value::Null, true)).unwrap())
                .unwrap();
        });
        let mut rpc = Connection::connect(&path, Duration::from_secs(1)).unwrap();
        let error = rpc
            .call("nvim_get_mode", serde_json::json!([]), 4096)
            .unwrap_err();
        assert!(error.to_string().contains("identity"), "{error:#}");
        server.join().unwrap();
    }

    #[test]
    fn socket_deadline_and_response_size_are_bounded() {
        for overflow in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("rpc.sock");
            let listener = UnixListener::bind(&path).unwrap();
            let server = std::thread::spawn(move || {
                let (mut peer, _) = listener.accept().unwrap();
                let _: Value = rmp_serde::from_read(&mut peer).unwrap();
                if overflow {
                    let _ = peer.write_all(
                        &rmp_serde::to_vec(&(1, 1, Value::Null, "x".repeat(2048))).unwrap(),
                    );
                } else {
                    std::thread::sleep(Duration::from_millis(150));
                }
            });
            let started = Instant::now();
            let mut rpc = Connection::connect(&path, Duration::from_millis(50)).unwrap();
            let error = rpc
                .call("nvim_get_mode", serde_json::json!([]), 128)
                .unwrap_err();
            if overflow {
                assert!(format!("{error:#}").contains("limit"), "{error:#}");
            } else {
                assert!(timed_out(&error), "{error:#}");
            }
            assert!(started.elapsed() < Duration::from_secs(1));
            server.join().unwrap();
        }
    }
}

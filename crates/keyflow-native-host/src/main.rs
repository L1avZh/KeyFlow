//! The native messaging host the browser extension talks to.
//!
//! Chrome/Edge/Firefox launch this binary fresh for each
//! `chrome.runtime.connectNative(...)` call, hand it the extension's ID
//! as an argument, and speak the length-prefixed native-messaging
//! framing over its stdin/stdout for as long as the extension keeps the
//! port open. This process holds no secrets and has no vault access of
//! its own — its only job is to relay each request to the already-
//! running KeyFlow desktop app over the local agent socket (see
//! `keyflow-agent::paths`) and relay the response back. If the desktop
//! app isn't running, every request gets a clear "not running" error
//! rather than the process crashing or hanging.

use std::io::{self, BufRead, BufReader, Write};

use keyflow_agent::native_messaging::{read_message, write_message};
use keyflow_agent::{AgentRequest, AgentResponse};

#[cfg(unix)]
mod conn {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    pub struct AgentConn(UnixStream);

    pub fn connect() -> std::io::Result<AgentConn> {
        let stream = UnixStream::connect(keyflow_agent::paths::socket_path())?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        Ok(AgentConn(stream))
    }

    impl Read for AgentConn {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.0.read(buf)
        }
    }
    impl Write for AgentConn {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.0.flush()
        }
    }
    impl AgentConn {
        pub fn try_clone(&self) -> std::io::Result<Self> {
            Ok(AgentConn(self.0.try_clone()?))
        }
    }
}

#[cfg(windows)]
mod conn {
    use std::fs::{File, OpenOptions};
    use std::io::{Read, Write};

    pub struct AgentConn(File);

    pub fn connect() -> std::io::Result<AgentConn> {
        let file = OpenOptions::new().read(true).write(true).open(keyflow_agent::paths::pipe_name())?;
        Ok(AgentConn(file))
    }

    impl Read for AgentConn {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.0.read(buf)
        }
    }
    impl Write for AgentConn {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.0.flush()
        }
    }
    impl AgentConn {
        pub fn try_clone(&self) -> std::io::Result<Self> {
            Ok(AgentConn(self.0.try_clone()?))
        }
    }
}

use conn::connect;

/// Sends one request to the desktop app over the agent socket/pipe and
/// reads back exactly one line-delimited JSON response. A fresh
/// connection per request keeps this simple and avoids any shared
/// mutable state between requests, at the cost of a reconnect each time
/// — acceptable given autofill requests are infrequent, human-paced
/// events, not a hot path.
fn send_request(request: &AgentRequest) -> AgentResponse {
    match send_request_inner(request) {
        Ok(response) => response,
        Err(e) => AgentResponse::error(format!("couldn't reach the KeyFlow desktop app — is it running? ({e})")),
    }
}

fn send_request_inner(request: &AgentRequest) -> io::Result<AgentResponse> {
    let conn = connect()?;
    let mut writer = conn.try_clone()?;
    let mut reader = BufReader::new(conn);

    let line = serde_json::to_string(request)?;
    writeln!(writer, "{line}")?;
    writer.flush()?;

    let mut response_line = String::new();
    let bytes_read = reader.read_line(&mut response_line)?;
    if bytes_read == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "agent closed the connection without responding"));
    }
    let response: AgentResponse = serde_json::from_str(response_line.trim_end())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(response)
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdin_lock = stdin.lock();
    let mut stdout_lock = stdout.lock();

    while let Ok(Some(request)) = read_message::<_, AgentRequest>(&mut stdin_lock) {
        let response = send_request(&request);

        if write_message(&mut stdout_lock, &response).is_err() {
            break; // browser end of the pipe is gone
        }
    }
}

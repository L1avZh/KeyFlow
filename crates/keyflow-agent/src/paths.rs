//! Where the local agent listens. Both the desktop app and the native
//! messaging host call the *same* functions from this shared crate, so
//! there's no risk of the two independently-maintained path
//! computations drifting apart — a mismatch here would silently break
//! every browser-extension connection.

use std::path::PathBuf;

/// Directory the agent's connection info lives under. Deliberately
/// separate from Tauri's own app-data directory (which holds the vault)
/// — the native host has no reason to know where the vault file is, and
/// keeping these independent means neither side depends on replicating
/// Tauri's platform-specific path-resolution logic.
pub fn agent_dir() -> PathBuf {
    let base = dirs::data_local_dir().expect("could not resolve a local data directory for this OS");
    base.join("keyflow-agent")
}

/// Unix domain socket path (macOS/Linux only — see [`pipe_name`] for
/// Windows).
#[cfg(unix)]
pub fn socket_path() -> PathBuf {
    agent_dir().join("agent.sock")
}

/// Named pipe path (Windows only). Named pipes aren't filesystem paths
/// in the usual sense — this is a fixed, well-known pipe name rather
/// than anything under [`agent_dir`].
#[cfg(windows)]
pub fn pipe_name() -> String {
    r"\\.\pipe\keyflow-agent".to_string()
}

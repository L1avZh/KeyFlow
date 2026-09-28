//! The local agent socket the browser extension's native messaging host
//! (`keyflow-native-host`) connects to. This is what actually answers
//! autofill-matching questions — the native host is just a thin relay
//! over stdio, this is where the real vault lookups happen, using the
//! exact same `keyflow_core` calls the desktop UI's Tauri commands use.
//!
//! # Trust boundary (read before touching this file)
//!
//! This socket authenticates nothing beyond what the OS itself
//! guarantees: only processes running as the same OS user can connect to
//! it (a Unix domain socket in a user-owned directory / a Windows named
//! pipe with default, creator-owned ACLs). That is a *new*, additional
//! local attack surface compared to not having this feature — any other
//! program already running as you could speak this protocol directly,
//! without going through a real browser or the native messaging host at
//! all, and ask for a credential by guessing an id and origin. This is a
//! fundamental limitation of native-messaging-based browser integration
//! generally (every password manager that does this has the same
//! boundary), not something unique to KeyFlow, and it is disclosed in
//! THREAT_MODEL.md rather than left implicit. Two things keep it from
//! being worse than necessary: the vault must actually be unlocked for
//! any credential to come back (same condition under which the secret
//! already sits in this process's memory anyway), and `GetCredential`
//! re-validates the origin match server-side rather than trusting the
//! caller's word for it.

use std::io;

use keyflow_agent::{AgentRequest, AgentResponse, MatchSummary};
use keyflow_core::domain::Origin;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;

/// Our JSON requests/responses are tiny (a UUID, an origin string, a
/// username/password) — 64 KiB is enormously generous headroom while
/// still bounding the read.
const MAX_LINE_BYTES: usize = 64 * 1024;

/// Reads one newline-delimited line, capped at `MAX_LINE_BYTES`.
///
/// `BufReader::lines()`/`read_until` grow their buffer without any limit
/// until they find the delimiter or hit EOF — found by reasoning about
/// what a same-OS-user local process (the disclosed trust boundary for
/// this socket; see the module doc) could do by simply connecting and
/// writing an endless stream of bytes with no newline. Each such
/// connection would accumulate memory forever, and since connections are
/// unbounded too, several of them could exhaust memory — a local denial
/// of service against the whole desktop app, not just this feature.
/// Wrapping the reader in `.take()` for each read means the underlying
/// stream reports EOF once the cap is hit, so `read_until` can't grow
/// past it.
async fn read_line_bounded<R: AsyncBufRead + Unpin>(reader: &mut R) -> io::Result<Option<String>> {
    let mut buf = Vec::new();
    let mut limited = reader.take(MAX_LINE_BYTES as u64);
    let n = limited.read_until(b'\n', &mut buf).await?;
    if n == 0 {
        return Ok(None); // clean EOF, nothing read at all
    }
    if buf.last() != Some(&b'\n') {
        // Either the byte cap was hit before a newline appeared, or the
        // connection closed mid-line. Either way, don't try to parse a
        // partial/oversized buffer as JSON.
        return Err(io::Error::new(io::ErrorKind::InvalidData, "line exceeds maximum size or connection closed mid-line"));
    }
    buf.pop();
    String::from_utf8(buf).map(Some).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

use crate::state::AppState;

fn handle_request(app: &AppHandle, request: AgentRequest) -> AgentResponse {
    match request {
        AgentRequest::Ping => {
            let state = app.state::<AppState>();
            let unlocked = state.vault.lock().unwrap().is_some();
            AgentResponse::Pong { unlocked }
        }
        AgentRequest::FindMatches { origin } => handle_find_matches(app, &origin),
        AgentRequest::GetCredential { id, origin } => handle_get_credential(app, &id, &origin),
    }
}

fn handle_find_matches(app: &AppHandle, origin: &str) -> AgentResponse {
    let page = match Origin::parse(origin) {
        Ok(o) => o,
        Err(e) => return AgentResponse::error(format!("invalid origin: {e}")),
    };
    let state = app.state::<AppState>();
    let guard = state.vault.lock().unwrap();
    let Some(vault) = guard.as_ref() else {
        return AgentResponse::error("vault is locked");
    };
    let items = vault
        .find_autofill_matches(&page)
        .into_iter()
        .map(|(cred, decision)| MatchSummary {
            id: cred.id.to_string(),
            name: cred.name.clone(),
            username: cred.username.clone(),
            decision: crate::dto::decision_label(&decision),
        })
        .collect();
    AgentResponse::Matches { items }
}

fn handle_get_credential(app: &AppHandle, id: &str, origin: &str) -> AgentResponse {
    let Ok(uuid) = Uuid::parse_str(id) else {
        return AgentResponse::error("invalid credential id");
    };
    let page = match Origin::parse(origin) {
        Ok(o) => o,
        Err(e) => return AgentResponse::error(format!("invalid origin: {e}")),
    };
    let state = app.state::<AppState>();
    let mut guard = state.vault.lock().unwrap();
    let Some(vault) = guard.as_mut() else {
        return AgentResponse::error("vault is locked");
    };

    // Re-validate the match ourselves rather than trusting that the
    // extension's own earlier FindMatches call is still accurate — the
    // vault could have changed, or a caller could skip straight to this
    // call without ever going through FindMatches at all.
    let matches_now = vault
        .find_autofill_matches(&page)
        .iter()
        .any(|(cred, _)| cred.id == uuid);
    if !matches_now {
        return AgentResponse::error("this credential is not valid for the current page");
    }

    let Some(mut cred) = vault.credentials().iter().find(|c| c.id == uuid).cloned() else {
        return AgentResponse::error("this credential is not valid for the current page");
    };
    let username = cred.username.clone();
    let password = cred.password.clone();
    cred.touch_used();
    let _ = vault.update_credential(cred);
    AgentResponse::Credential { username, password }
}

async fn serve_connection<S>(app: AppHandle, stream: S)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);

    loop {
        let line = match read_line_bounded(&mut reader).await {
            Ok(Some(l)) => l,
            Ok(None) => return, // client disconnected
            Err(_) => return,   // malformed/oversized line — nothing safe to do but close
        };
        let response = match serde_json::from_str::<AgentRequest>(&line) {
            Ok(request) => handle_request(&app, request),
            Err(e) => AgentResponse::error(format!("malformed request: {e}")),
        };
        let Ok(mut serialized) = serde_json::to_string(&response) else { return };
        serialized.push('\n');
        if writer.write_all(serialized.as_bytes()).await.is_err() {
            return;
        }
    }
}

#[cfg(unix)]
async fn run(app: AppHandle) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use tokio::net::UnixListener;

    let dir = keyflow_agent::paths::agent_dir();
    std::fs::create_dir_all(&dir)?;
    let path = keyflow_agent::paths::socket_path();
    // Remove a stale socket left behind by an unclean shutdown; binding
    // to an existing path otherwise fails with AddrInUse.
    let _ = std::fs::remove_file(&path);

    let listener = UnixListener::bind(&path)?;
    // Belt-and-suspenders on top of the directory already being
    // user-owned: restrict the socket file itself to the owner only.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;

    loop {
        let (stream, _) = listener.accept().await?;
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            serve_connection(app, stream).await;
        });
    }
}

#[cfg(windows)]
async fn run(app: AppHandle) -> io::Result<()> {
    use tokio::net::windows::named_pipe::ServerOptions;

    let pipe_name = keyflow_agent::paths::pipe_name();
    let mut server = ServerOptions::new().first_pipe_instance(true).create(&pipe_name)?;

    loop {
        // A `connect()` failure used to propagate straight out of `run()`
        // via `?`, which — because `spawn()` below only logs a failed
        // `run()` rather than restarting it — permanently killed the
        // *entire* agent socket after a single failed connection
        // attempt, with no retry and no visible indication beyond a
        // stderr line. A failed/aborted connect on one pipe instance is
        // exactly the kind of transient event that shouldn't take down
        // every future browser-extension request for the rest of the
        // app's lifetime. (Static fix — not runtime-verified on Windows
        // in this environment; see ROADMAP.md.)
        if let Err(e) = server.connect().await {
            eprintln!("keyflow agent pipe: connect failed, recreating and retrying: {e}");
            server = ServerOptions::new().create(&pipe_name)?;
            continue;
        }
        let connected = server;
        // Create the next instance before handing this one off, so a
        // second client can queue up while we're serving the first.
        server = ServerOptions::new().create(&pipe_name)?;

        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            serve_connection(app, connected).await;
        });
    }
}

/// Starts the agent listener in the background. Errors are logged, not
/// fatal — the desktop app is fully usable without the browser extension
/// feature working, so a bind failure here (e.g. a stale lock, unusual
/// permissions) shouldn't take down the rest of the app.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run(app).await {
            eprintln!("keyflow agent socket failed to start: {e}");
        }
    });
}

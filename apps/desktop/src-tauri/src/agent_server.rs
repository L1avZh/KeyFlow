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
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;

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
    let mut lines = BufReader::new(reader).lines();

    loop {
        let line = match lines.next_line().await {
            Ok(Some(l)) => l,
            Ok(None) => return, // client disconnected
            Err(_) => return,
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
        server.connect().await?;
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

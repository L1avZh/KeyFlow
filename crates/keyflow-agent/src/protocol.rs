//! The message protocol spoken between the browser extension's native
//! messaging host (`keyflow-native-host`) and the local agent socket
//! inside the running desktop app.
//!
//! This is *not* the same framing as native messaging itself (see
//! [`crate::framing`]) — it's the payload shape carried inside whichever
//! framing a given transport uses.
//!
//! # Security note
//!
//! `GetCredential` returns a plaintext username/password. The desktop
//! app re-validates the origin match for the specific credential
//! requested before releasing it — it never trusts a caller's claim
//! that a given credential is safe for a given origin, even though the
//! extension is expected to have already made that determination once
//! via `FindMatches`. See THREAT_MODEL.md's "local agent socket" entry
//! for the trust boundary this protocol operates within (same OS user —
//! it is not authenticated beyond that).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentRequest {
    /// Cheap liveness/lock-state check; safe to call frequently.
    Ping,
    /// Returns the credentials (without passwords) that may be offered
    /// for autofill on `origin`, using the exact same
    /// `keyflow_core::domain` matching engine the desktop UI's Autofill
    /// Tester uses.
    FindMatches { origin: String },
    /// Returns the username/password for one specific credential,
    /// *only* if it still matches `origin` at the moment of the call.
    GetCredential { id: String, origin: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentResponse {
    Pong { unlocked: bool },
    Matches { items: Vec<MatchSummary> },
    Credential { username: String, password: String },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchSummary {
    pub id: String,
    pub name: String,
    pub username: String,
    /// Human-readable match reason ("exact", "subdomain") — never
    /// "blocked" or "no-match", since those are never returned here.
    pub decision: String,
}

impl AgentResponse {
    pub fn error(message: impl Into<String>) -> Self {
        AgentResponse::Error { message: message.into() }
    }
}

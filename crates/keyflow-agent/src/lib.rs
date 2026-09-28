//! Shared code between the KeyFlow desktop app and its browser-extension
//! native messaging host (`keyflow-native-host`): the request/response
//! protocol, where the local agent socket lives, and native-messaging
//! wire framing. Kept as its own crate (rather than duplicating this in
//! both binaries) specifically so the two sides can never drift apart.

pub mod native_messaging;
pub mod paths;
pub mod protocol;

pub use protocol::{AgentRequest, AgentResponse, MatchSummary};

//! `keyflow-core`: the platform-independent heart of KeyFlow.
//!
//! Everything in this crate is pure Rust with no OS-specific code, no
//! UI, and no networking. It owns the parts of the product where a bug
//! is a security incident, not a cosmetic issue:
//!
//! - [`crypto`] — Argon2id key derivation, AES-256-GCM authenticated
//!   encryption, secure randomness.
//! - [`vault`] — the encrypted on-disk vault format, atomic writes, and
//!   the security-dashboard analyses.
//! - [`credential`] — the credential data model.
//! - [`domain`] — origin/domain matching and phishing-resistant autofill
//!   decisions.
//! - [`generator`] — password/passphrase generation and strength
//!   estimation.
//!
//! `#![forbid(unsafe_code)]` is enforced via the `unsafe_code = "forbid"`
//! lint in Cargo.toml: this crate has no reason to need it, and won't.

pub mod credential;
pub mod crypto;
pub mod domain;
pub mod error;
pub mod generator;
pub mod vault;
mod wordlist;

pub use credential::Credential;
pub use domain::{evaluate_match, MatchDecision, Origin};
pub use error::{KeyflowError, Result};
pub use generator::{estimate_strength, generate_passphrase, generate_password, PassphraseOptions, PasswordOptions};
pub use vault::{ImportPreview, ImportWarning, SecurityOverview, Vault};

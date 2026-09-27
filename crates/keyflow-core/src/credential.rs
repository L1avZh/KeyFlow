//! The credential data model.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;
use zeroize::Zeroize;

use crate::domain::Origin;

/// A single saved login. Cloning is intentionally cheap (needed for the
/// UI layer to list credentials) — the *vault file* is what's encrypted
/// at rest, not each individual clone in memory. Still, `password` and
/// `notes` are wiped when a `Credential` is dropped, so short-lived
/// clones (e.g. one handed to the OS clipboard for a few seconds) don't
/// linger in freed memory any longer than necessary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: Uuid,
    /// Display name shown in the vault UI, e.g. "GitHub".
    pub name: String,
    /// The origin this credential was saved against (used by
    /// [`crate::domain::evaluate_match`]). Stored as a URL string; parse
    /// with [`Origin::parse`] when matching.
    pub url: String,
    pub username: String,
    pub password: String,
    pub notes: String,
    /// Free-form labels for organizing credentials, e.g. "work",
    /// "personal" — supports the work/personal separation and
    /// multiple-accounts-per-site use cases without a rigid schema.
    pub tags: Vec<String>,
    pub favorite: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_used_at: Option<OffsetDateTime>,
}

impl Credential {
    pub fn new(name: impl Into<String>, url: impl Into<String>, username: impl Into<String>, password: impl Into<String>) -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            url: url.into(),
            username: username.into(),
            password: password.into(),
            notes: String::new(),
            tags: Vec::new(),
            favorite: false,
            created_at: now,
            updated_at: now,
            last_used_at: None,
        }
    }

    pub fn origin(&self) -> Option<Origin> {
        Origin::parse(&self.url).ok()
    }

    pub fn touch_used(&mut self) {
        self.last_used_at = Some(OffsetDateTime::now_utc());
    }

    pub fn touch_updated(&mut self) {
        self.updated_at = OffsetDateTime::now_utc();
    }

    /// Age of the password since it was last changed, in days. Used by
    /// the security dashboard to flag old passwords.
    pub fn password_age_days(&self) -> i64 {
        (OffsetDateTime::now_utc() - self.updated_at).whole_days()
    }
}

impl Drop for Credential {
    fn drop(&mut self) {
        self.password.zeroize();
        self.notes.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_credential_has_matching_created_and_updated_timestamps() {
        let cred = Credential::new("GitHub", "https://github.com", "me@example.com", "hunter2");
        assert_eq!(cred.created_at, cred.updated_at);
        assert!(cred.last_used_at.is_none());
    }

    #[test]
    fn touch_used_sets_last_used_at() {
        let mut cred = Credential::new("GitHub", "https://github.com", "me@example.com", "hunter2");
        assert!(cred.last_used_at.is_none());
        cred.touch_used();
        assert!(cred.last_used_at.is_some());
    }

    #[test]
    fn serialization_roundtrip_preserves_fields() {
        let cred = Credential::new("GitHub", "https://github.com", "me@example.com", "hunter2");
        let json = serde_json::to_string(&cred).unwrap();
        let restored: Credential = serde_json::from_str(&json).unwrap();
        assert_eq!(cred.id, restored.id);
        assert_eq!(cred.password, restored.password);
        assert_eq!(cred.created_at, restored.created_at);
    }
}

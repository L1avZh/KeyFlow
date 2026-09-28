//! UniFFI bridge between `keyflow-core` (the same vault/crypto/generator/
//! domain-matching engine the desktop app and browser-extension agent
//! socket use) and the Android app's Kotlin code.
//!
//! This crate intentionally contains **no cryptography, no vault-file
//! parsing, and no domain-matching logic of its own** — every
//! security-sensitive decision is delegated to `keyflow-core`, exactly
//! like `apps/desktop/src-tauri/src/lib.rs` + `dto.rs` already do for the
//! desktop app's Tauri IPC boundary. This module mirrors that DTO pattern
//! (see `dto.rs`) rather than inventing a new one: plain records for data,
//! one opaque object (`MobileVault`) for the stateful, key-holding vault
//! handle, and a structured error enum standing in for `KeyflowError`.
//!
//! `keyflow-core` is 100% synchronous (no tokio, no async) — every method
//! below does blocking file I/O and CPU-bound Argon2id/AES-GCM work on
//! whatever thread calls it. The Kotlin side is responsible for calling
//! these from a background dispatcher (e.g. `Dispatchers.IO`), never from
//! the main/UI thread.

use std::str::FromStr;
use std::sync::Mutex;

use keyflow_core::credential::Credential;
use keyflow_core::domain::{MatchDecision, Origin};
use keyflow_core::error::KeyflowError;
use keyflow_core::generator::{PassphraseOptions, PasswordOptions, StrengthBand};
use keyflow_core::vault::{ImportPreview, ImportWarning, SecurityOverview, Vault};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

uniffi::setup_scaffolding!("keyflow_mobile");

// ---------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------

/// Mirrors `keyflow_core::error::KeyflowError` across the FFI boundary.
///
/// Two variants (`Io`, `Serialization`) wrap external error types UniFFI
/// can't represent directly, so they're flattened to their `Display`
/// string — the same pragmatic tradeoff the desktop app's Tauri layer
/// already makes (`map_err` calls `.to_string()` on every `KeyflowError`).
/// Every other variant keeps its structure, since the UI plausibly wants
/// to branch on e.g. `WeakMasterPassword` vs. `AuthenticationFailed` for
/// different error messages, exactly like the desktop frontend does.
///
/// `InvalidId` and `InvalidTimestamp` have no equivalent in
/// `KeyflowError` — they cover malformed UUID/RFC3339 strings crossing
/// the FFI boundary itself (e.g. a corrupted `CredentialRecord.id` from
/// the Kotlin side), the same class of boundary validation the desktop
/// Tauri commands do ad hoc via `Uuid::from_str(&id).map_err(map_err)`.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum MobileError {
    #[error("incorrect master password or corrupted vault")]
    AuthenticationFailed,
    #[error("vault file is corrupted or has an unrecognized format")]
    CorruptVault,
    #[error("vault file was created by a newer version of KeyFlow (format version {found}, supported up to {supported})")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("vault is locked")]
    VaultLocked,
    #[error("credential not found")]
    CredentialNotFound,
    #[error("invalid domain or URL: {0}")]
    InvalidDomain(String),
    #[error("password generator was given an impossible configuration")]
    InvalidGeneratorConfig,
    #[error("master password must be at least {min_length} characters")]
    WeakMasterPassword { min_length: u32 },
    #[error("i/o error while accessing the vault file: {0}")]
    Io(String),
    #[error("failed to (de)serialize vault data: {0}")]
    Serialization(String),
    #[error("invalid credential id: {0}")]
    InvalidId(String),
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(String),
}

impl From<KeyflowError> for MobileError {
    fn from(e: KeyflowError) -> Self {
        match e {
            KeyflowError::AuthenticationFailed => MobileError::AuthenticationFailed,
            KeyflowError::CorruptVault => MobileError::CorruptVault,
            KeyflowError::UnsupportedVersion { found, supported } => MobileError::UnsupportedVersion { found, supported },
            KeyflowError::VaultLocked => MobileError::VaultLocked,
            KeyflowError::CredentialNotFound => MobileError::CredentialNotFound,
            KeyflowError::InvalidDomain(s) => MobileError::InvalidDomain(s),
            KeyflowError::InvalidGeneratorConfig => MobileError::InvalidGeneratorConfig,
            KeyflowError::WeakMasterPassword { min_length } => MobileError::WeakMasterPassword { min_length: min_length as u32 },
            KeyflowError::Io(e) => MobileError::Io(e.to_string()),
            KeyflowError::Serialization(e) => MobileError::Serialization(e.to_string()),
        }
    }
}

fn parse_id(id: &str) -> Result<Uuid, MobileError> {
    Uuid::from_str(id).map_err(|e| MobileError::InvalidId(e.to_string()))
}

fn parse_timestamp(s: &str) -> Result<OffsetDateTime, MobileError> {
    OffsetDateTime::parse(s, &Rfc3339).map_err(|e| MobileError::InvalidTimestamp(e.to_string()))
}

fn fmt_timestamp(t: OffsetDateTime) -> String {
    t.format(&Rfc3339).unwrap_or_else(|_| t.to_string())
}

// ---------------------------------------------------------------------
// Records (plain data crossing the boundary)
// ---------------------------------------------------------------------

/// A saved login, as presented to Kotlin. Timestamps are RFC 3339
/// strings (matching `apps/desktop/src-tauri/src/dto.rs::CredentialDto`)
/// since UniFFI has no native datetime type.
#[derive(Debug, Clone, uniffi::Record)]
pub struct CredentialRecord {
    pub id: String,
    pub name: String,
    pub url: String,
    pub username: String,
    pub password: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: Option<String>,
    pub password_age_days: i64,
}

impl From<&Credential> for CredentialRecord {
    fn from(c: &Credential) -> Self {
        Self {
            id: c.id.to_string(),
            name: c.name.clone(),
            url: c.url.clone(),
            username: c.username.clone(),
            password: c.password.clone(),
            notes: c.notes.clone(),
            tags: c.tags.clone(),
            favorite: c.favorite,
            created_at: fmt_timestamp(c.created_at),
            updated_at: fmt_timestamp(c.updated_at),
            last_used_at: c.last_used_at.map(fmt_timestamp),
            password_age_days: c.password_age_days(),
        }
    }
}

impl CredentialRecord {
    /// Reconstructs the full `keyflow_core::Credential` this record
    /// represents, for `update_credential` — the only call that needs
    /// every field (including `id`/timestamps) round-tripped back.
    fn into_credential(self) -> Result<Credential, MobileError> {
        Ok(Credential {
            id: parse_id(&self.id)?,
            name: self.name,
            url: self.url,
            username: self.username,
            password: self.password,
            notes: self.notes,
            tags: self.tags,
            favorite: self.favorite,
            created_at: parse_timestamp(&self.created_at)?,
            updated_at: parse_timestamp(&self.updated_at)?,
            last_used_at: self.last_used_at.map(|s| parse_timestamp(&s)).transpose()?,
        })
    }
}

/// Input for creating a brand-new credential (no id/timestamps yet —
/// `Credential::new` assigns those). Also reused as the row shape for
/// `commit_import`, mirroring the desktop app's `CredentialInput`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct NewCredential {
    pub name: String,
    pub url: String,
    pub username: String,
    pub password: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub favorite: bool,
}

impl From<NewCredential> for Credential {
    fn from(input: NewCredential) -> Self {
        let mut cred = Credential::new(input.name, input.url, input.username, input.password);
        cred.notes = input.notes;
        cred.tags = input.tags;
        cred.favorite = input.favorite;
        cred
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SecurityOverviewRecord {
    pub total: u32,
    pub weak: u32,
    pub reused: u32,
    pub old: u32,
    pub duplicates: u32,
    pub missing_password: u32,
}

impl From<SecurityOverview> for SecurityOverviewRecord {
    fn from(o: SecurityOverview) -> Self {
        Self {
            total: o.total as u32,
            weak: o.weak as u32,
            reused: o.reused as u32,
            old: o.old as u32,
            duplicates: o.duplicates as u32,
            missing_password: o.missing_password as u32,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ImportWarningRecord {
    pub row: u32,
    pub message: String,
}

impl From<&ImportWarning> for ImportWarningRecord {
    fn from(w: &ImportWarning) -> Self {
        Self { row: w.row as u32, message: w.message.clone() }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ImportPreviewRecord {
    pub credentials: Vec<NewCredential>,
    pub warnings: Vec<ImportWarningRecord>,
}

impl From<ImportPreview> for ImportPreviewRecord {
    fn from(p: ImportPreview) -> Self {
        Self {
            credentials: p
                .credentials
                .iter()
                .map(|c| NewCredential {
                    name: c.name.clone(),
                    url: c.url.clone(),
                    username: c.username.clone(),
                    password: c.password.clone(),
                    notes: c.notes.clone(),
                    tags: c.tags.clone(),
                    favorite: c.favorite,
                })
                .collect(),
            warnings: p.warnings.iter().map(ImportWarningRecord::from).collect(),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AutofillMatchRecord {
    pub credential: CredentialRecord,
    pub decision: String,
}

fn decision_label(decision: &MatchDecision) -> String {
    match decision {
        MatchDecision::ExactMatch => "exact".to_string(),
        MatchDecision::SubdomainMatch => "subdomain".to_string(),
        MatchDecision::Blocked { reason } => format!("blocked: {reason}"),
        MatchDecision::NoMatch => "no-match".to_string(),
    }
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum StrengthBandRecord {
    VeryWeak,
    Weak,
    Fair,
    Strong,
    VeryStrong,
}

impl From<StrengthBand> for StrengthBandRecord {
    fn from(b: StrengthBand) -> Self {
        match b {
            StrengthBand::VeryWeak => StrengthBandRecord::VeryWeak,
            StrengthBand::Weak => StrengthBandRecord::Weak,
            StrengthBand::Fair => StrengthBandRecord::Fair,
            StrengthBand::Strong => StrengthBandRecord::Strong,
            StrengthBand::VeryStrong => StrengthBandRecord::VeryStrong,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct StrengthEstimateRecord {
    pub entropy_bits: f64,
    pub band: StrengthBandRecord,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PasswordOptionsRecord {
    pub length: u32,
    pub uppercase: bool,
    pub lowercase: bool,
    pub digits: bool,
    pub symbols: bool,
    pub exclude_ambiguous: bool,
}

impl From<PasswordOptionsRecord> for PasswordOptions {
    fn from(o: PasswordOptionsRecord) -> Self {
        Self {
            length: o.length as usize,
            uppercase: o.uppercase,
            lowercase: o.lowercase,
            digits: o.digits,
            symbols: o.symbols,
            exclude_ambiguous: o.exclude_ambiguous,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PassphraseOptionsRecord {
    pub word_count: u32,
    pub separator: String,
    pub capitalize: bool,
    pub include_number: bool,
}

impl From<PassphraseOptionsRecord> for PassphraseOptions {
    fn from(o: PassphraseOptionsRecord) -> Self {
        Self {
            word_count: o.word_count as usize,
            separator: o.separator,
            capitalize: o.capitalize,
            include_number: o.include_number,
        }
    }
}

// ---------------------------------------------------------------------
// Free functions (no vault state needed)
// ---------------------------------------------------------------------

/// The server-side (not just UI-side) minimum master-password length
/// `Vault::create`/`change_master_password` enforce. Exposed so the
/// Kotlin UI's own pre-flight check uses the *same* number instead of a
/// hardcoded duplicate — the desktop app shipped with exactly that kind
/// of drift once (frontend checked `.length`, backend checked
/// `.chars().count()`, both against a separately-hardcoded `10`).
#[uniffi::export]
pub fn min_master_password_length() -> u32 {
    keyflow_core::vault::MIN_MASTER_PASSWORD_LEN as u32
}

#[uniffi::export]
pub fn generate_password(options: PasswordOptionsRecord) -> Result<String, MobileError> {
    Ok(keyflow_core::generator::generate_password(&options.into())?)
}

#[uniffi::export]
pub fn generate_passphrase(options: PassphraseOptionsRecord) -> Result<String, MobileError> {
    Ok(keyflow_core::generator::generate_passphrase(&options.into())?)
}

#[uniffi::export]
pub fn estimate_strength(password: String) -> StrengthEstimateRecord {
    let s = keyflow_core::generator::estimate_strength(&password);
    StrengthEstimateRecord { entropy_bits: s.entropy_bits, band: s.band.into() }
}

/// Whether the file at `path` looks like an existing KeyFlow vault, i.e.
/// whether the Android app should show "Unlock" or "Create vault" on
/// first launch. This does not attempt to decrypt or even fully parse
/// it — it only checks the file exists and is non-empty, matching the
/// coarse existence check the desktop app's own `vault_exists` command
/// does (real corruption/format errors surface later from `unlock`
/// itself, which is the actual security boundary).
#[uniffi::export]
pub fn vault_exists(path: String) -> bool {
    std::fs::metadata(&path).map(|m| m.is_file() && m.len() > 0).unwrap_or(false)
}

// ---------------------------------------------------------------------
// MobileVault: the opaque, key-holding vault handle
// ---------------------------------------------------------------------

/// The Android equivalent of the desktop app's `Mutex<Option<Vault>>`
/// app state — except here, "locked" is represented by simply not
/// holding an instance of this object at all (dropping the last `Arc`
/// to it drops the underlying `Vault`, which zeroizes its key), rather
/// than a separate boolean flag guarding a live-but-empty slot. The
/// Kotlin repository layer is expected to hold at most one instance at
/// a time and null it out on lock/timeout.
///
/// Wrapped in a `Mutex` (not `RwLock`) because every mutating operation
/// (add/update/delete/save/import/change-password) needs `&mut Vault`,
/// and UniFFI object methods only ever receive `&self` — same as the
/// desktop app's own `state.vault.lock().unwrap()` pattern.
#[derive(uniffi::Object)]
pub struct MobileVault {
    inner: Mutex<Vault>,
}

#[uniffi::export]
impl MobileVault {
    #[uniffi::constructor]
    pub fn create(path: String, master_password: String) -> Result<Self, MobileError> {
        let vault = Vault::create(path, &master_password)?;
        Ok(Self { inner: Mutex::new(vault) })
    }

    #[uniffi::constructor]
    pub fn unlock(path: String, master_password: String) -> Result<Self, MobileError> {
        let vault = Vault::unlock(path, &master_password)?;
        Ok(Self { inner: Mutex::new(vault) })
    }

    pub fn save(&self) -> Result<(), MobileError> {
        Ok(self.inner.lock().unwrap().save()?)
    }

    pub fn change_master_password(&self, new_master_password: String) -> Result<(), MobileError> {
        Ok(self.inner.lock().unwrap().change_master_password(&new_master_password)?)
    }

    pub fn list_credentials(&self) -> Vec<CredentialRecord> {
        self.inner.lock().unwrap().credentials().iter().map(CredentialRecord::from).collect()
    }

    /// Returns the new credential's id.
    pub fn add_credential(&self, input: NewCredential) -> Result<String, MobileError> {
        let credential: Credential = input.into();
        let id = credential.id.to_string();
        self.inner.lock().unwrap().add_credential(credential)?;
        Ok(id)
    }

    pub fn update_credential(&self, credential: CredentialRecord) -> Result<(), MobileError> {
        let updated = credential.into_credential()?;
        Ok(self.inner.lock().unwrap().update_credential(updated)?)
    }

    pub fn delete_credential(&self, id: String) -> Result<(), MobileError> {
        let uuid = parse_id(&id)?;
        Ok(self.inner.lock().unwrap().delete_credential(uuid)?)
    }

    /// Marks a credential as just-used (drives the "recently used" list
    /// and, on Android specifically, will also be called after the
    /// Autofill service actually fills a credential into another app).
    pub fn touch_credential_used(&self, id: String) -> Result<(), MobileError> {
        let uuid = parse_id(&id)?;
        let mut guard = self.inner.lock().unwrap();
        let mut cred = guard.credentials().iter().find(|c| c.id == uuid).cloned().ok_or(MobileError::CredentialNotFound)?;
        cred.touch_used();
        guard.update_credential(cred)?;
        Ok(())
    }

    pub fn find_autofill_matches(&self, current_page_url: String) -> Result<Vec<AutofillMatchRecord>, MobileError> {
        let page = Origin::parse(&current_page_url).map_err(MobileError::from)?;
        let guard = self.inner.lock().unwrap();
        Ok(guard
            .find_autofill_matches(&page)
            .into_iter()
            .map(|(cred, decision)| AutofillMatchRecord { credential: CredentialRecord::from(cred), decision: decision_label(&decision) })
            .collect())
    }

    pub fn explain_match(&self, credential_id: String, current_page_url: String) -> Result<String, MobileError> {
        let uuid = parse_id(&credential_id)?;
        let page = Origin::parse(&current_page_url).map_err(MobileError::from)?;
        let guard = self.inner.lock().unwrap();
        let decision = guard.explain_match(uuid, &page).ok_or(MobileError::CredentialNotFound)?;
        Ok(decision_label(&decision))
    }

    pub fn security_overview(&self) -> SecurityOverviewRecord {
        self.inner.lock().unwrap().security_overview().into()
    }

    pub fn weak_credentials(&self) -> Vec<CredentialRecord> {
        self.inner.lock().unwrap().weak_passwords().into_iter().map(CredentialRecord::from).collect()
    }

    pub fn old_credentials(&self, older_than_days: i64) -> Vec<CredentialRecord> {
        self.inner.lock().unwrap().old_passwords(older_than_days).into_iter().map(CredentialRecord::from).collect()
    }

    pub fn reused_credential_groups(&self) -> Vec<Vec<CredentialRecord>> {
        let guard = self.inner.lock().unwrap();
        let mut groups: Vec<Vec<CredentialRecord>> =
            guard.reused_passwords().into_values().map(|g| g.into_iter().map(CredentialRecord::from).collect()).collect();
        groups.sort_by_key(|g| g.first().map(|c| c.name.clone()).unwrap_or_default());
        groups
    }

    pub fn duplicate_credential_groups(&self) -> Vec<Vec<CredentialRecord>> {
        let guard = self.inner.lock().unwrap();
        let mut groups: Vec<Vec<CredentialRecord>> =
            guard.duplicate_credentials().into_values().map(|g| g.into_iter().map(CredentialRecord::from).collect()).collect();
        groups.sort_by_key(|g| g.first().map(|c| c.name.clone()).unwrap_or_default());
        groups
    }

    pub fn export_json_plaintext(&self) -> Result<String, MobileError> {
        Ok(self.inner.lock().unwrap().export_json_plaintext()?)
    }

    pub fn preview_csv_import(&self, csv_text: String) -> Result<ImportPreviewRecord, MobileError> {
        Ok(self.inner.lock().unwrap().preview_csv_import(&csv_text)?.into())
    }

    pub fn commit_import(&self, credentials: Vec<NewCredential>) -> Result<(), MobileError> {
        let credentials: Vec<Credential> = credentials.into_iter().map(Credential::from).collect();
        Ok(self.inner.lock().unwrap().commit_import(credentials)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn vault_path(dir: &tempfile::TempDir) -> String {
        dir.path().join("vault.kf").to_string_lossy().into_owned()
    }

    #[test]
    fn create_then_unlock_round_trips() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        {
            let vault = MobileVault::create(path.clone(), "correct horse battery".into()).unwrap();
            let id = vault
                .add_credential(NewCredential {
                    name: "GitHub".into(),
                    url: "https://github.com".into(),
                    username: "me".into(),
                    password: "hunter2".into(),
                    notes: "".into(),
                    tags: vec![],
                    favorite: false,
                })
                .unwrap();
            assert!(Uuid::from_str(&id).is_ok());
        }
        let vault = MobileVault::unlock(path, "correct horse battery".into()).unwrap();
        let creds = vault.list_credentials();
        assert_eq!(creds.len(), 1);
        assert_eq!(creds[0].name, "GitHub");
        assert_eq!(creds[0].password, "hunter2");
    }

    #[test]
    fn unlock_with_wrong_password_returns_authentication_failed() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        MobileVault::create(path.clone(), "correct horse battery".into()).unwrap();
        let result = MobileVault::unlock(path, "wrong password entirely".into());
        assert!(matches!(result, Err(MobileError::AuthenticationFailed)));
    }

    #[test]
    fn create_rejects_password_below_minimum_and_reports_min_length() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        let result = MobileVault::create(path, "short".into());
        match result {
            Err(MobileError::WeakMasterPassword { min_length }) => assert_eq!(min_length, min_master_password_length()),
            Err(other) => panic!("expected WeakMasterPassword, got {other:?}"),
            Ok(_) => panic!("expected WeakMasterPassword, got Ok"),
        }
    }

    #[test]
    fn update_credential_round_trips_through_string_id_and_timestamps() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        let vault = MobileVault::create(path, "correct horse battery".into()).unwrap();
        vault
            .add_credential(NewCredential {
                name: "Old Name".into(),
                url: "https://example.com".into(),
                username: "me".into(),
                password: "pw".into(),
                notes: "".into(),
                tags: vec![],
                favorite: false,
            })
            .unwrap();
        let mut cred = vault.list_credentials().remove(0);
        cred.name = "New Name".into();
        vault.update_credential(cred).unwrap();
        assert_eq!(vault.list_credentials()[0].name, "New Name");
    }

    #[test]
    fn delete_with_malformed_id_returns_invalid_id_not_a_panic() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        let vault = MobileVault::create(path, "correct horse battery".into()).unwrap();
        let result = vault.delete_credential("not-a-uuid".into());
        assert!(matches!(result, Err(MobileError::InvalidId(_))));
    }

    #[test]
    fn find_autofill_matches_reports_exact_match() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        let vault = MobileVault::create(path, "correct horse battery".into()).unwrap();
        vault
            .add_credential(NewCredential {
                name: "GitHub".into(),
                url: "https://github.com".into(),
                username: "me".into(),
                password: "pw".into(),
                notes: "".into(),
                tags: vec![],
                favorite: false,
            })
            .unwrap();
        let matches = vault.find_autofill_matches("https://github.com/login".into()).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].decision, "exact");
    }

    #[test]
    fn find_autofill_matches_on_unrelated_origin_returns_empty() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        let vault = MobileVault::create(path, "correct horse battery".into()).unwrap();
        vault
            .add_credential(NewCredential {
                name: "GitHub".into(),
                url: "https://github.com".into(),
                username: "me".into(),
                password: "pw".into(),
                notes: "".into(),
                tags: vec![],
                favorite: false,
            })
            .unwrap();
        let matches = vault.find_autofill_matches("https://evil-phishing-site.example".into()).unwrap();
        assert!(matches.is_empty());
    }

    #[test]
    fn generate_password_honors_length() {
        let pw = generate_password(PasswordOptionsRecord {
            length: 24,
            uppercase: true,
            lowercase: true,
            digits: true,
            symbols: false,
            exclude_ambiguous: false,
        })
        .unwrap();
        assert_eq!(pw.chars().count(), 24);
    }

    #[test]
    fn generate_password_rejects_zero_length() {
        let result = generate_password(PasswordOptionsRecord {
            length: 0,
            uppercase: true,
            lowercase: true,
            digits: true,
            symbols: true,
            exclude_ambiguous: false,
        });
        assert!(matches!(result, Err(MobileError::InvalidGeneratorConfig)));
    }

    #[test]
    fn vault_exists_is_false_for_missing_file_and_true_after_create() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        assert!(!vault_exists(path.clone()));
        MobileVault::create(path.clone(), "correct horse battery".into()).unwrap();
        assert!(vault_exists(path));
    }

    #[test]
    fn security_overview_reflects_a_weak_password() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        let vault = MobileVault::create(path, "correct horse battery".into()).unwrap();
        vault
            .add_credential(NewCredential {
                name: "Weak".into(),
                url: "https://example.com".into(),
                username: "me".into(),
                password: "a".into(),
                notes: "".into(),
                tags: vec![],
                favorite: false,
            })
            .unwrap();
        let overview = vault.security_overview();
        assert_eq!(overview.total, 1);
        assert_eq!(overview.weak, 1);
    }
}

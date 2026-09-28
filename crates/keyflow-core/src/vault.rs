//! Encrypted vault: on-disk format, atomic writes, locking, and the
//! security-dashboard analyses (weak/reused/old/duplicate credentials).

use std::collections::HashMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::credential::Credential;
use crate::crypto::{self, KdfParams, SealedBlob, VaultKey, SALT_LEN};
use crate::domain::{evaluate_match, MatchDecision, Origin};
use crate::error::{KeyflowError, Result};
use crate::generator::{estimate_strength, StrengthBand};

pub const CURRENT_FORMAT_VERSION: u32 = 1;
const VERIFIER_PLAINTEXT: &[u8] = b"keyflow-vault-verifier-v1";

/// On-disk representation. Everything here except `format_version`,
/// `kdf`, and `salt` is opaque ciphertext — those three fields must stay
/// unencrypted because they're inputs *to* key derivation, not data
/// protected *by* it.
#[derive(Debug, Serialize, Deserialize)]
struct VaultFile {
    format_version: u32,
    kdf: KdfParams,
    #[serde(with = "salt_b64")]
    salt: [u8; SALT_LEN],
    verifier: SealedBlob,
    body: SealedBlob,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct VaultBody {
    credentials: Vec<Credential>,
}

/// An unlocked vault held in memory. The [`VaultKey`] inside zeroizes
/// itself on drop; call [`Vault::lock`] (or just drop the value) as soon
/// as the user is done, rather than holding it for the lifetime of the
/// application.
pub struct Vault {
    path: PathBuf,
    key: VaultKey,
    salt: [u8; SALT_LEN],
    kdf: KdfParams,
    body: VaultBody,
}

fn format_aad(version: u32) -> Vec<u8> {
    format!("keyflow-vault-v{version}").into_bytes()
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir)?;
    let tmp_path = dir.join(format!(
        ".{}.tmp-{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("vault"),
        Uuid::new_v4()
    ));
    {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(contents)?;
        f.sync_all()?;
    }
    fs::rename(&tmp_path, path)?;
    Ok(())
}

impl Vault {
    /// Creates a brand-new, empty vault protected by `master_password`
    /// and writes it to `path`. Fails if a file already exists at
    /// `path` — callers must not silently overwrite an existing vault.
    pub fn create(path: impl Into<PathBuf>, master_password: &str) -> Result<Self> {
        let path = path.into();
        if path.exists() {
            return Err(KeyflowError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "a vault already exists at this path",
            )));
        }
        let salt = crypto::generate_salt();
        let kdf = KdfParams::default();
        let key = VaultKey::derive(master_password, &salt, kdf)?;
        let vault = Vault {
            path,
            key,
            salt,
            kdf,
            body: VaultBody::default(),
        };
        vault.save()?;
        Ok(vault)
    }

    /// Opens an existing vault file, deriving the key from
    /// `master_password` and the salt/KDF parameters stored (in the
    /// clear) in the file header. Returns
    /// [`KeyflowError::AuthenticationFailed`] for a wrong password,
    /// indistinguishably from a corrupted/tampered file — this is
    /// intentional; see [`crypto::open`].
    pub fn unlock(path: impl Into<PathBuf>, master_password: &str) -> Result<Self> {
        let path = path.into();
        let raw = fs::read(&path)?;
        let file: VaultFile = serde_json::from_slice(&raw).map_err(|_| KeyflowError::CorruptVault)?;
        if file.format_version > CURRENT_FORMAT_VERSION {
            return Err(KeyflowError::UnsupportedVersion {
                found: file.format_version,
                supported: CURRENT_FORMAT_VERSION,
            });
        }
        let key = VaultKey::derive(master_password, &file.salt, file.kdf)?;
        let aad = format_aad(file.format_version);
        // Cheap check first: does this key even open the small verifier
        // blob? Gives fast, correct "wrong password" feedback without
        // paying to decrypt a potentially large body — but the body
        // decrypt below is the actual security boundary, not this.
        let verified = crypto::open(&key, &file.verifier, &aad)?;
        if verified != VERIFIER_PLAINTEXT {
            return Err(KeyflowError::AuthenticationFailed);
        }
        let body_bytes = crypto::open(&key, &file.body, &aad)?;
        let body: VaultBody = serde_json::from_slice(&body_bytes).map_err(|_| KeyflowError::CorruptVault)?;
        Ok(Vault {
            path,
            key,
            salt: file.salt,
            kdf: file.kdf,
            body,
        })
    }

    /// Serializes, encrypts, and atomically writes the vault to disk.
    /// The write is verified by re-opening the freshly written file
    /// before returning — a corrupt write is caught here rather than on
    /// the user's next unlock.
    pub fn save(&self) -> Result<()> {
        let aad = format_aad(CURRENT_FORMAT_VERSION);
        let verifier = crypto::seal(&self.key, VERIFIER_PLAINTEXT, &aad)?;
        let body_bytes = serde_json::to_vec(&self.body)?;
        let body = crypto::seal(&self.key, &body_bytes, &aad)?;
        let file = VaultFile {
            format_version: CURRENT_FORMAT_VERSION,
            kdf: self.kdf,
            salt: self.salt,
            verifier,
            body,
        };
        let serialized = serde_json::to_vec_pretty(&file)?;
        atomic_write(&self.path, &serialized)?;

        // Crash-recovery / corruption self-check: read back what we just
        // wrote and confirm it decrypts before trusting the save.
        let check: VaultFile = serde_json::from_slice(&fs::read(&self.path)?).map_err(|_| KeyflowError::CorruptVault)?;
        crypto::open(&self.key, &check.verifier, &aad)?;
        Ok(())
    }

    /// Re-encrypts the entire vault under a new master password (new
    /// salt, fresh KDF derivation) and saves it. The old password stops
    /// working the moment this returns.
    pub fn change_master_password(&mut self, new_master_password: &str) -> Result<()> {
        let salt = crypto::generate_salt();
        let kdf = KdfParams::default();
        let key = VaultKey::derive(new_master_password, &salt, kdf)?;
        self.salt = salt;
        self.kdf = kdf;
        self.key = key;
        self.save()
    }

    pub fn add_credential(&mut self, credential: Credential) -> Result<()> {
        self.body.credentials.push(credential);
        self.save()
    }

    pub fn update_credential(&mut self, updated: Credential) -> Result<()> {
        let slot = self
            .body
            .credentials
            .iter_mut()
            .find(|c| c.id == updated.id)
            .ok_or(KeyflowError::CredentialNotFound)?;
        *slot = updated;
        slot.touch_updated();
        self.save()
    }

    pub fn delete_credential(&mut self, id: Uuid) -> Result<()> {
        let len_before = self.body.credentials.len();
        self.body.credentials.retain(|c| c.id != id);
        if self.body.credentials.len() == len_before {
            return Err(KeyflowError::CredentialNotFound);
        }
        self.save()
    }

    pub fn credentials(&self) -> &[Credential] {
        &self.body.credentials
    }

    /// Returns credentials that may be offered for autofill on
    /// `current_page`, alongside the [`MatchDecision`] that justified
    /// (or explains blocking) each one. Never returns a `Blocked` or
    /// `NoMatch` credential — callers that want to explain a block to
    /// the user should call [`evaluate_match`] directly.
    pub fn find_autofill_matches(&self, current_page: &Origin) -> Vec<(&Credential, MatchDecision)> {
        self.body
            .credentials
            .iter()
            .filter_map(|c| {
                let saved_origin = c.origin()?;
                let decision = evaluate_match(&saved_origin, current_page);
                decision.should_offer_autofill().then_some((c, decision))
            })
            .collect()
    }

    /// Explains why a credential was or wasn't matched — used by the UI
    /// to show the "KeyFlow blocked autofill" explanation rather than
    /// silently doing nothing.
    pub fn explain_match(&self, credential_id: Uuid, current_page: &Origin) -> Option<MatchDecision> {
        let cred = self.body.credentials.iter().find(|c| c.id == credential_id)?;
        let saved_origin = cred.origin()?;
        Some(evaluate_match(&saved_origin, current_page))
    }

    // ---- Security dashboard ----

    pub fn security_overview(&self) -> SecurityOverview {
        SecurityOverview {
            total: self.body.credentials.len(),
            weak: self.weak_passwords().len(),
            reused: self.reused_passwords().values().map(|v| v.len()).sum(),
            old: self.old_passwords(180).len(),
            duplicates: self.duplicate_credentials().values().map(|v| v.len()).sum(),
            missing_password: self.body.credentials.iter().filter(|c| c.password.is_empty()).count(),
        }
    }

    pub fn weak_passwords(&self) -> Vec<&Credential> {
        self.body
            .credentials
            .iter()
            .filter(|c| !c.password.is_empty())
            .filter(|c| matches!(estimate_strength(&c.password).band, StrengthBand::VeryWeak | StrengthBand::Weak))
            .collect()
    }

    /// Groups credentials that share an identical password. The map key
    /// is intentionally not the password itself (avoid making it easy to
    /// accidentally log/print) — group by a per-call opaque index.
    pub fn reused_passwords(&self) -> HashMap<usize, Vec<&Credential>> {
        let mut by_password: HashMap<&str, Vec<&Credential>> = HashMap::new();
        for c in &self.body.credentials {
            if c.password.is_empty() {
                continue;
            }
            by_password.entry(c.password.as_str()).or_default().push(c);
        }
        by_password
            .into_values()
            .filter(|group| group.len() > 1)
            .enumerate()
            .collect()
    }

    pub fn old_passwords(&self, older_than_days: i64) -> Vec<&Credential> {
        self.body
            .credentials
            .iter()
            .filter(|c| !c.password.is_empty() && c.password_age_days() > older_than_days)
            .collect()
    }

    /// Groups credentials with the same (normalized url, username) pair.
    pub fn duplicate_credentials(&self) -> HashMap<usize, Vec<&Credential>> {
        let mut by_key: HashMap<(String, String), Vec<&Credential>> = HashMap::new();
        for c in &self.body.credentials {
            let key = (c.url.trim().to_ascii_lowercase(), c.username.trim().to_ascii_lowercase());
            by_key.entry(key).or_default().push(c);
        }
        by_key
            .into_values()
            .filter(|group| group.len() > 1)
            .enumerate()
            .collect()
    }

    // ---- Import / export ----

    /// Exports all credentials as plaintext JSON. The returned string
    /// contains unencrypted passwords — callers MUST warn the user
    /// before writing it anywhere and should prefer writing it to a
    /// location the user explicitly chose.
    pub fn export_json_plaintext(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&self.body.credentials)?)
    }

    /// Parses a CSV import (columns: name,url,username,password,notes —
    /// header row required) into a review-able preview without
    /// modifying the vault. Call [`Vault::commit_import`] to actually
    /// add the accepted rows.
    pub fn preview_csv_import(&self, csv_text: &str) -> Result<ImportPreview> {
        let mut reader = csv::ReaderBuilder::new().has_headers(true).from_reader(csv_text.as_bytes());
        let headers = reader.headers().map_err(|_| KeyflowError::CorruptVault)?.clone();
        let col = |name: &str| headers.iter().position(|h| h.eq_ignore_ascii_case(name));
        let (name_i, url_i, user_i, pass_i, notes_i) = (
            col("name"),
            col("url"),
            col("username"),
            col("password"),
            col("notes"),
        );

        let mut accepted = Vec::new();
        let mut warnings = Vec::new();

        for (row_num, record) in reader.records().enumerate() {
            let record = match record {
                Ok(r) => r,
                Err(_) => {
                    warnings.push(ImportWarning {
                        row: row_num + 2, // +1 header, +1 1-indexed
                        message: "malformed CSV row, skipped".into(),
                    });
                    continue;
                }
            };
            let get = |idx: Option<usize>| idx.and_then(|i| record.get(i)).unwrap_or("").trim().to_string();
            let name = get(name_i);
            let url = get(url_i);
            let username = get(user_i);
            let password = get(pass_i);
            let notes = get(notes_i);

            if url.is_empty() {
                warnings.push(ImportWarning {
                    row: row_num + 2,
                    message: format!("\"{name}\" has no URL — autofill matching won't work for it"),
                });
            } else if Origin::parse(&url).is_err() {
                warnings.push(ImportWarning {
                    row: row_num + 2,
                    message: format!("\"{name}\" has an unparseable URL ({url}) and was skipped"),
                });
                continue;
            }

            if password.is_empty() {
                warnings.push(ImportWarning {
                    row: row_num + 2,
                    message: format!("\"{name}\" has no password"),
                });
            } else if matches!(estimate_strength(&password).band, StrengthBand::VeryWeak | StrengthBand::Weak) {
                warnings.push(ImportWarning {
                    row: row_num + 2,
                    message: format!("\"{name}\" has a weak password"),
                });
            }

            let is_duplicate_of_existing = self.body.credentials.iter().any(|existing| {
                existing.url.trim().eq_ignore_ascii_case(&url) && existing.username.trim().eq_ignore_ascii_case(&username)
            });
            if is_duplicate_of_existing {
                warnings.push(ImportWarning {
                    row: row_num + 2,
                    message: format!("\"{name}\" already exists in this vault for the same URL and username"),
                });
            }

            let mut cred = Credential::new(name, url, username, password);
            cred.notes = notes;
            accepted.push(cred);
        }

        Ok(ImportPreview { credentials: accepted, warnings })
    }

    pub fn commit_import(&mut self, credentials: Vec<Credential>) -> Result<()> {
        self.body.credentials.extend(credentials);
        self.save()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SecurityOverview {
    pub total: usize,
    pub weak: usize,
    pub reused: usize,
    pub old: usize,
    pub duplicates: usize,
    pub missing_password: usize,
}

#[derive(Debug, Clone)]
pub struct ImportWarning {
    pub row: usize,
    pub message: String,
}

#[derive(Debug)]
pub struct ImportPreview {
    pub credentials: Vec<Credential>,
    pub warnings: Vec<ImportWarning>,
}

mod salt_b64 {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde::{Deserialize, Deserializer, Serializer};

    use super::SALT_LEN;

    pub fn serialize<S: Serializer>(salt: &[u8; SALT_LEN], s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(salt))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<[u8; SALT_LEN], D::Error> {
        let s = String::deserialize(d)?;
        let bytes = STANDARD.decode(s.as_bytes()).map_err(serde::de::Error::custom)?;
        bytes.try_into().map_err(|_| serde::de::Error::custom("salt has wrong length"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn vault_path(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("vault.keyflow")
    }

    #[test]
    fn create_then_unlock_with_correct_password_succeeds() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        Vault::create(&path, "correct-password").unwrap();
        let vault = Vault::unlock(&path, "correct-password").unwrap();
        assert_eq!(vault.credentials().len(), 0);
    }

    #[test]
    fn unlock_with_wrong_password_fails() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        Vault::create(&path, "correct-password").unwrap();
        let result = Vault::unlock(&path, "wrong-password");
        assert!(matches!(result, Err(KeyflowError::AuthenticationFailed)));
    }

    #[test]
    fn create_refuses_to_overwrite_existing_vault() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        Vault::create(&path, "pw1").unwrap();
        assert!(Vault::create(&path, "pw2").is_err());
    }

    #[test]
    fn credentials_persist_across_lock_and_unlock() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        let mut vault = Vault::create(&path, "pw").unwrap();
        vault
            .add_credential(Credential::new("GitHub", "https://github.com", "me@example.com", "s3cret!"))
            .unwrap();
        drop(vault);

        let reopened = Vault::unlock(&path, "pw").unwrap();
        assert_eq!(reopened.credentials().len(), 1);
        assert_eq!(reopened.credentials()[0].name, "GitHub");
    }

    #[test]
    fn changing_master_password_locks_out_the_old_one() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        let mut vault = Vault::create(&path, "old-pw").unwrap();
        vault.change_master_password("new-pw").unwrap();
        drop(vault);

        assert!(matches!(Vault::unlock(&path, "old-pw"), Err(KeyflowError::AuthenticationFailed)));
        assert!(Vault::unlock(&path, "new-pw").is_ok());
    }

    #[test]
    fn delete_nonexistent_credential_errors_without_corrupting_vault() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        let mut vault = Vault::create(&path, "pw").unwrap();
        assert!(matches!(vault.delete_credential(Uuid::new_v4()), Err(KeyflowError::CredentialNotFound)));
    }

    #[test]
    fn autofill_matching_respects_domain_rules() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        let mut vault = Vault::create(&path, "pw").unwrap();
        vault
            .add_credential(Credential::new("GitHub", "https://github.com", "me", "pw"))
            .unwrap();

        let legit = Origin::parse("https://github.com/login").unwrap();
        assert_eq!(vault.find_autofill_matches(&legit).len(), 1);

        let phishing = Origin::parse("https://github.com.attacker.net").unwrap();
        assert_eq!(vault.find_autofill_matches(&phishing).len(), 0);
    }

    #[test]
    fn security_overview_flags_weak_and_reused_and_duplicate() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        let mut vault = Vault::create(&path, "pw").unwrap();
        vault.add_credential(Credential::new("Site A", "https://a.example", "me", "abc")).unwrap();
        vault.add_credential(Credential::new("Site B", "https://b.example", "me", "abc")).unwrap();
        vault
            .add_credential(Credential::new("Site A dup", "https://a.example", "me", "totally-different-strong-pw-9!"))
            .unwrap();

        let overview = vault.security_overview();
        assert_eq!(overview.total, 3);
        assert!(overview.weak >= 2); // "abc" x2
        assert!(overview.reused >= 2); // shared "abc"
        assert!(overview.duplicates >= 2); // same url+username as Site A
    }

    #[test]
    fn csv_import_preview_flags_weak_missing_and_duplicate_rows() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        let mut vault = Vault::create(&path, "pw").unwrap();
        vault.add_credential(Credential::new("Existing", "https://existing.example", "user", "pw")).unwrap();

        let csv_text = "name,url,username,password,notes\n\
                         Weak,https://weak.example,user,abc,\n\
                         NoUrl,,user,strongpassword123!,\n\
                         Dup,https://existing.example,user,another-pass,\n\
                         Fine,https://fine.example,user,Sup3r-Str0ng-Passw0rd!,\n";

        let preview = vault.preview_csv_import(csv_text).unwrap();
        assert_eq!(preview.credentials.len(), 4);
        assert!(preview.warnings.iter().any(|w| w.message.contains("weak")));
        assert!(preview.warnings.iter().any(|w| w.message.contains("no URL")));
        assert!(preview.warnings.iter().any(|w| w.message.contains("already exists")));

        vault.commit_import(preview.credentials).unwrap();
        assert_eq!(vault.credentials().len(), 5);
    }

    #[test]
    fn export_json_contains_plaintext_password_and_caller_must_be_warned() {
        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        let mut vault = Vault::create(&path, "pw").unwrap();
        vault.add_credential(Credential::new("Site", "https://site.example", "user", "plaintext-pw")).unwrap();
        let json = vault.export_json_plaintext().unwrap();
        assert!(json.contains("plaintext-pw"));
    }

    #[test]
    fn tampered_vault_file_fails_to_unlock() {
        // Corrupt the ciphertext deterministically (decode, flip a bit,
        // re-encode) rather than searching the raw file bytes for a
        // specific literal character to flip — that approach had a
        // real, roughly 1-in-20 chance per run of finding nothing to
        // flip in the randomly-generated base64 content, silently
        // leaving the file untouched and making the test flaky.
        use base64::{engine::general_purpose::STANDARD, Engine as _};

        let dir = tempdir().unwrap();
        let path = vault_path(&dir);
        Vault::create(&path, "pw").unwrap();

        let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let ciphertext_b64 = json["body"]["ciphertext"].as_str().unwrap().to_string();
        let mut ciphertext = STANDARD.decode(&ciphertext_b64).unwrap();
        let last_index = ciphertext.len() - 1;
        ciphertext[last_index] ^= 0xFF;
        json["body"]["ciphertext"] = serde_json::Value::String(STANDARD.encode(ciphertext));
        fs::write(&path, serde_json::to_vec_pretty(&json).unwrap()).unwrap();

        assert!(Vault::unlock(&path, "pw").is_err());
    }
}

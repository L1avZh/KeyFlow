use thiserror::Error;

/// Errors surfaced by keyflow-core.
///
/// Variants intentionally omit any secret material (passwords, derived
/// keys, plaintext credentials). Callers may show `Display` output to
/// end users; it must never leak vault contents.
#[derive(Debug, Error)]
pub enum KeyflowError {
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

    #[error("i/o error while accessing the vault file")]
    Io(#[from] std::io::Error),

    #[error("failed to (de)serialize vault data")]
    Serialization(#[from] serde_json::Error),

    #[error("password generator was given an impossible configuration")]
    InvalidGeneratorConfig,

    #[error("master password must be at least {min_length} characters")]
    WeakMasterPassword { min_length: usize },
}

pub type Result<T> = std::result::Result<T, KeyflowError>;

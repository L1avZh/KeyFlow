//! Optional "quick unlock" convenience: stores the master password in the
//! OS's native secure storage (macOS Keychain, Windows Credential
//! Manager, or the Linux Secret Service, via the cross-platform
//! `keyring` crate) so the user isn't forced to retype it every launch.
//!
//! # Security trade-off (documented, not hidden)
//! This is convenience, not an additional security boundary: whoever can
//! read the OS keychain entry (typically anyone logged into the user's
//! OS account) can unlock the vault without knowing the master password.
//! It is off by default, can be disabled at any time from Settings, and
//! is automatically cleared whenever the master password changes so a
//! stale copy can never linger. See SECURITY.md.
//!
//! A fresh biometric prompt on every unlock (Touch ID / Windows Hello)
//! is tracked in ROADMAP.md — the `keyring` crate does not expose
//! per-access biometric gating, so that requires platform-native code
//! this session did not implement.

use keyring::Entry;
use zeroize::Zeroize;

const SERVICE: &str = "com.keyflow.desktop";
const ACCOUNT: &str = "quick-unlock";

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT).map_err(|e| format!("could not access OS secure storage: {e}"))
}

pub fn is_enabled() -> bool {
    entry().and_then(|e| e.get_password().map_err(|e| e.to_string())).is_ok()
}

pub fn enable(master_password: &str) -> Result<(), String> {
    entry()?.set_password(master_password).map_err(|e| format!("could not save to OS secure storage: {e}"))
}

pub fn disable() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("could not clear OS secure storage: {e}")),
    }
}

/// Retrieves the stored master password, if any. The returned string
/// should be zeroized by the caller as soon as it's used.
pub fn retrieve() -> Option<String> {
    entry().ok()?.get_password().ok()
}

pub fn clear_string(mut s: String) {
    s.zeroize();
}

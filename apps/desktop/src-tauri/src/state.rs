use std::path::PathBuf;
use std::sync::Mutex;

use keyflow_core::vault::Vault;

pub struct AppState {
    pub vault: Mutex<Option<Vault>>,
    pub vault_path: PathBuf,
    /// Incremented on every clipboard-copy so a pending clear timer can
    /// tell whether it's still the most recent copy (avoids wiping a
    /// clipboard value the user has since replaced with something else).
    pub clipboard_epoch: Mutex<u64>,
}

impl AppState {
    pub fn new(vault_path: PathBuf) -> Self {
        Self {
            vault: Mutex::new(None),
            vault_path,
            clipboard_epoch: Mutex::new(0),
        }
    }
}

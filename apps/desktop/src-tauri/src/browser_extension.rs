//! Registers `keyflow-native-host` as a native messaging host for
//! whichever Chromium-based browsers are actually installed, so the
//! KeyFlow browser extension (apps/browser-extension/chrome) can reach it via
//! `chrome.runtime.sendNativeMessage`.
//!
//! This only writes the small JSON manifest (and, on Windows, the
//! registry key pointing at it) that tells the browser "here's an
//! executable you're allowed to launch for this extension ID, talking
//! stdio". It grants no permissions beyond that, and only for the one
//! pinned extension ID this build ships (see
//! `apps/browser-extension/chrome/manifest.json`'s `key` field) — a
//! different, unknown extension claiming the same name would not be in
//! `allowed_origins` and the browser itself would refuse to launch the
//! host for it.

use std::path::{Path, PathBuf};

use serde_json::json;
use tauri::{AppHandle, Manager};

const NATIVE_HOST_NAME: &str = "app.keyflow.native_host";
/// Must match the extension ID that `apps/browser-extension/chrome/manifest.json`'s
/// pinned `key` deterministically produces.
const EXTENSION_ID: &str = "lkljkibkbdbgjmljnhoioceloeiiigmj";

fn manifest_json(native_host_path: &Path) -> serde_json::Value {
    json!({
        "name": NATIVE_HOST_NAME,
        "description": "KeyFlow browser-extension bridge",
        "path": native_host_path,
        "type": "stdio",
        "allowed_origins": [format!("chrome-extension://{EXTENSION_ID}/")],
    })
}

/// Finds the `keyflow-native-host` binary: a bundled resource next to a
/// packaged app, or a sibling of the running executable in dev (cargo
/// places every workspace binary in the same `target/<profile>/`
/// directory).
pub fn resolve_native_host_binary(app: &AppHandle) -> Result<PathBuf, String> {
    let binary_name = if cfg!(windows) { "keyflow-native-host.exe" } else { "keyflow-native-host" };

    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidate = resource_dir.join(binary_name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let sibling = current_exe.parent().ok_or("running executable has no parent directory")?.join(binary_name);
    if sibling.exists() {
        return Ok(sibling);
    }

    Err(format!(
        "couldn't find the {binary_name} binary next to KeyFlow — build it with `cargo build -p keyflow-native-host`"
    ))
}

#[cfg(unix)]
fn candidate_manifest_dirs() -> Vec<(&'static str, PathBuf)> {
    let Some(home) = dirs::home_dir() else { return Vec::new() };

    #[cfg(target_os = "macos")]
    let bases: &[(&str, &str)] = &[
        ("Google Chrome", "Library/Application Support/Google/Chrome"),
        ("Chromium", "Library/Application Support/Chromium"),
        ("Microsoft Edge", "Library/Application Support/Microsoft Edge"),
        ("Brave", "Library/Application Support/BraveSoftware/Brave-Browser"),
    ];
    #[cfg(all(unix, not(target_os = "macos")))]
    let bases: &[(&str, &str)] = &[
        ("Google Chrome", ".config/google-chrome"),
        ("Chromium", ".config/chromium"),
        ("Microsoft Edge", ".config/microsoft-edge"),
        ("Brave", ".config/BraveSoftware/Brave-Browser"),
    ];

    bases
        .iter()
        .filter(|(_, rel)| home.join(rel).is_dir()) // only browsers that have actually been run at least once
        .map(|(label, rel)| (*label, home.join(rel).join("NativeMessagingHosts")))
        .collect()
}

#[cfg(unix)]
pub fn register(native_host_path: &Path) -> Result<Vec<String>, String> {
    let targets = candidate_manifest_dirs();
    if targets.is_empty() {
        return Err("no supported Chromium-based browser (Chrome, Chromium, Edge, Brave) was found on this system".into());
    }
    let manifest = manifest_json(native_host_path);
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;

    let mut registered = Vec::new();
    for (label, dir) in targets {
        std::fs::create_dir_all(&dir).map_err(|e| format!("{label}: {e}"))?;
        let manifest_path = dir.join(format!("{NATIVE_HOST_NAME}.json"));
        std::fs::write(&manifest_path, &manifest_bytes).map_err(|e| format!("{label}: {e}"))?;
        registered.push(label.to_string());
    }
    Ok(registered)
}

#[cfg(unix)]
pub fn unregister() -> Result<(), String> {
    for (_, dir) in candidate_manifest_dirs() {
        let manifest_path = dir.join(format!("{NATIVE_HOST_NAME}.json"));
        if manifest_path.exists() {
            std::fs::remove_file(&manifest_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn registry_roots() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Google Chrome", r"Software\Google\Chrome\NativeMessagingHosts"),
        ("Chromium", r"Software\Chromium\NativeMessagingHosts"),
        ("Microsoft Edge", r"Software\Microsoft\Edge\NativeMessagingHosts"),
        ("Brave", r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts"),
    ]
}

/// On Windows the manifest is one file (in the app's own data directory)
/// that every browser's registry key points at — unlike macOS/Linux
/// there's no per-browser manifest *directory* convention, just a
/// registry value naming an arbitrary path.
#[cfg(windows)]
fn windows_manifest_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("native-messaging");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(format!("{NATIVE_HOST_NAME}.json")))
}

#[cfg(windows)]
pub fn register(app: &AppHandle, native_host_path: &Path) -> Result<Vec<String>, String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let manifest_path = windows_manifest_path(app)?;
    let manifest = manifest_json(native_host_path);
    std::fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let mut registered = Vec::new();
    for (label, subkey) in registry_roots() {
        let full_key = format!(r"{subkey}\{NATIVE_HOST_NAME}");
        let (key, _) = hkcu.create_subkey(&full_key).map_err(|e| format!("{label}: {e}"))?;
        key.set_value("", &manifest_path.to_string_lossy().to_string()).map_err(|e| format!("{label}: {e}"))?;
        registered.push(label.to_string());
    }
    Ok(registered)
}

#[cfg(windows)]
pub fn unregister(app: &AppHandle) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    for (_, subkey) in registry_roots() {
        let full_key = format!(r"{subkey}\{NATIVE_HOST_NAME}");
        let _ = hkcu.delete_subkey(&full_key);
    }
    if let Ok(path) = windows_manifest_path(app) {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

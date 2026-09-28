mod agent_server;
mod browser_extension;
mod dto;
mod quick_unlock;
mod state;

use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use dto::{decision_label, AutofillMatchDto, CredentialDto, CredentialInput, ImportPreviewDto, SecurityOverviewDto};
use keyflow_core::credential::Credential;
use keyflow_core::domain::Origin;
use keyflow_core::generator::{estimate_strength, generate_passphrase, generate_password, PassphraseOptions, PasswordOptions};
use keyflow_core::vault::Vault;
use serde::Serialize;
use state::AppState;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

fn map_err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
fn vault_exists(state: State<AppState>) -> bool {
    state.vault_path.exists()
}

#[tauri::command]
fn is_unlocked(state: State<AppState>) -> bool {
    state.vault.lock().unwrap().is_some()
}

#[tauri::command]
fn quick_unlock_available() -> bool {
    quick_unlock::is_enabled()
}

#[tauri::command]
fn create_vault(master_password: String, state: State<AppState>) -> Result<(), String> {
    let vault = Vault::create(&state.vault_path, &master_password).map_err(map_err)?;
    *state.vault.lock().unwrap() = Some(vault);
    Ok(())
}

#[tauri::command]
fn unlock_vault(master_password: String, state: State<AppState>) -> Result<Vec<CredentialDto>, String> {
    let vault = Vault::unlock(&state.vault_path, &master_password).map_err(map_err)?;
    let creds = vault.credentials().iter().map(CredentialDto::from).collect();
    *state.vault.lock().unwrap() = Some(vault);
    Ok(creds)
}

#[tauri::command]
fn try_quick_unlock(state: State<AppState>) -> Result<Option<Vec<CredentialDto>>, String> {
    let Some(password) = quick_unlock::retrieve() else {
        return Ok(None);
    };
    let result = Vault::unlock(&state.vault_path, &password);
    quick_unlock::clear_string(password);
    match result {
        Ok(vault) => {
            let creds = vault.credentials().iter().map(CredentialDto::from).collect();
            *state.vault.lock().unwrap() = Some(vault);
            Ok(Some(creds))
        }
        Err(e) => Err(map_err(e)),
    }
}

#[tauri::command]
fn lock_vault(state: State<AppState>) -> Result<(), String> {
    *state.vault.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
fn list_credentials(state: State<AppState>) -> Result<Vec<CredentialDto>, String> {
    let guard = state.vault.lock().unwrap();
    let vault = guard.as_ref().ok_or("vault is locked")?;
    Ok(vault.credentials().iter().map(CredentialDto::from).collect())
}

#[tauri::command]
fn add_credential(input: CredentialInput, state: State<AppState>) -> Result<CredentialDto, String> {
    let mut guard = state.vault.lock().unwrap();
    let vault = guard.as_mut().ok_or("vault is locked")?;
    let mut cred = Credential::new(input.name, input.url, input.username, input.password);
    cred.notes = input.notes;
    cred.tags = input.tags;
    cred.favorite = input.favorite;
    let dto = CredentialDto::from(&cred);
    vault.add_credential(cred).map_err(map_err)?;
    Ok(dto)
}

#[tauri::command]
fn update_credential(id: String, input: CredentialInput, state: State<AppState>) -> Result<CredentialDto, String> {
    let uuid = Uuid::from_str(&id).map_err(map_err)?;
    let mut guard = state.vault.lock().unwrap();
    let vault = guard.as_mut().ok_or("vault is locked")?;
    let mut updated = vault
        .credentials()
        .iter()
        .find(|c| c.id == uuid)
        .cloned()
        .ok_or("credential not found")?;
    updated.name = input.name;
    updated.url = input.url;
    updated.username = input.username;
    updated.password = input.password;
    updated.notes = input.notes;
    updated.tags = input.tags;
    updated.favorite = input.favorite;
    let dto = CredentialDto::from(&updated);
    vault.update_credential(updated).map_err(map_err)?;
    Ok(dto)
}

#[tauri::command]
fn delete_credential(id: String, state: State<AppState>) -> Result<(), String> {
    let uuid = Uuid::from_str(&id).map_err(map_err)?;
    let mut guard = state.vault.lock().unwrap();
    let vault = guard.as_mut().ok_or("vault is locked")?;
    vault.delete_credential(uuid).map_err(map_err)
}

#[tauri::command]
fn touch_credential_used(id: String, state: State<AppState>) -> Result<(), String> {
    let uuid = Uuid::from_str(&id).map_err(map_err)?;
    let mut guard = state.vault.lock().unwrap();
    let vault = guard.as_mut().ok_or("vault is locked")?;
    let mut cred = vault
        .credentials()
        .iter()
        .find(|c| c.id == uuid)
        .cloned()
        .ok_or("credential not found")?;
    cred.touch_used();
    vault.update_credential(cred).map_err(map_err)
}

#[tauri::command]
fn find_autofill_matches(url: String, state: State<AppState>) -> Result<Vec<AutofillMatchDto>, String> {
    let page = Origin::parse(&url).map_err(map_err)?;
    let guard = state.vault.lock().unwrap();
    let vault = guard.as_ref().ok_or("vault is locked")?;
    Ok(vault
        .find_autofill_matches(&page)
        .into_iter()
        .map(|(cred, decision)| AutofillMatchDto {
            credential: CredentialDto::from(cred),
            decision: decision_label(&decision),
        })
        .collect())
}

#[tauri::command]
fn explain_match(id: String, url: String, state: State<AppState>) -> Result<String, String> {
    let uuid = Uuid::from_str(&id).map_err(map_err)?;
    let page = Origin::parse(&url).map_err(map_err)?;
    let guard = state.vault.lock().unwrap();
    let vault = guard.as_ref().ok_or("vault is locked")?;
    let decision = vault.explain_match(uuid, &page).ok_or("credential has no valid saved URL")?;
    Ok(decision_label(&decision))
}

#[derive(Debug, Clone, Serialize)]
struct GeneratedSecretDto {
    value: String,
    strength: dto::StrengthEstimateDto,
}

#[tauri::command]
fn generate_password_cmd(opts: PasswordOptions) -> Result<GeneratedSecretDto, String> {
    let password = generate_password(&opts).map_err(map_err)?;
    let strength = estimate_strength(&password);
    Ok(GeneratedSecretDto { value: password, strength: strength.into() })
}

#[tauri::command]
fn generate_passphrase_cmd(opts: PassphraseOptions) -> Result<GeneratedSecretDto, String> {
    let phrase = generate_passphrase(&opts).map_err(map_err)?;
    let strength = estimate_strength(&phrase);
    Ok(GeneratedSecretDto { value: phrase, strength: strength.into() })
}

#[tauri::command]
fn estimate_strength_cmd(password: String) -> dto::StrengthEstimateDto {
    estimate_strength(&password).into()
}

#[tauri::command]
fn security_overview(state: State<AppState>) -> Result<SecurityOverviewDto, String> {
    let guard = state.vault.lock().unwrap();
    let vault = guard.as_ref().ok_or("vault is locked")?;
    Ok(vault.security_overview().into())
}

#[tauri::command]
fn weak_credentials(state: State<AppState>) -> Result<Vec<CredentialDto>, String> {
    let guard = state.vault.lock().unwrap();
    let vault = guard.as_ref().ok_or("vault is locked")?;
    Ok(vault.weak_passwords().into_iter().map(CredentialDto::from).collect())
}

#[tauri::command]
fn old_credentials(state: State<AppState>) -> Result<Vec<CredentialDto>, String> {
    let guard = state.vault.lock().unwrap();
    let vault = guard.as_ref().ok_or("vault is locked")?;
    Ok(vault.old_passwords(180).into_iter().map(CredentialDto::from).collect())
}

#[tauri::command]
fn reused_credential_groups(state: State<AppState>) -> Result<Vec<Vec<CredentialDto>>, String> {
    let guard = state.vault.lock().unwrap();
    let vault = guard.as_ref().ok_or("vault is locked")?;
    let mut groups: Vec<Vec<CredentialDto>> = vault
        .reused_passwords()
        .into_values()
        .map(|g| g.into_iter().map(CredentialDto::from).collect())
        .collect();
    groups.sort_by_key(|g: &Vec<CredentialDto>| g.first().map(|c| c.name.clone()).unwrap_or_default());
    Ok(groups)
}

#[tauri::command]
fn duplicate_credential_groups(state: State<AppState>) -> Result<Vec<Vec<CredentialDto>>, String> {
    let guard = state.vault.lock().unwrap();
    let vault = guard.as_ref().ok_or("vault is locked")?;
    let mut groups: Vec<Vec<CredentialDto>> = vault
        .duplicate_credentials()
        .into_values()
        .map(|g| g.into_iter().map(CredentialDto::from).collect())
        .collect();
    groups.sort_by_key(|g: &Vec<CredentialDto>| g.first().map(|c| c.name.clone()).unwrap_or_default());
    Ok(groups)
}

#[tauri::command]
fn change_master_password(new_password: String, state: State<AppState>) -> Result<(), String> {
    let mut guard = state.vault.lock().unwrap();
    let vault = guard.as_mut().ok_or("vault is locked")?;
    vault.change_master_password(&new_password).map_err(map_err)?;
    if quick_unlock::is_enabled() {
        quick_unlock::enable(&new_password)?;
    }
    Ok(())
}

#[tauri::command]
fn set_quick_unlock(enabled: bool, master_password: Option<String>, state: State<AppState>) -> Result<(), String> {
    if enabled {
        let password = master_password.ok_or("master password required to enable quick unlock")?;
        Vault::unlock(&state.vault_path, &password).map_err(map_err)?;
        quick_unlock::enable(&password)
    } else {
        quick_unlock::disable()
    }
}

#[tauri::command]
fn export_json(state: State<AppState>) -> Result<String, String> {
    let guard = state.vault.lock().unwrap();
    let vault = guard.as_ref().ok_or("vault is locked")?;
    vault.export_json_plaintext().map_err(map_err)
}

/// Opens the native save dialog and writes the plaintext export directly
/// to whatever path the user picks — the dialog and the write both
/// happen in Rust, so the webview never gets to supply a raw filesystem
/// path itself. (An earlier version of this command took `path: String`
/// as an argument from the frontend; that would have let *any* script
/// running in the webview — not just KeyFlow's own code, e.g. a future
/// XSS bug or a compromised frontend dependency — write to an arbitrary
/// path by calling the command directly. Doing the picking here closes
/// that off entirely rather than trying to sanitize/scope a path after
/// the fact.) Returns `false` if the user cancelled the dialog.
#[tauri::command]
fn export_json_via_dialog(app: AppHandle, state: State<AppState>) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt;

    let json = export_json(state)?;
    let Some(file_path) = app
        .dialog()
        .file()
        .set_file_name("keyflow-export.json")
        .add_filter("JSON", &["json"])
        .blocking_save_file()
    else {
        return Ok(false);
    };
    let path = file_path.into_path().map_err(map_err)?;
    fs::write(path, json).map_err(map_err)?;
    Ok(true)
}

/// Opens the native open dialog and reads the chosen CSV file directly,
/// for the same reason `export_json_via_dialog` picks its own path
/// instead of accepting one as an argument. Returns `None` if the user
/// cancelled the dialog.
#[tauri::command]
fn pick_and_read_csv(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let Some(file_path) = app.dialog().file().add_filter("CSV", &["csv"]).blocking_pick_file() else {
        return Ok(None);
    };
    let path = file_path.into_path().map_err(map_err)?;
    Ok(Some(fs::read_to_string(path).map_err(map_err)?))
}

#[tauri::command]
fn preview_csv_import(csv_text: String, state: State<AppState>) -> Result<ImportPreviewDto, String> {
    let guard = state.vault.lock().unwrap();
    let vault = guard.as_ref().ok_or("vault is locked")?;
    vault.preview_csv_import(&csv_text).map(ImportPreviewDto::from).map_err(map_err)
}

#[tauri::command]
fn commit_import(credentials: Vec<CredentialInput>, state: State<AppState>) -> Result<(), String> {
    let mut guard = state.vault.lock().unwrap();
    let vault = guard.as_mut().ok_or("vault is locked")?;
    let creds = credentials
        .into_iter()
        .map(|c| {
            let mut cred = Credential::new(c.name, c.url, c.username, c.password);
            cred.notes = c.notes;
            cred.tags = c.tags;
            cred.favorite = c.favorite;
            cred
        })
        .collect();
    vault.commit_import(creds).map_err(map_err)
}

#[tauri::command]
fn register_browser_extension(app: AppHandle) -> Result<Vec<String>, String> {
    let host_path = browser_extension::resolve_native_host_binary(&app)?;
    #[cfg(unix)]
    {
        browser_extension::register(&host_path)
    }
    #[cfg(windows)]
    {
        browser_extension::register(&app, &host_path)
    }
}

#[tauri::command]
#[allow(unused_variables)] // `app` is only used in the cfg(windows) branch below
fn unregister_browser_extension(app: AppHandle) -> Result<(), String> {
    #[cfg(unix)]
    {
        browser_extension::unregister()
    }
    #[cfg(windows)]
    {
        browser_extension::unregister(&app)
    }
}

/// Writes `text` to the system clipboard and, after `seconds`, clears it
/// again — but only if the clipboard still holds exactly what we put
/// there, so we never stomp on something the user copied afterward.
#[tauri::command]
async fn copy_with_clipboard_timeout(app: AppHandle, state: State<'_, AppState>, text: String, seconds: u64) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    app.clipboard().write_text(text.clone()).map_err(map_err)?;

    let epoch = {
        let mut e = state.clipboard_epoch.lock().unwrap();
        *e += 1;
        *e
    };

    let app_for_timer = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
        let state = app_for_timer.state::<AppState>();
        let still_current = *state.clipboard_epoch.lock().unwrap() == epoch;
        if !still_current {
            return;
        }
        if let Ok(current) = app_for_timer.clipboard().read_text() {
            if current == text {
                let _ = app_for_timer.clipboard().write_text(String::new());
            }
        }
    });

    Ok(())
}

fn register_global_shortcut(app: &AppHandle) {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

    let shortcut = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyK);
    let app_handle = app.clone();
    let _ = app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.emit("quick-access:toggle", ());
            }
        }
    });
}

fn app_data_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().expect("app data dir must be resolvable")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app_data_dir(&handle);
            fs::create_dir_all(&data_dir).ok();
            let vault_path = data_dir.join("vault.keyflow");
            app.manage(AppState::new(vault_path));
            register_global_shortcut(&handle);
            agent_server::spawn(handle.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            vault_exists,
            is_unlocked,
            quick_unlock_available,
            create_vault,
            unlock_vault,
            try_quick_unlock,
            lock_vault,
            list_credentials,
            add_credential,
            update_credential,
            delete_credential,
            touch_credential_used,
            find_autofill_matches,
            explain_match,
            generate_password_cmd,
            generate_passphrase_cmd,
            estimate_strength_cmd,
            security_overview,
            weak_credentials,
            old_credentials,
            reused_credential_groups,
            duplicate_credential_groups,
            change_master_password,
            set_quick_unlock,
            export_json,
            export_json_via_dialog,
            pick_and_read_csv,
            preview_csv_import,
            commit_import,
            copy_with_clipboard_timeout,
            register_browser_extension,
            unregister_browser_extension,
        ])
        .run(tauri::generate_context!())
        .expect("error while running keyflow-desktop");
}

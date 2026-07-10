use std::str::FromStr;

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use tokio::sync::oneshot;

use crate::{keychain, settings_store};

#[derive(Serialize)]
pub struct SettingsView {
    provider: String,
    lmstudio_base_url: String,
    ui_language: String,
    source_lang: String,
    target_lang: String,
    shortcut: String,
    has_api_key: bool,
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<SettingsView, String> {
    let settings = settings_store::load(&app)?;
    let has_api_key = keychain::get_api_key(&settings.provider)?.is_some();
    Ok(SettingsView {
        provider: settings.provider,
        lmstudio_base_url: settings.lmstudio_base_url,
        ui_language: settings.ui_language,
        source_lang: settings.source_lang,
        target_lang: settings.target_lang,
        shortcut: settings.shortcut,
        has_api_key,
    })
}

#[tauri::command]
pub fn save_provider(app: AppHandle, provider: String) -> Result<(), String> {
    let mut settings = settings_store::load(&app)?;
    settings.provider = provider;
    settings_store::save(&app, &settings)
}

#[tauri::command]
pub fn save_lmstudio_base_url(app: AppHandle, url: String) -> Result<(), String> {
    let mut settings = settings_store::load(&app)?;
    settings.lmstudio_base_url = url.trim().to_string();
    settings_store::save(&app, &settings)
}

#[tauri::command]
pub fn save_ui_language(app: AppHandle, language: String) -> Result<(), String> {
    let mut settings = settings_store::load(&app)?;
    settings.ui_language = language;
    settings_store::save(&app, &settings)
}

#[tauri::command]
pub fn save_translation_langs(
    app: AppHandle,
    source_lang: String,
    target_lang: String,
) -> Result<(), String> {
    let mut settings = settings_store::load(&app)?;
    settings.source_lang = source_lang;
    settings.target_lang = target_lang;
    settings_store::save(&app, &settings)
}

/// Unregisters whatever shortcut is currently active and registers the new
/// one, then persists it. Called from Settings when the user records a new
/// key combo — this is what lets someone move off the Cmd+Shift+T default
/// if it collides with something else (e.g. Chrome's reopen-closed-tab).
///
/// macOS's hotkey registration APIs assert the main thread, the same way
/// enigo's keycode lookups do (see commands/translate.rs) — this command
/// isn't `async` on the IPC side conceptually, but Tauri still dispatches
/// command handlers off the main thread, so calling register/unregister
/// directly here hangs the app. Hop onto the main thread and wait for the
/// result over a channel, same fix as the paste flow.
#[tauri::command]
pub async fn save_shortcut(app: AppHandle, shortcut: String) -> Result<(), String> {
    let new_shortcut = Shortcut::from_str(&shortcut).map_err(|e| e.to_string())?;
    let mut settings = settings_store::load(&app)?;
    let old_shortcut = Shortcut::from_str(&settings.shortcut).ok();

    let (tx, rx) = oneshot::channel();
    let app_for_main = app.clone();
    app.run_on_main_thread(move || {
        let result = (|| -> Result<(), String> {
            if let Some(old) = old_shortcut {
                // Best-effort: if it was never actually registered (e.g.
                // stale config), unregister just errors, which we ignore.
                let _ = app_for_main.global_shortcut().unregister(old);
            }
            app_for_main
                .global_shortcut()
                .register(new_shortcut)
                .map_err(|e| e.to_string())
        })();
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;

    rx.await.map_err(|e| e.to_string())??;

    settings.shortcut = shortcut;
    settings_store::save(&app, &settings)
}

#[tauri::command]
pub fn save_api_key(provider: String, key: String) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("La API key no puede estar vacía".to_string());
    }
    keychain::set_api_key(&provider, key)
}

#[tauri::command]
pub fn clear_api_key(provider: String) -> Result<(), String> {
    keychain::clear_api_key(&provider)
}

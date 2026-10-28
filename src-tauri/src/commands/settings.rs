use std::str::FromStr;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use tokio::sync::oneshot;

use crate::{keychain, services, settings_store, tray};

#[derive(Serialize)]
pub struct SettingsView {
    provider: String,
    lmstudio_base_url: String,
    local_model: String,
    ui_language: String,
    source_lang: String,
    target_lang: String,
    shortcut: String,
    tone: String,
    snippet_shortcut: String,
    dictation_shortcut: String,
    dictation_output: String,
    direct_output: String,
    autostart_enabled: bool,
    autostart_command: String,
    stt_base_url: String,
    vocabulary: String,
    has_api_key: bool,
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<SettingsView, String> {
    let settings = settings_store::load(&app)?;
    let has_api_key = keychain::get_api_key(&settings.provider)?.is_some();
    Ok(SettingsView {
        provider: settings.provider,
        lmstudio_base_url: settings.lmstudio_base_url,
        local_model: settings.local_model,
        ui_language: settings.ui_language,
        source_lang: settings.source_lang,
        target_lang: settings.target_lang,
        shortcut: settings.shortcut,
        tone: settings.tone,
        snippet_shortcut: settings.snippet_shortcut,
        dictation_shortcut: settings.dictation_shortcut,
        // An unknown stored value pastes the translation (see `run()`), so
        // that is what the UI shows too.
        dictation_output: settings_store::parse_dictation_output(&settings.dictation_output)
            .unwrap_or("translate")
            .to_string(),
        direct_output: settings.direct_output,
        autostart_enabled: settings.autostart_enabled,
        autostart_command: settings.autostart_command,
        stt_base_url: settings.stt_base_url,
        vocabulary: settings.vocabulary,
        has_api_key,
    })
}

#[tauri::command]
pub fn save_provider(app: AppHandle, provider: String) -> Result<(), String> {
    settings_store::update(&app, |s| {
        s.provider = provider;
        Ok(())
    })
    .map(drop)
}

#[tauri::command]
pub fn save_lmstudio_base_url(app: AppHandle, url: String) -> Result<(), String> {
    settings_store::update(&app, |s| {
        s.lmstudio_base_url = url.trim().to_string();
        Ok(())
    })
    .map(drop)
}

#[tauri::command]
pub fn save_stt_base_url(app: AppHandle, url: String) -> Result<(), String> {
    settings_store::update(&app, |s| {
        s.stt_base_url = url.trim().to_string();
        Ok(())
    })
    .map(drop)
}

#[tauri::command]
pub fn save_vocabulary(app: AppHandle, vocabulary: String) -> Result<(), String> {
    settings_store::update(&app, |s| {
        s.vocabulary = vocabulary.trim().to_string();
        Ok(())
    })
    .map(drop)
}

#[tauri::command]
pub fn save_local_model(app: AppHandle, model: String) -> Result<(), String> {
    settings_store::update(&app, |s| {
        s.local_model = model.trim().to_string();
        Ok(())
    })
    .map(drop)
}

#[tauri::command]
pub fn save_ui_language(app: AppHandle, language: String) -> Result<(), String> {
    settings_store::update(&app, |s| {
        s.ui_language = language.clone();
        Ok(())
    })?;
    tray::update_language(&app, &language);
    Ok(())
}

#[tauri::command]
pub fn save_tone(app: AppHandle, tone: String) -> Result<(), String> {
    settings_store::update(&app, |s| {
        s.tone = tone.clone();
        Ok(())
    })?;
    tray::update_tone(&app, &tone);
    Ok(())
}

#[tauri::command]
pub fn save_translation_langs(
    app: AppHandle,
    source_lang: String,
    target_lang: String,
) -> Result<(), String> {
    settings_store::update(&app, |s| {
        s.source_lang = source_lang;
        s.target_lang = target_lang;
        Ok(())
    })
    .map(drop)
}

/// Each global shortcut must be unique: one combo can't trigger two actions.
fn ensure_not_taken(new: &Shortcut, others: [&str; 2]) -> Result<(), String> {
    if others.iter().any(|o| Shortcut::from_str(o).ok().as_ref() == Some(new)) {
        return Err("Ese atajo ya está en uso por otra acción de TypeLang".to_string());
    }
    Ok(())
}

/// Unregisters whatever shortcut is currently active and registers the new
/// one, then persists it. Called from Settings when the user records a new
/// key combo — this is what lets someone move off the Cmd+Shift+T default
/// if it collides with something else (e.g. Chrome's reopen-closed-tab).
///
/// macOS's hotkey registration APIs assert the main thread, the same way
/// the paste keystroke posting does (see commands/translate.rs) — this command
/// isn't `async` on the IPC side conceptually, but Tauri still dispatches
/// command handlers off the main thread, so calling register/unregister
/// directly here hangs the app. Hop onto the main thread and wait for the
/// result over a channel, same fix as the paste flow.
#[tauri::command]
pub async fn save_shortcut(app: AppHandle, shortcut: String) -> Result<(), String> {
    let new_shortcut = Shortcut::from_str(&shortcut).map_err(|e| e.to_string())?;
    let settings = settings_store::load(&app)?;
    ensure_not_taken(&new_shortcut, [&settings.snippet_shortcut, &settings.dictation_shortcut])?;
    let old_shortcut = Shortcut::from_str(&settings.shortcut).ok();

    let (tx, rx) = oneshot::channel();
    let app_for_main = app.clone();
    app.run_on_main_thread(move || {
        let result: Result<(), String> = {
            if let Some(old) = old_shortcut {
                // Best-effort: if it was never actually registered (e.g.
                // stale config), unregister just errors, which we ignore.
                let _ = app_for_main.global_shortcut().unregister(old);
            }
            app_for_main
                .global_shortcut()
                .register(new_shortcut)
                .map_err(|e| e.to_string())
        };
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;

    rx.await.map_err(|e| e.to_string())??;

    settings_store::update(&app, |s| {
        s.shortcut = shortcut;
        Ok(())
    })
    .map(drop)
}

/// Same unregister-then-register-then-persist flow as `save_shortcut`,
/// scoped to the snippets shortcut instead — the two are fully independent
/// (changing one never touches the other's registration).
#[tauri::command]
pub async fn save_snippet_shortcut(app: AppHandle, shortcut: String) -> Result<(), String> {
    let new_shortcut = Shortcut::from_str(&shortcut).map_err(|e| e.to_string())?;
    let settings = settings_store::load(&app)?;
    ensure_not_taken(&new_shortcut, [&settings.shortcut, &settings.dictation_shortcut])?;
    let old_shortcut = Shortcut::from_str(&settings.snippet_shortcut).ok();

    let (tx, rx) = oneshot::channel();
    let app_for_main = app.clone();
    app.run_on_main_thread(move || {
        let result: Result<(), String> = {
            if let Some(old) = old_shortcut {
                let _ = app_for_main.global_shortcut().unregister(old);
            }
            app_for_main
                .global_shortcut()
                .register(new_shortcut)
                .map_err(|e| e.to_string())
        };
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;

    rx.await.map_err(|e| e.to_string())??;

    settings_store::update(&app, |s| {
        s.snippet_shortcut = shortcut;
        Ok(())
    })
    .map(drop)
}

/// Same flow again for the hold-to-talk dictation shortcut. The old combo may
/// never have been registered (startup registration is best-effort), which
/// the ignored unregister error covers.
#[tauri::command]
pub async fn save_dictation_shortcut(app: AppHandle, shortcut: String) -> Result<(), String> {
    let new_shortcut = Shortcut::from_str(&shortcut).map_err(|e| e.to_string())?;
    let settings = settings_store::load(&app)?;
    ensure_not_taken(&new_shortcut, [&settings.shortcut, &settings.snippet_shortcut])?;
    let old_shortcut = Shortcut::from_str(&settings.dictation_shortcut).ok();

    let (tx, rx) = oneshot::channel();
    let app_for_main = app.clone();
    app.run_on_main_thread(move || {
        let result: Result<(), String> = {
            if let Some(old) = old_shortcut {
                let _ = app_for_main.global_shortcut().unregister(old);
            }
            app_for_main
                .global_shortcut()
                .register(new_shortcut)
                .map_err(|e| e.to_string())
        };
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;

    rx.await.map_err(|e| e.to_string())??;

    settings_store::update(&app, |s| {
        s.dictation_shortcut = shortcut;
        Ok(())
    })
    .map(drop)
}

/// Tells every window (Settings, the recording pill) the dictation output
/// changed, so whichever one did not make the change can follow it.
pub fn emit_output_changed(app: &AppHandle, output: &str) {
    if let Err(e) = app.emit("dictation-output-changed", serde_json::json!({ "output": output })) {
        eprintln!("[settings] could not announce the dictation output: {e}");
    }
}

/// "translate", "improve" or "original": what the hold-to-talk dictation
/// pastes.
#[tauri::command]
pub fn save_dictation_output(app: AppHandle, output: String) -> Result<(), String> {
    let output = settings_store::parse_dictation_output(&output)
        .ok_or_else(|| format!("Valor de salida inválido: {output}"))?;
    let saved = settings_store::update(&app, |s| {
        s.dictation_output = output.to_string();
        Ok(())
    })?;
    emit_output_changed(&app, &saved.dictation_output);
    Ok(())
}

/// "improve" or "original": what the dictation pastes when it is not
/// translating. Chosen here only, never from the pill. While the dictation is
/// in that direct mode its effective output follows the choice.
#[tauri::command]
pub fn save_direct_output(app: AppHandle, output: String) -> Result<(), String> {
    let saved = settings_store::update(&app, |s| settings_store::set_direct_output(s, &output))?;
    emit_output_changed(&app, &saved.dictation_output);
    Ok(())
}

/// Persists the autostart toggle + command, then applies it right away:
/// ON starts the services (after the same already-running check as at
/// launch), OFF stops them if TypeLang started them.
#[tauri::command]
pub fn save_autostart(app: AppHandle, enabled: bool, command: String) -> Result<(), String> {
    let command = command.trim().to_string();
    if enabled && command.is_empty() {
        return Err("El comando no puede estar vacío".to_string());
    }
    settings_store::update(&app, |s| {
        s.autostart_enabled = enabled;
        s.autostart_command = command;
        Ok(())
    })?;
    if enabled {
        services::start_if_configured(&app);
    } else {
        services::stop(&app);
    }
    Ok(())
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

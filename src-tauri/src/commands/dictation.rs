use tauri::{AppHandle, Manager, State, WebviewWindow};

use crate::{
    audio,
    dictation_hotkey::{self, PillPayload},
    settings_store,
    state::AppState,
    stt, window,
};

#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // A blur handled before this command arrived already ran its cancel, so
    // without this check the mic would open behind a hidden popup.
    let focused = app
        .get_webview_window(window::POPUP_LABEL)
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false);
    if !focused {
        return Err("Grabación cancelada".to_string());
    }
    audio::start(&state).await
}

/// Stops the recording and returns its transcript. An empty string means
/// nothing worth keeping (too short, or no speech) — the popup appends nothing.
#[tauri::command]
pub async fn stop_and_transcribe(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let wav = audio::stop(&state).await?;
    if wav.is_empty() {
        return Ok(String::new());
    }
    let settings = settings_store::load(&app)?;
    stt::transcribe(
        &settings.stt_base_url,
        wav,
        &settings.source_lang,
        &settings.vocabulary_terms(),
    )
    .await
}

#[tauri::command]
pub fn cancel_recording(state: State<'_, AppState>) {
    audio::cancel(&state);
}

/// The pill's commands steer the dictation shortcut's session; no other
/// window may call them.
fn require_pill(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == window::PILL_LABEL {
        Ok(())
    } else {
        Err("not allowed from this window".to_string())
    }
}

/// The pill's check mark: stop the recording, transcribe and paste.
#[tauri::command]
pub fn dictation_pill_accept(window: WebviewWindow) -> Result<(), String> {
    require_pill(&window)?;
    dictation_hotkey::pill_accept(window.app_handle());
    Ok(())
}

/// The pill's cross: discard the recording, or abandon the running job.
#[tauri::command]
pub fn dictation_pill_cancel(window: WebviewWindow) -> Result<(), String> {
    require_pill(&window)?;
    dictation_hotkey::pill_cancel(window.app_handle());
    Ok(())
}

/// What the pill shows right now. The pill asks on mount, in case the
/// `pill-state` event that matters fired before its listener was ready.
#[tauri::command]
pub fn dictation_pill_state(window: WebviewWindow) -> Result<Option<PillPayload>, String> {
    require_pill(&window)?;
    Ok(dictation_hotkey::pill_state(window.app_handle()))
}

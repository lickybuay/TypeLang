use tauri::{AppHandle, Manager, State};

use crate::{audio, settings_store, state::AppState, stt, window};

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

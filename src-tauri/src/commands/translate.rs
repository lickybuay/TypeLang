use tauri::{AppHandle, State};
use tokio::sync::oneshot;

use crate::{focus, keychain, llm, paste, settings_store, state::AppState, window};

/// Called by the popup on submit. Translates the input, then hides the
/// popup, restores focus to the originally-captured app, and pastes.
#[tauri::command]
pub async fn translate_and_paste(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
    tone: String,
) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("empty input".to_string());
    }

    let settings = settings_store::load(&app)?;
    let api_key = keychain::get_api_key(&settings.provider)?;
    let cfg = llm::ProviderConfig {
        provider: settings.provider,
        api_key,
        base_url: Some(settings.lmstudio_base_url),
        model: Some(settings.local_model),
    };
    // `tone` comes from the popup, not `settings` — it starts as the saved
    // default but Tab lets the user flip it for just this one message
    // without touching Settings (see Popup.tsx).
    // Providers (Claude especially) sometimes wrap the answer in a leading/
    // trailing newline despite the prompt saying "output only the text" —
    // left as-is that pastes as a blank line above the translation.
    let translated = llm::translate(&cfg, text, &settings.source_lang, &settings.target_lang, &tone)
        .await?
        .trim()
        .to_string();

    let captured = state
        .captured_focus
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "No se capturó la app original".to_string())?;

    // `enigo`'s macOS keycode lookups call into TSM/AppKit, which asserts
    // it's running on the main thread. Async commands run on a tokio worker
    // thread, so calling paste::paste_text directly here crashes the whole
    // app (dispatch_assert_queue_fail). Hop the hide/restore/paste sequence
    // onto the main thread and bring the result back over a channel.
    let (tx, rx) = oneshot::channel();
    let app_for_main = app.clone();
    app.run_on_main_thread(move || {
        let result = (|| -> Result<(), String> {
            window::hide_popup(&app_for_main);
            focus::restore(&captured)?;
            paste::paste_text(&translated)?;
            Ok(())
        })();
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;

    rx.await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn cancel_popup(app: AppHandle) {
    window::hide_popup(&app);
}

#[tauri::command]
pub fn open_settings_from_popup(app: AppHandle) {
    window::hide_popup(&app);
    window::show_settings(&app);
}

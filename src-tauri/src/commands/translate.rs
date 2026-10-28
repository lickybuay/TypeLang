use tauri::{AppHandle, Manager, State};
use tokio::sync::oneshot;

use crate::{
    focus, keychain, llm, paste, settings_store, snippets_store,
    state::{AppState, CapturedTarget},
    window,
};

// Keeps the snippets picker's list readable — long labels just wrap/clip
// there anyway, so there's no value in keeping the full text.
const AUTO_LABEL_MAX_CHARS: usize = 40;

/// Derives a snippet label from its own text (Cmd+Enter pins with no
/// separate naming step — see Popup.tsx) by truncating at the last word
/// boundary within the limit, rather than mid-word.
fn auto_label(text: &str) -> String {
    if text.chars().count() <= AUTO_LABEL_MAX_CHARS {
        return text.to_string();
    }
    let truncated: String = text.chars().take(AUTO_LABEL_MAX_CHARS).collect();
    let trimmed = truncated.rsplit_once(' ').map_or(truncated.as_str(), |(head, _)| head);
    format!("{trimmed}…")
}

const NO_TARGET_MESSAGE: &str =
    "No había una app donde pegar; la traducción quedó en el portapapeles.";

fn copy_to_clipboard(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(text))
        .map_err(|e| e.to_string())
}

/// Hides the popup/snippets window with `hide`, brings the captured target to
/// the front and pastes `text` there. Must run on the main thread. With no
/// usable target (none captured, or the captured app has since quit) nothing
/// is hidden or pasted: the text is left on the clipboard and the error tells
/// the user so. When the target is TypeLang's own Settings window no
/// keystroke is simulated: this thread is also the one that would have to
/// process it, so the paste would run before the keystroke and restore the
/// old clipboard. Settings is focused and the text is left on the clipboard
/// for the user to paste with Cmd+V.
pub(crate) fn paste_to_target(
    app: &AppHandle,
    target: Option<CapturedTarget>,
    text: &str,
    hide: fn(&AppHandle),
) -> Result<(), String> {
    match target {
        Some(CapturedTarget::App { app: target_app, .. }) if target_app.is_alive() => {
            hide(app);
            focus::restore(&target_app)?;
            paste::paste_text(text)
        }
        Some(CapturedTarget::SelfSettings) if window::settings_visible(app) => {
            hide(app);
            if let Some(settings) = app.get_webview_window(window::SETTINGS_LABEL) {
                let _ = settings.set_focus();
            }
            copy_to_clipboard(text)
        }
        _ => {
            copy_to_clipboard(text)?;
            Err(NO_TARGET_MESSAGE.to_string())
        }
    }
}

/// Called by the popup on submit. Translates the input, then hides the
/// popup, restores focus to the originally-captured app, and pastes. When
/// `pin` is set (Cmd+Enter instead of plain Enter), also saves the result
/// as a snippet — see the "snippet memory" feature this backs.
#[tauri::command]
pub async fn translate_and_paste(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
    tone: String,
    pin: bool,
    from_voice: Option<bool>,
) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("empty input".to_string());
    }

    let settings = settings_store::load(&app)?;
    let opts = llm::PromptOptions {
        from_voice: from_voice.unwrap_or(false),
        vocabulary: settings.vocabulary_terms(),
        keep_language: false,
    };
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
    let translated = llm::translate(&cfg, text, &settings.source_lang, &settings.target_lang, &tone, &opts)
        .await?
        .trim()
        .to_string();

    let captured = state.captured_focus.lock().map_err(|e| e.to_string())?.clone();

    // CoreGraphics keystroke posting is kept on the main thread, which
    // AppKit-backed APIs can assert on. Async commands run on a tokio worker
    // thread, so calling paste::paste_text directly here is unsafe. Hop the hide/restore/paste sequence
    // onto the main thread and bring the result back over a channel.
    // Cloned rather than moved so `translated` is still around afterward to
    // pin as a snippet.
    let text_for_paste = translated.clone();
    let (tx, rx) = oneshot::channel();
    let app_for_main = app.clone();
    app.run_on_main_thread(move || {
        let result = paste_to_target(&app_for_main, captured, &text_for_paste, window::hide_popup);
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;

    rx.await.map_err(|e| e.to_string())??;

    if pin {
        // Best-effort: the paste already succeeded, so a snippet-save
        // failure (e.g. a disk error) shouldn't surface as a translate
        // failure to the user.
        let _ = snippets_store::add(&app, auto_label(&translated), translated);
    }

    Ok(())
}

#[tauri::command]
pub fn cancel_popup(app: AppHandle) {
    window::hide_popup(&app);
    window::release_activation(&app);
}

#[tauri::command]
pub fn open_settings_from_popup(app: AppHandle) {
    window::hide_popup(&app);
    window::show_settings(&app);
}

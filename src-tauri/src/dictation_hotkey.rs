//! Hold-to-talk dictation: hold the dictation shortcut to record, release to
//! transcribe (and translate, per setting) and paste at the cursor of the app
//! that was frontmost. No window is shown, so focus never leaves that app.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;

use crate::{
    audio, focus,
    focus::FocusedApp,
    keychain, llm, paste, settings_store,
    state::{AppState, HotkeyStart},
    stt, tray, window,
};

const RECORDING_STATUS: &str = "●";
const WORKING_STATUS: &str = "…";
const ERROR_STATUS: &str = "⚠";
const ERROR_SHOWN: Duration = Duration::from_secs(4);
/// A job busy for longer than the recorder's own cap plus its grace period
/// lost its release (e.g. the shortcut was swallowed): treat it as stale.
const STALE_AFTER: Duration = Duration::from_secs(150);

fn is_current(app: &AppHandle, id: u64) -> bool {
    app.state::<AppState>().hotkey_gen.load(Ordering::SeqCst) == id
}

fn popup_visible(app: &AppHandle) -> bool {
    app.get_webview_window(window::POPUP_LABEL)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

/// Shortcut pressed: start recording. OS key-repeat presses and presses during
/// a running job are ignored through the busy flag.
pub fn on_pressed(app: &AppHandle) {
    let state = app.state::<AppState>();
    if popup_visible(app) {
        return;
    }
    if state.hotkey_busy.load(Ordering::SeqCst) {
        let stale = state
            .hotkey_started
            .lock()
            .ok()
            .and_then(|s| *s)
            .is_none_or(|t| t.elapsed() > STALE_AFTER);
        if !stale {
            return;
        }
        eprintln!("[dictation] stale job reset");
        // The old release never came: drop its recording, never transcribe it.
        if let Some(old) = state.hotkey_start.lock().ok().and_then(|mut s| s.take()) {
            old.abort();
        }
        audio::cancel(&state);
    }

    let id = state.hotkey_gen.fetch_add(1, Ordering::SeqCst) + 1;
    state.hotkey_busy.store(true, Ordering::SeqCst);
    if let Ok(mut started) = state.hotkey_started.lock() {
        *started = Some(Instant::now());
    }
    tray::set_status(app, Some(RECORDING_STATUS));

    let app_for_start = app.clone();
    let handle = tauri::async_runtime::spawn(async move { start(&app_for_start, id).await });
    if let Ok(mut slot) = state.hotkey_start.lock() {
        *slot = Some(handle);
    };
}

/// Captures the frontmost app (off the main thread: it shells out) and opens
/// the microphone.
async fn start(app: &AppHandle, id: u64) -> Result<Option<FocusedApp>, String> {
    let target = tauri::async_runtime::spawn_blocking(focus::capture)
        .await
        .ok()
        .and_then(Result::ok);
    let state = app.state::<AppState>();
    if let Err(e) = audio::start(&state).await {
        audio::cancel(&state);
        if is_current(app, id) {
            tray::set_status(app, None);
        }
        return Err(e);
    }
    Ok(target)
}

/// Shortcut released: stop recording and run the rest of the job.
pub fn on_released(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some(start) = state.hotkey_start.lock().ok().and_then(|mut s| s.take()) else {
        return;
    };
    let app = app.clone();
    let id = state.hotkey_gen.load(Ordering::SeqCst);
    tauri::async_runtime::spawn(async move {
        let result = run(&app, id, start).await;
        finish(&app, id, result);
    });
}

/// Clears the busy state in every exit path, then shows the error if any. A
/// superseded job only logs: the newer job owns the busy flag and the tray.
fn finish(app: &AppHandle, id: u64, result: Result<(), String>) {
    if !is_current(app, id) {
        if let Err(e) = result {
            eprintln!("[dictation] superseded job failed: {e}");
        }
        return;
    }
    let state = app.state::<AppState>();
    state.hotkey_busy.store(false, Ordering::SeqCst);
    if let Ok(mut started) = state.hotkey_started.lock() {
        *started = None;
    }
    match result {
        Ok(()) => tray::set_status(app, None),
        Err(e) => {
            eprintln!("[dictation] {e}");
            tray::set_status(app, Some(ERROR_STATUS));
            tray::set_tooltip(app, Some(&e));
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(ERROR_SHOWN).await;
                // A new job already owns the tray text.
                if !app.state::<AppState>().hotkey_busy.load(Ordering::SeqCst) {
                    tray::set_status(&app, None);
                    tray::set_tooltip(&app, None);
                }
            });
        }
    }
}

async fn run(app: &AppHandle, id: u64, start: HotkeyStart) -> Result<(), String> {
    let target = start.await.map_err(|e| e.to_string())??;

    // A newer job owns the recorder now; stopping it here would steal its audio.
    if !is_current(app, id) {
        return Ok(());
    }
    let state = app.state::<AppState>();
    let Some(target) = target else {
        audio::cancel(&state);
        return Err("No se pudo detectar la app destino".to_string());
    };
    let wav = audio::stop(&state).await?;
    if wav.is_empty() {
        return Ok(());
    }
    if is_current(app, id) {
        tray::set_status(app, Some(WORKING_STATUS));
    }

    let settings = settings_store::load(app)?;
    let vocabulary = settings.vocabulary_terms();
    let transcript =
        stt::transcribe(&settings.stt_base_url, wav, &settings.source_lang, &vocabulary).await?;
    if transcript.is_empty() {
        return Ok(());
    }

    let text = if settings.dictation_output == "original" {
        transcript
    } else {
        let opts = llm::PromptOptions {
            from_voice: true,
            vocabulary,
        };
        let cfg = llm::ProviderConfig {
            api_key: keychain::get_api_key(&settings.provider)?,
            provider: settings.provider,
            base_url: Some(settings.lmstudio_base_url),
            model: Some(settings.local_model),
        };
        llm::translate(
            &cfg,
            &transcript,
            &settings.source_lang,
            &settings.target_lang,
            &settings.tone,
            &opts,
        )
        .await?
    };
    let text = text.trim().to_string();
    if text.is_empty() {
        return Ok(());
    }

    // Keystroke posting is kept on the main thread, same as the popup flow.
    let (tx, rx) = oneshot::channel();
    let app_for_main = app.clone();
    app.run_on_main_thread(move || {
        let result = (|| -> Result<(), String> {
            if !is_current(&app_for_main, id) {
                return Ok(());
            }
            let still_front = focus::capture().map(|f| f.is_same(&target));
            if still_front != Ok(true) {
                focus::restore(&target)?;
            }
            paste::paste_text(&text)
        })();
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    rx.await.map_err(|e| e.to_string())?
}

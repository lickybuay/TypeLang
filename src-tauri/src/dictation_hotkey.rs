//! Dictation shortcut: hold it to record and release to transcribe (and
//! translate, per setting) and paste at the cursor of the app that was
//! frontmost, or tap it twice to keep recording hands-free until the next
//! press. A small non-focusable pill (see `window::show_pill`) shows the
//! recording and lets the user finish or discard it, so focus never leaves the
//! target app.
//!
//! The decisions live in `dictation_machine` (pure, testable); this module
//! feeds it events under one mutex and executes the actions it returns.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;

use crate::{
    audio,
    dictation_machine::{self as machine, Action, Event, Machine, PillView, TimerKind, TrayView},
    focus,
    focus::FocusedApp,
    keychain, llm, paste, settings_store,
    state::{AppState, HotkeyStart},
    stt, tray, window,
};

const RECORDING_STATUS: &str = "●";
const WORKING_STATUS: &str = "…";
const ERROR_STATUS: &str = "⚠";
const NO_TARGET_STATUS: &str = "No había una app donde pegar";
const PILL_EVENT: &str = "pill-state";

/// Everything the dictation shortcut mutates, behind one mutex in `AppState`.
#[derive(Default)]
pub struct DictationRuntime {
    machine: Machine,
    /// The press-time task (focus capture + recorder start) of the current
    /// session; accepting awaits it so stop can never overtake start.
    start: Option<HotkeyStart>,
}

/// What the pill window receives (event `pill-state` and command
/// `dictation_pill_state`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PillPayload {
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

impl From<&PillView> for PillPayload {
    fn from(view: &PillView) -> Self {
        let (state, started_at_ms, message) = match view {
            PillView::Starting => ("starting", None, None),
            PillView::Recording { started_at_ms } => ("recording", Some(*started_at_ms), None),
            PillView::Locked { started_at_ms } => ("locked", Some(*started_at_ms), None),
            PillView::Processing => ("processing", None, None),
            PillView::Hint => ("hint", None, None),
            PillView::Error(message) => ("error", None, Some(message.clone())),
        };
        PillPayload {
            state,
            started_at_ms,
            message,
        }
    }
}

fn is_current(app: &AppHandle, id: u64) -> bool {
    app.state::<AppState>()
        .dictation
        .lock()
        .is_ok_and(|rt| rt.machine.gen == id)
}

fn popup_visible(app: &AppHandle) -> bool {
    app.get_webview_window(window::POPUP_LABEL)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Shortcut pressed. Ignored while the translate popup is open.
pub fn on_pressed(app: &AppHandle) {
    if popup_visible(app) {
        return;
    }
    dispatch(app, Event::Pressed);
}

/// Shortcut released.
pub fn on_released(app: &AppHandle) {
    dispatch(app, Event::Released);
}

/// The pill's check mark: stop, transcribe and paste.
pub fn pill_accept(app: &AppHandle) {
    dispatch(app, Event::PillAccept);
}

/// The pill's cross: discard the recording, or abandon the running job.
pub fn pill_cancel(app: &AppHandle) {
    dispatch(app, Event::PillCancel);
}

/// The popup or the snippets picker is about to open: drop any dictation and
/// hide the pill first. A no-op while idle.
pub fn external_cancel(app: &AppHandle) {
    dispatch(app, Event::ExternalCancel);
}

/// What the pill should show right now, if anything.
pub fn pill_state(app: &AppHandle) -> Option<PillPayload> {
    let state = app.state::<AppState>();
    let rt = state.dictation.lock().ok()?;
    rt.machine.pill.as_ref().map(PillPayload::from)
}

/// Feeds `event` to the machine and runs the resulting actions. The lock is
/// released before any action runs: they touch windows and spawn tasks, which
/// must never wait on this mutex.
fn dispatch(app: &AppHandle, event: Event) {
    let actions = {
        let state = app.state::<AppState>();
        let Ok(mut rt) = state.dictation.lock() else {
            return;
        };
        let (next, actions) = machine::step(&rt.machine, event, Instant::now());
        rt.machine = next;
        actions
    };
    for action in actions {
        execute(app, action);
    }
}

fn execute(app: &AppHandle, action: Action) {
    match action {
        Action::Start { gen } => start_session(app, gen),
        Action::Accept { gen, recapture } => spawn_job(app, gen, recapture),
        Action::ReleaseRecording => release_recording(app),
        Action::SyncPill => sync_pill(app),
        Action::Tray(view) => set_tray(app, &view),
        Action::Arm { kind, gen, after } => arm_timer(app, kind, gen, after),
    }
}

fn set_tray(app: &AppHandle, view: &TrayView) {
    match view {
        TrayView::Clear => {
            tray::set_status(app, None);
            tray::set_tooltip(app, None);
        }
        TrayView::Recording => tray::set_status(app, Some(RECORDING_STATUS)),
        TrayView::Working => tray::set_status(app, Some(WORKING_STATUS)),
        TrayView::Error(message) => {
            tray::set_status(app, Some(ERROR_STATUS));
            tray::set_tooltip(app, Some(message));
        }
    }
}

fn arm_timer(app: &AppHandle, kind: TimerKind, gen: u64, after: Duration) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(after).await;
        dispatch(&app, Event::Timer { kind, gen });
    });
}

/// Makes the pill window match the machine's desired view. Runs on the main
/// thread, one job per change, and reads the view when it runs: window
/// operations from different threads can therefore never leave the pill
/// showing an older decision.
fn sync_pill(app: &AppHandle) {
    let app_for_main = app.clone();
    if let Err(e) = app.run_on_main_thread(move || {
        let view = app_for_main
            .state::<AppState>()
            .dictation
            .lock()
            .ok()
            .and_then(|rt| rt.machine.pill.clone());
        let Some(view) = view else {
            window::hide_pill(&app_for_main);
            return;
        };
        if let Err(e) = window::ensure_pill(&app_for_main) {
            eprintln!("[dictation] could not create the pill: {e}");
            return;
        }
        // State first, then show: a window that was hidden must not flash
        // the previous state.
        if let Err(e) = app_for_main.emit_to(window::PILL_LABEL, PILL_EVENT, PillPayload::from(&view)) {
            eprintln!("[dictation] could not update the pill: {e}");
        }
        if let Err(e) = window::show_pill(&app_for_main) {
            eprintln!("[dictation] could not show the pill: {e}");
        }
    }) {
        eprintln!("[dictation] could not sync the pill: {e}");
    }
}

/// Spawns the press-time task and keeps its handle for accept/cancel.
fn start_session(app: &AppHandle, gen: u64) {
    let app_for_start = app.clone();
    let handle = tauri::async_runtime::spawn(async move { start(&app_for_start, gen).await });
    let state = app.state::<AppState>();
    let Ok(mut rt) = state.dictation.lock() else {
        handle.abort();
        return;
    };
    if rt.machine.gen != gen {
        // Cancelled before the handle could be stored.
        handle.abort();
        return;
    }
    if let Some(old) = rt.start.replace(handle) {
        old.abort();
    }
}

/// Captures the frontmost app (off the main thread: it shells out), then opens
/// the microphone. The pill appears once the capture is done: capturing after
/// a window is shown could see that window instead of the target app.
async fn start(app: &AppHandle, gen: u64) -> Result<Option<FocusedApp>, String> {
    let target = tauri::async_runtime::spawn_blocking(focus::capture)
        .await
        .ok()
        .and_then(Result::ok);
    dispatch(app, Event::Captured { gen });

    let state = app.state::<AppState>();
    match audio::start(&state).await {
        Ok(()) => {
            dispatch(
                app,
                Event::AudioStarted {
                    gen,
                    at_ms: epoch_ms(),
                },
            );
            Ok(target)
        }
        Err(e) => {
            // A newer session owns the recorder: stopping it here would steal
            // its audio.
            if is_current(app, gen) {
                audio::cancel(&state);
            }
            dispatch(
                app,
                Event::StartFailed {
                    gen,
                    message: e.clone(),
                },
            );
            Err(e)
        }
    }
}

/// Drops the in-flight start task and the recording.
fn release_recording(app: &AppHandle) {
    let state = app.state::<AppState>();
    let old = state.dictation.lock().ok().and_then(|mut rt| rt.start.take());
    if let Some(old) = old {
        old.abort();
    }
    audio::cancel(&state);
}

/// Runs the rest of the job for session `gen` and reports the outcome back to
/// the machine.
fn spawn_job(app: &AppHandle, gen: u64, recapture: bool) {
    let start = app
        .state::<AppState>()
        .dictation
        .lock()
        .ok()
        .and_then(|mut rt| rt.start.take());
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = match start {
            Some(start) => run(&app, gen, start, recapture).await,
            None => Err("No hay una grabación en curso".to_string()),
        };
        if let Err(e) = &result {
            eprintln!("[dictation] {e}");
        }
        // A superseded job's outcome is ignored by the machine.
        dispatch(
            &app,
            Event::JobFinished {
                gen,
                error: result.err(),
            },
        );
    });
}

/// With `recapture` (the session was locked) the paste target is whatever app
/// is in front when the user accepts, not the one captured at the first press:
/// a locked recording can last minutes, and the user may have moved on. A hold
/// is over in seconds, so it keeps the captured target and restores it.
async fn run(app: &AppHandle, id: u64, start: HotkeyStart, recapture: bool) -> Result<(), String> {
    let mut target = start.await.map_err(|e| e.to_string())??;

    // A newer session owns the recorder now; stopping it here would steal its
    // audio.
    if !is_current(app, id) {
        return Ok(());
    }
    let state = app.state::<AppState>();
    if target.is_none() && !recapture {
        audio::cancel(&state);
        return Err("No se pudo detectar la app destino".to_string());
    }
    let wav = audio::stop(&state).await?;
    if wav.is_empty() {
        return Ok(());
    }
    if recapture {
        let fresh = tauri::async_runtime::spawn_blocking(focus::capture)
            .await
            .ok()
            .and_then(Result::ok);
        // TypeLang in front says nothing about where the user was (the pill
        // should not activate it, but if it ever does): keep the app captured
        // at the start, with its restore path, instead of failing.
        if !fresh.as_ref().is_some_and(FocusedApp::is_self) {
            target = fresh;
        }
    }
    let Some(target) = target else {
        return Err("No se pudo detectar la app destino".to_string());
    };

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
            if target.is_self() {
                // Never restore ourselves: TypeLang can only be the target
                // when its Settings window is the one in front. And never
                // simulate a keystroke at ourselves either: it would only be
                // processed after this thread has restored the old clipboard,
                // so the text is left on the clipboard for the user to paste.
                if !window::settings_focused(&app_for_main) {
                    return Err(NO_TARGET_STATUS.to_string());
                }
                return arboard::Clipboard::new()
                    .and_then(|mut clipboard| clipboard.set_text(&text))
                    .map_err(|e| e.to_string());
            } else {
                let still_front = focus::capture().map(|f| f.is_same(&target));
                if still_front != Ok(true) {
                    focus::restore(&target)?;
                }
            }
            paste::paste_text(&text)
        })();
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    rx.await.map_err(|e| e.to_string())?
}

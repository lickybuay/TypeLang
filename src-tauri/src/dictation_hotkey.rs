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
const NOT_TRANSLATED_NOTICE: &str = "No se tradujo: la transcripción quedó en el portapapeles";
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
            PillView::Notice(message) => ("notice", None, Some(message.clone())),
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
        TrayView::Error(message) | TrayView::Notice(message) => {
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
        let result: Result<Option<String>, String> = match start {
            Some(start) => run(&app, gen, start, recapture).await,
            None => Err("No hay una grabación en curso".to_string()),
        };
        if let Err(e) = &result {
            eprintln!("[dictation] {e}");
        }
        let (error, notice) = match result {
            Ok(notice) => (None, notice),
            Err(e) => (Some(e), None),
        };
        if let Some(notice) = &notice {
            eprintln!("[dictation] {notice}");
        }
        // A superseded job's outcome is ignored by the machine.
        dispatch(&app, Event::JobFinished { gen, error, notice });
    });
}

/// What to paste for `mode` ("translate", "improve" or "original"), given the
/// transcript and what the LLM answered (`None` when it was not asked).
/// The second value is a notice for the user when the result is not what the
/// mode promises: "improve" never fails the job, it falls back to the
/// transcript and says so. A failed translation stays an error, except a reply
/// that was not a translation: that returns the transcript with a notice, and
/// `copy_only` says it goes to the clipboard instead of being pasted.
///
/// The notice is one of a few fixed texts, never the provider's error: that
/// text is for the log, and it can carry details that do not belong on screen.
fn choose_output(
    mode: &str,
    transcript: String,
    llm: Option<Result<String, String>>,
) -> Result<(String, Option<String>), String> {
    let unimproved = |transcript: String, reason: &str| {
        Ok((transcript, Some(format!("Pegado sin mejorar: {reason}"))))
    };
    match (mode, llm) {
        ("original", _) => Ok((transcript, None)),
        ("improve", Some(Ok(reply))) => match llm::improved_or_none(&transcript, &reply) {
            Some(text) => Ok((text, None)),
            None => unimproved(transcript, "la respuesta no parecía una corrección"),
        },
        ("improve", Some(Err(e))) if e == llm::TRUNCATED => {
            unimproved(transcript, "la respuesta se cortó")
        }
        ("improve", Some(Err(_)) | None) => unimproved(transcript, "el modelo no respondió"),
        ("translate", Some(Err(e))) if e == llm::NOT_A_TRANSLATION => {
            Ok((transcript, Some(NOT_TRANSLATED_NOTICE.to_string())))
        }
        (_, Some(reply)) => reply.map(|text| (text, None)),
        (_, None) => Err("El modelo no respondió".to_string()),
    }
}

/// Whether `choose_output` returns the transcript to copy rather than paste.
/// Pasting it in a translate flow would put the wrong language in the target
/// app.
fn copy_only(mode: &str, reply: &Option<Result<String, String>>) -> bool {
    mode == "translate" && matches!(reply, Some(Err(e)) if e == llm::NOT_A_TRANSLATION)
}

fn copy_to_clipboard(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(text))
        .map_err(|e| e.to_string())
}

/// With `recapture` (the session was locked) the paste target is whatever app
/// is in front when the user accepts, not the one captured at the first press:
/// a locked recording can last minutes, and the user may have moved on. A hold
/// is over in seconds, so it keeps the captured target and restores it.
///
/// `Ok(Some(notice))` means the text was pasted but not as configured.
async fn run(
    app: &AppHandle,
    id: u64,
    start: HotkeyStart,
    recapture: bool,
) -> Result<Option<String>, String> {
    let mut target = start.await.map_err(|e| e.to_string())??;

    // A newer session owns the recorder now; stopping it here would steal its
    // audio.
    if !is_current(app, id) {
        return Ok(None);
    }
    let state = app.state::<AppState>();
    if target.is_none() && !recapture {
        audio::cancel(&state);
        return Err("No se pudo detectar la app destino".to_string());
    }
    let wav = audio::stop(&state).await?;
    if wav.is_empty() {
        return Ok(None);
    }
    if recapture {
        let fresh = tauri::async_runtime::spawn_blocking(focus::capture)
            .await
            .ok()
            .and_then(Result::ok);
        // TypeLang in front says nothing about where the user was (the pill
        // should not activate it, but if it ever does): keep the app captured
        // at the start, with its restore path, instead of failing. The one
        // exception is Settings being focused: that is a real target, and
        // the self-target rules below copy the text for it.
        let stray_self = fresh.as_ref().is_some_and(FocusedApp::is_self)
            && !window::settings_focused(app);
        if !stray_self {
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
        return Ok(None);
    }

    // An unknown stored value behaves as "translate".
    let mode = settings_store::parse_dictation_output(&settings.dictation_output)
        .unwrap_or("translate");
    let reply = if mode == "original" {
        None
    } else {
        // "improve" edits in the spoken language: source as target, and the
        // tone setting does not apply (see `llm::system_prompt`).
        let keep_language = mode == "improve";
        let opts = llm::PromptOptions {
            from_voice: true,
            vocabulary,
            keep_language,
        };
        let target_lang = if keep_language {
            &settings.source_lang
        } else {
            &settings.target_lang
        };
        // Built inside the async block: a missing API key must fall back to
        // the transcript in "improve" mode like any other model failure.
        let reply = async {
            let cfg = llm::ProviderConfig {
                api_key: keychain::get_api_key(&settings.provider)?,
                provider: settings.provider.clone(),
                base_url: Some(settings.lmstudio_base_url.clone()),
                model: Some(settings.local_model.clone()),
            };
            llm::translate(
                &cfg,
                &transcript,
                &settings.source_lang,
                target_lang,
                &settings.tone,
                &opts,
            )
            .await
    }
        .await;
        // The notice shown to the user is fixed text; the real reason stays
        // in the log.
        if let (true, Err(e)) = (keep_language, &reply) {
            eprintln!("[dictation] improve failed: {e}");
        }
        Some(reply)
    };
    let skip_paste = copy_only(mode, &reply);
    let (text, notice) = choose_output(mode, transcript, reply)?;
    let text = text.trim().to_string();
    if text.is_empty() {
        return Ok(None);
    }

    // Keystroke posting is kept on the main thread, same as the popup flow.
    let (tx, rx) = oneshot::channel();
    let app_for_main = app.clone();
    app.run_on_main_thread(move || {
        let result = (|| -> Result<(), String> {
            if !is_current(&app_for_main, id) {
                return Ok(());
            }
            // Nothing is pasted, so no target app is needed (or restored).
            if skip_paste {
                return copy_to_clipboard(&text);
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
                return copy_to_clipboard(&text);
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
    rx.await.map_err(|e| e.to_string())??;
    Ok(notice)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRANSCRIPT: &str = "eh o sea mañana llego tarde";

    fn chosen(
        mode: &str,
        llm: Option<Result<String, String>>,
    ) -> Result<(String, Option<String>), String> {
        choose_output(mode, TRANSCRIPT.to_string(), llm)
    }

    #[test]
    fn original_pastes_the_transcript_without_asking_the_model() {
        assert_eq!(chosen("original", None), Ok((TRANSCRIPT.to_string(), None)));
        // Even if a reply is there, it is not used.
        assert_eq!(
            chosen("original", Some(Ok("other".to_string()))),
            Ok((TRANSCRIPT.to_string(), None))
        );
    }

    #[test]
    fn translate_pastes_the_reply() {
        assert_eq!(
            chosen("translate", Some(Ok("tomorrow I'll be late".to_string()))),
            Ok(("tomorrow I'll be late".to_string(), None))
        );
    }

    #[test]
    fn translate_error_stays_an_error() {
        assert_eq!(
            chosen("translate", Some(Err("boom".to_string()))),
            Err("boom".to_string())
        );
        assert!(chosen("translate", None).is_err());
    }

    #[test]
    fn improve_pastes_the_checked_reply() {
        assert_eq!(
            chosen("improve", Some(Ok("  Mañana llego tarde.\n".to_string()))),
            Ok(("Mañana llego tarde.".to_string(), None))
        );
    }

    #[test]
    fn improve_falls_back_on_error_with_a_fixed_notice() {
        // The provider's text (here with a key-like secret) never reaches the notice.
        let raw = "network error calling Gemini: https://x/?key=SECRET timed out";
        let (text, notice) = chosen("improve", Some(Err(raw.to_string()))).unwrap();
        assert_eq!(text, TRANSCRIPT);
        assert_eq!(notice.as_deref(), Some("Pegado sin mejorar: el modelo no respondió"));
    }

    #[test]
    fn improve_notice_names_a_truncated_reply() {
        let (text, notice) =
            chosen("improve", Some(Err(llm::TRUNCATED.to_string()))).unwrap();
        assert_eq!(text, TRANSCRIPT);
        assert_eq!(notice.as_deref(), Some("Pegado sin mejorar: la respuesta se cortó"));
    }

    #[test]
    fn improve_falls_back_when_the_post_check_rejects_the_reply() {
        for reply in ["", "   ", "Claro, aquí tienes el texto corregido: Mañana llego tarde."] {
            let (text, notice) = chosen("improve", Some(Ok(reply.to_string()))).unwrap();
            assert_eq!(text, TRANSCRIPT, "{reply:?}");
            assert_eq!(
                notice.as_deref(),
                Some("Pegado sin mejorar: la respuesta no parecía una corrección"),
                "{reply:?}"
            );
        }
    }

    #[test]
    fn improve_without_a_reply_falls_back() {
        let (text, notice) = chosen("improve", None).unwrap();
        assert_eq!(text, TRANSCRIPT);
        assert_eq!(notice.as_deref(), Some("Pegado sin mejorar: el modelo no respondió"));
    }

    #[test]
    fn translate_that_is_not_a_translation_returns_the_transcript_with_a_notice() {
        let reply = Some(Err(llm::NOT_A_TRANSLATION.to_string()));
        assert_eq!(
            chosen("translate", reply),
            Ok((TRANSCRIPT.to_string(), Some(NOT_TRANSLATED_NOTICE.to_string())))
        );
    }

    #[test]
    fn copy_only_applies_just_to_a_translate_that_is_not_a_translation() {
        assert!(copy_only("translate", &Some(Err(llm::NOT_A_TRANSLATION.to_string()))));
        assert!(!copy_only("translate", &Some(Ok("hello".to_string()))));
        assert!(!copy_only("translate", &Some(Err("boom".to_string()))));
        assert!(!copy_only("improve", &Some(Err(llm::NOT_A_TRANSLATION.to_string()))));
        assert!(!copy_only("original", &None));
    }
}

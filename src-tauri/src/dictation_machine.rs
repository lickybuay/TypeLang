//! Pure state machine behind the dictation shortcut. No Tauri types and no
//! clock reads: `step` takes the current time and returns the next state plus
//! the side effects to run, so every transition can be unit-tested. The glue in
//! `dictation_hotkey.rs` feeds it events and executes the actions.
//!
//! Gestures (one 500 ms constant is both the hold threshold and the
//! double-tap window):
//! - hold >= 500 ms, release        -> stop, transcribe, paste
//! - tap < 500 ms, second press     -> locked: keeps recording until the next
//!   press, which stops and pastes
//! - tap < 500 ms, nothing after it -> discarded, with a hint
//!
//! Timers and background jobs are tagged with the generation they were
//! created in. The generation is bumped when a session starts and whenever it
//! is cancelled, so a stale timer or a superseded job is a no-op.

use std::time::{Duration, Instant};

/// Hold threshold and double-tap window.
pub const TAP_WINDOW: Duration = Duration::from_millis(500);
/// Longest hotkey recording; keep it at or under `audio::MAX_RECORDING_SECS`.
pub const HOTKEY_MAX_SECS: u64 = 300;
/// How long the "hold, or tap twice" hint stays on screen.
pub const HINT_SHOWN: Duration = Duration::from_millis(1500);
pub const ERROR_PILL_SHOWN: Duration = Duration::from_secs(3);
pub const ERROR_TRAY_SHOWN: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    /// Shortcut is down; `since` is the press time.
    Holding { since: Instant },
    /// Released after a short tap; waiting for a second press.
    PendingDouble,
    /// Recording hands-free until the next press.
    Locked,
    /// Stopped; transcribing, translating and pasting.
    Processing,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Machine {
    pub phase: Phase,
    /// Id of the current session. Bumped on start and on cancel.
    pub gen: u64,
    /// Epoch milliseconds when the microphone opened; `None` until then.
    pub started_at_ms: Option<u64>,
    /// True once the session went through Locked. A locked recording can run
    /// for minutes, so the app captured at the first press may be long gone.
    pub was_locked: bool,
    /// What the pill should show; `None` means hidden. Decided here, under
    /// the same lock as the phase, so the window only ever syncs to it.
    pub pill: Option<PillView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerKind {
    /// 500 ms after a short release: the second press never came.
    Tap,
    /// Recording reached `HOTKEY_MAX_SECS`.
    Cap,
    HidePill,
    ClearTray,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Pressed,
    Released,
    PillAccept,
    PillCancel,
    /// Something else (popup, snippets) is about to take over the screen.
    ExternalCancel,
    /// The frontmost app was captured; the pill can appear.
    Captured { gen: u64 },
    AudioStarted { gen: u64, at_ms: u64 },
    StartFailed { gen: u64, message: String },
    JobFinished { gen: u64, error: Option<String> },
    Timer { kind: TimerKind, gen: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PillView {
    Starting,
    Recording { started_at_ms: u64 },
    Locked { started_at_ms: u64 },
    Processing,
    Hint,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayView {
    Clear,
    Recording,
    Working,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Capture the target app, show the pill, open the microphone.
    Start { gen: u64 },
    /// Stop the recording and run transcribe -> translate -> paste. With
    /// `recapture` the paste target is the app in front now, not the one
    /// captured at the first press.
    Accept { gen: u64, recapture: bool },
    /// Drop the in-flight start and the recording.
    ReleaseRecording,
    /// Make the pill window match `Machine::pill` (show or hide).
    SyncPill,
    Tray(TrayView),
    Arm {
        kind: TimerKind,
        gen: u64,
        after: Duration,
    },
}

fn is_recording(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::Holding { .. } | Phase::PendingDouble | Phase::Locked
    )
}

/// The pill's look while recording, or `None` while the microphone is still
/// opening (the pill keeps showing "starting").
fn recording_view(m: &Machine) -> Option<PillView> {
    let started_at_ms = m.started_at_ms?;
    Some(if m.phase == Phase::Locked {
        PillView::Locked { started_at_ms }
    } else {
        PillView::Recording { started_at_ms }
    })
}

fn set_pill(next: &mut Machine, out: &mut Vec<Action>, view: Option<PillView>) {
    next.pill = view;
    out.push(Action::SyncPill);
}

fn accept(next: &mut Machine, out: &mut Vec<Action>) {
    next.phase = Phase::Processing;
    out.push(Action::Accept {
        gen: next.gen,
        recapture: next.was_locked,
    });
    set_pill(next, out, Some(PillView::Processing));
    out.push(Action::Tray(TrayView::Working));
}

/// Back to Idle under a new generation, so nothing from the old session can
/// act any more.
fn reset(next: &mut Machine) {
    next.gen += 1;
    next.phase = Phase::Idle;
    next.started_at_ms = None;
    next.was_locked = false;
}

fn cancel(next: &mut Machine, out: &mut Vec<Action>) {
    reset(next);
    out.push(Action::ReleaseRecording);
    set_pill(next, out, None);
    out.push(Action::Tray(TrayView::Clear));
}

fn show_error(next: &mut Machine, message: String, out: &mut Vec<Action>) {
    out.push(Action::Tray(TrayView::Error(message.clone())));
    set_pill(next, out, Some(PillView::Error(message)));
    let gen = next.gen;
    out.push(Action::Arm {
        kind: TimerKind::HidePill,
        gen,
        after: ERROR_PILL_SHOWN,
    });
    out.push(Action::Arm {
        kind: TimerKind::ClearTray,
        gen,
        after: ERROR_TRAY_SHOWN,
    });
}

/// Pure transition function: the next machine state and the actions to run,
/// in order.
pub fn step(m: &Machine, event: Event, now: Instant) -> (Machine, Vec<Action>) {
    let mut next = m.clone();
    let mut out = Vec::new();

    match event {
        Event::Pressed => match m.phase {
            Phase::Idle => {
                next.gen += 1;
                next.started_at_ms = None;
                next.was_locked = false;
                next.phase = Phase::Holding { since: now };
                out.push(Action::Start { gen: next.gen });
                out.push(Action::Tray(TrayView::Recording));
            }
            Phase::PendingDouble => {
                next.phase = Phase::Locked;
                next.was_locked = true;
                if let Some(view) = recording_view(&next) {
                    set_pill(&mut next, &mut out, Some(view));
                }
            }
            Phase::Locked => accept(&mut next, &mut out),
            // Key repeat while held, or a press while a job runs.
            Phase::Holding { .. } | Phase::Processing => {}
        },
        Event::Released => {
            if let Phase::Holding { since } = m.phase {
                if now.saturating_duration_since(since) >= TAP_WINDOW {
                    accept(&mut next, &mut out);
                } else {
                    next.phase = Phase::PendingDouble;
                    out.push(Action::Arm {
                        kind: TimerKind::Tap,
                        gen: m.gen,
                        after: TAP_WINDOW,
                    });
                }
            }
            // A release in any other phase (locked press, key repeat tail,
            // processing) carries no meaning.
        }
        Event::PillAccept => {
            if is_recording(m.phase) {
                accept(&mut next, &mut out);
            }
        }
        Event::PillCancel => {
            if m.phase != Phase::Idle {
                cancel(&mut next, &mut out);
            }
        }
        Event::ExternalCancel => {
            if m.phase != Phase::Idle {
                cancel(&mut next, &mut out);
            } else if m.pill.is_some() {
                // A hint or error pill is still up: hide it, and bump the
                // generation so its pending timers do nothing.
                reset(&mut next);
                set_pill(&mut next, &mut out, None);
                out.push(Action::Tray(TrayView::Clear));
            }
        }
        Event::Captured { gen } => {
            if gen == m.gen && is_recording(m.phase) && m.started_at_ms.is_none() {
                set_pill(&mut next, &mut out, Some(PillView::Starting));
            }
        }
        Event::AudioStarted { gen, at_ms } => {
            if gen == m.gen && is_recording(m.phase) {
                next.started_at_ms = Some(at_ms);
                if let Some(view) = recording_view(&next) {
                    set_pill(&mut next, &mut out, Some(view));
                }
                out.push(Action::Arm {
                    kind: TimerKind::Cap,
                    gen,
                    after: Duration::from_secs(HOTKEY_MAX_SECS),
                });
            }
        }
        Event::StartFailed { gen, message } => {
            // While Processing, the running job reports the failure itself.
            if gen == m.gen && is_recording(m.phase) {
                reset(&mut next);
                out.push(Action::ReleaseRecording);
                show_error(&mut next, message, &mut out);
            }
        }
        Event::JobFinished { gen, error } => {
            if gen == m.gen && m.phase == Phase::Processing {
                next.phase = Phase::Idle;
                next.started_at_ms = None;
                match error {
                    None => {
                        set_pill(&mut next, &mut out, None);
                        out.push(Action::Tray(TrayView::Clear));
                    }
                    Some(message) => show_error(&mut next, message, &mut out),
                }
            }
        }
        Event::Timer { kind, gen } => {
            if gen == m.gen {
                match (kind, m.phase) {
                    (TimerKind::Tap, Phase::PendingDouble) => {
                        reset(&mut next);
                        out.push(Action::ReleaseRecording);
                        out.push(Action::Tray(TrayView::Clear));
                        set_pill(&mut next, &mut out, Some(PillView::Hint));
                        out.push(Action::Arm {
                            kind: TimerKind::HidePill,
                            gen: next.gen,
                            after: HINT_SHOWN,
                        });
                    }
                    (TimerKind::Cap, Phase::Holding { .. } | Phase::Locked) => {
                        accept(&mut next, &mut out);
                    }
                    (TimerKind::HidePill, Phase::Idle) => set_pill(&mut next, &mut out, None),
                    (TimerKind::ClearTray, Phase::Idle) => out.push(Action::Tray(TrayView::Clear)),
                    _ => {}
                }
            }
        }
    }

    (next, out)
}

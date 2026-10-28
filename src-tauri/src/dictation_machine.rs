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

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn machine(phase: Phase, gen: u64) -> Machine {
        Machine {
            phase,
            gen,
            ..Machine::default()
        }
    }

    /// Idle machine after a press at `t0`, i.e. Holding with gen 1.
    fn holding(t0: Instant) -> Machine {
        step(&Machine::default(), Event::Pressed, t0).0
    }

    /// Machine in PendingDouble after a quick tap, gen 1.
    fn pending(t0: Instant) -> Machine {
        step(&holding(t0), Event::Released, t0 + ms(100)).0
    }

    fn locked(t0: Instant) -> Machine {
        step(&pending(t0), Event::Pressed, t0 + ms(200)).0
    }

    fn processing() -> Machine {
        Machine {
            phase: Phase::Processing,
            gen: 1,
            pill: Some(PillView::Processing),
            ..Machine::default()
        }
    }

    fn hint_idle(gen: u64) -> Machine {
        Machine {
            gen,
            pill: Some(PillView::Hint),
            ..Machine::default()
        }
    }

    fn assert_noop(m: &Machine, event: Event) {
        let (next, actions) = step(m, event.clone(), Instant::now());
        assert_eq!(&next, m, "state changed on {event:?}");
        assert!(actions.is_empty(), "actions on {event:?}: {actions:?}");
    }

    #[test]
    fn idle_press_starts_holding() {
        let t0 = Instant::now();
        let (next, actions) = step(&Machine::default(), Event::Pressed, t0);
        assert_eq!(next.phase, Phase::Holding { since: t0 });
        assert_eq!(next.gen, 1);
        assert_eq!(
            actions,
            vec![Action::Start { gen: 1 }, Action::Tray(TrayView::Recording)]
        );
    }

    #[test]
    fn holding_press_is_key_repeat_and_ignored() {
        let t0 = Instant::now();
        let m = holding(t0);
        let (next, actions) = step(&m, Event::Pressed, t0 + ms(50));
        assert_eq!(next, m);
        assert!(actions.is_empty());
    }

    #[test]
    fn long_hold_release_accepts_without_recapture() {
        let t0 = Instant::now();
        let (next, actions) = step(&holding(t0), Event::Released, t0 + TAP_WINDOW);
        assert_eq!(next.phase, Phase::Processing);
        assert_eq!(next.pill, Some(PillView::Processing));
        assert_eq!(
            actions,
            vec![
                Action::Accept { gen: 1, recapture: false },
                Action::SyncPill,
                Action::Tray(TrayView::Working),
            ]
        );
    }

    #[test]
    fn short_release_arms_tap_timer_from_release() {
        let t0 = Instant::now();
        let (next, actions) = step(&holding(t0), Event::Released, t0 + ms(499));
        assert_eq!(next.phase, Phase::PendingDouble);
        assert_eq!(next.gen, 1);
        assert_eq!(
            actions,
            vec![Action::Arm {
                kind: TimerKind::Tap,
                gen: 1,
                after: TAP_WINDOW
            }]
        );
    }

    #[test]
    fn tap_timer_fires_discards_and_shows_hint() {
        let t0 = Instant::now();
        let m = pending(t0);
        let (next, actions) = step(&m, Event::Timer { kind: TimerKind::Tap, gen: 1 }, t0 + ms(600));
        assert_eq!(next.phase, Phase::Idle);
        assert_eq!(next.gen, 2);
        assert_eq!(next.pill, Some(PillView::Hint));
        assert!(!next.was_locked);
        assert_eq!(
            actions,
            vec![
                Action::ReleaseRecording,
                Action::Tray(TrayView::Clear),
                Action::SyncPill,
                Action::Arm {
                    kind: TimerKind::HidePill,
                    gen: 2,
                    after: HINT_SHOWN
                },
            ]
        );
    }

    #[test]
    fn second_press_window_is_measured_from_release() {
        let t0 = Instant::now();
        // Released at 490 ms; the second press lands 300 ms later, i.e. 790 ms
        // after the first press but within the window from the release.
        let (pend, _) = step(&holding(t0), Event::Released, t0 + ms(490));
        let (next, _) = step(&pend, Event::Pressed, t0 + ms(790));
        assert_eq!(next.phase, Phase::Locked);
        assert!(next.was_locked);
        assert_eq!(next.gen, 1);
    }

    #[test]
    fn locked_release_is_ignored() {
        let t0 = Instant::now();
        let m = locked(t0);
        let (next, actions) = step(&m, Event::Released, t0 + ms(900));
        assert_eq!(next, m);
        assert!(actions.is_empty());
    }

    #[test]
    fn locked_press_accepts_with_recapture() {
        let t0 = Instant::now();
        let (next, actions) = step(&locked(t0), Event::Pressed, t0 + ms(5000));
        assert_eq!(next.phase, Phase::Processing);
        assert_eq!(
            actions,
            vec![
                Action::Accept { gen: 1, recapture: true },
                Action::SyncPill,
                Action::Tray(TrayView::Working),
            ]
        );
    }

    #[test]
    fn pending_press_before_audio_keeps_pill_untouched() {
        let t0 = Instant::now();
        let (next, actions) = step(&pending(t0), Event::Pressed, t0 + ms(200));
        assert_eq!(next.phase, Phase::Locked);
        assert!(actions.is_empty());
    }

    #[test]
    fn pending_press_after_audio_shows_locked_pill() {
        let t0 = Instant::now();
        let mut m = pending(t0);
        m.started_at_ms = Some(42);
        let (next, actions) = step(&m, Event::Pressed, t0 + ms(200));
        assert_eq!(next.pill, Some(PillView::Locked { started_at_ms: 42 }));
        assert_eq!(actions, vec![Action::SyncPill]);
    }

    #[test]
    fn pill_accept_in_recording_phases_accepts() {
        let t0 = Instant::now();
        for (m, recapture) in [
            (holding(t0), false),
            (pending(t0), false),
            (locked(t0), true),
        ] {
            let (next, actions) = step(&m, Event::PillAccept, t0 + ms(50));
            assert_eq!(next.phase, Phase::Processing, "from {:?}", m.phase);
            assert!(
                actions.contains(&Action::Accept { gen: 1, recapture }),
                "from {:?}: {actions:?}",
                m.phase
            );
        }
    }

    #[test]
    fn pill_accept_in_processing_and_idle_is_ignored() {
        assert_noop(&processing(), Event::PillAccept);
        assert_noop(&Machine::default(), Event::PillAccept);
    }

    #[test]
    fn pill_cancel_in_every_non_idle_phase_resets() {
        let t0 = Instant::now();
        for m in [holding(t0), pending(t0), locked(t0), processing()] {
            let (next, actions) = step(&m, Event::PillCancel, t0 + ms(50));
            assert_eq!(next.phase, Phase::Idle, "from {:?}", m.phase);
            assert_eq!(next.gen, m.gen + 1);
            assert_eq!(next.pill, None);
            assert_eq!(next.started_at_ms, None);
            assert!(!next.was_locked);
            assert_eq!(
                actions,
                vec![
                    Action::ReleaseRecording,
                    Action::SyncPill,
                    Action::Tray(TrayView::Clear)
                ]
            );
        }
    }

    #[test]
    fn pill_cancel_in_idle_is_noop() {
        assert_noop(&Machine::default(), Event::PillCancel);
        assert_noop(&hint_idle(3), Event::PillCancel);
    }

    #[test]
    fn external_cancel_hides_lingering_pill_and_bumps_gen() {
        for pill in [PillView::Hint, PillView::Error("x".into())] {
            let m = Machine {
                gen: 4,
                pill: Some(pill),
                ..Machine::default()
            };
            let (next, actions) = step(&m, Event::ExternalCancel, Instant::now());
            assert_eq!(next.phase, Phase::Idle);
            assert_eq!(next.gen, 5);
            assert_eq!(next.pill, None);
            assert_eq!(actions, vec![Action::SyncPill, Action::Tray(TrayView::Clear)]);
        }
    }

    #[test]
    fn external_cancel_idle_without_pill_is_noop() {
        assert_noop(&machine(Phase::Idle, 7), Event::ExternalCancel);
    }

    #[test]
    fn external_cancel_while_recording_cancels_session() {
        let t0 = Instant::now();
        let (next, actions) = step(&locked(t0), Event::ExternalCancel, t0 + ms(10));
        assert_eq!(next.phase, Phase::Idle);
        assert_eq!(next.gen, 2);
        assert!(actions.contains(&Action::ReleaseRecording));
    }

    #[test]
    fn hidden_pill_timer_after_external_cancel_is_stale() {
        let m = hint_idle(4);
        let (cancelled, _) = step(&m, Event::ExternalCancel, Instant::now());
        // The HidePill armed for gen 4 must now do nothing.
        assert_noop(&cancelled, Event::Timer { kind: TimerKind::HidePill, gen: 4 });
    }

    #[test]
    fn stale_generation_events_are_noops() {
        let t0 = Instant::now();
        let machines = [
            holding(t0),
            pending(t0),
            locked(t0),
            processing(),
            hint_idle(1),
        ];
        let old = 0;
        for m in machines {
            let events = [
                Event::Timer { kind: TimerKind::Tap, gen: old },
                Event::Timer { kind: TimerKind::Cap, gen: old },
                Event::Timer { kind: TimerKind::HidePill, gen: old },
                Event::Timer { kind: TimerKind::ClearTray, gen: old },
                Event::AudioStarted { gen: old, at_ms: 9 },
                Event::Captured { gen: old },
                Event::StartFailed { gen: old, message: "boom".into() },
                Event::JobFinished { gen: old, error: None },
                Event::JobFinished { gen: old, error: Some("e".into()) },
            ];
            for e in events {
                assert_noop(&m, e);
            }
        }
    }

    #[test]
    fn cap_timer_accepts_in_holding_and_locked() {
        let t0 = Instant::now();
        for (m, recapture) in [(holding(t0), false), (locked(t0), true)] {
            let (next, actions) = step(
                &m,
                Event::Timer { kind: TimerKind::Cap, gen: 1 },
                t0 + ms(1000),
            );
            assert_eq!(next.phase, Phase::Processing);
            assert!(actions.contains(&Action::Accept { gen: 1, recapture }));
        }
    }

    #[test]
    fn cap_timer_in_pending_and_processing_is_noop() {
        let t0 = Instant::now();
        assert_noop(&pending(t0), Event::Timer { kind: TimerKind::Cap, gen: 1 });
        assert_noop(&processing(), Event::Timer { kind: TimerKind::Cap, gen: 1 });
    }

    #[test]
    fn tap_timer_outside_pending_is_noop() {
        let t0 = Instant::now();
        assert_noop(&holding(t0), Event::Timer { kind: TimerKind::Tap, gen: 1 });
        assert_noop(&locked(t0), Event::Timer { kind: TimerKind::Tap, gen: 1 });
    }

    #[test]
    fn hide_pill_and_clear_tray_timers_in_idle() {
        let m = hint_idle(2);
        let (next, actions) = step(&m, Event::Timer { kind: TimerKind::HidePill, gen: 2 }, Instant::now());
        assert_eq!(next.pill, None);
        assert_eq!(actions, vec![Action::SyncPill]);

        let (next, actions) = step(&m, Event::Timer { kind: TimerKind::ClearTray, gen: 2 }, Instant::now());
        assert_eq!(next, m);
        assert_eq!(actions, vec![Action::Tray(TrayView::Clear)]);
    }

    #[test]
    fn hide_pill_timer_does_not_hide_a_live_session() {
        let t0 = Instant::now();
        assert_noop(&holding(t0), Event::Timer { kind: TimerKind::HidePill, gen: 1 });
    }

    #[test]
    fn captured_shows_starting_pill_until_audio_opens() {
        let t0 = Instant::now();
        let (next, actions) = step(&holding(t0), Event::Captured { gen: 1 }, t0);
        assert_eq!(next.pill, Some(PillView::Starting));
        assert_eq!(actions, vec![Action::SyncPill]);

        let mut started = holding(t0);
        started.started_at_ms = Some(5);
        assert_noop(&started, Event::Captured { gen: 1 });
        assert_noop(&processing(), Event::Captured { gen: 1 });
    }

    #[test]
    fn audio_started_sets_recording_pill_and_arms_cap() {
        let t0 = Instant::now();
        let (next, actions) = step(&holding(t0), Event::AudioStarted { gen: 1, at_ms: 1234 }, t0);
        assert_eq!(next.started_at_ms, Some(1234));
        assert_eq!(next.pill, Some(PillView::Recording { started_at_ms: 1234 }));
        assert_eq!(
            actions,
            vec![
                Action::SyncPill,
                Action::Arm {
                    kind: TimerKind::Cap,
                    gen: 1,
                    after: Duration::from_secs(HOTKEY_MAX_SECS)
                },
            ]
        );
    }

    #[test]
    fn audio_started_while_locked_uses_locked_look() {
        let t0 = Instant::now();
        let (next, _) = step(&locked(t0), Event::AudioStarted { gen: 1, at_ms: 77 }, t0);
        assert_eq!(next.pill, Some(PillView::Locked { started_at_ms: 77 }));
    }

    #[test]
    fn audio_started_in_processing_or_idle_is_noop() {
        assert_noop(&processing(), Event::AudioStarted { gen: 1, at_ms: 1 });
        assert_noop(&machine(Phase::Idle, 1), Event::AudioStarted { gen: 1, at_ms: 1 });
    }

    #[test]
    fn start_failed_goes_idle_with_error_pill() {
        let t0 = Instant::now();
        let (next, actions) = step(
            &holding(t0),
            Event::StartFailed { gen: 1, message: "no mic".into() },
            t0,
        );
        assert_eq!(next.phase, Phase::Idle);
        assert_eq!(next.gen, 2);
        assert_eq!(next.pill, Some(PillView::Error("no mic".into())));
        assert_eq!(
            actions,
            vec![
                Action::ReleaseRecording,
                Action::Tray(TrayView::Error("no mic".into())),
                Action::SyncPill,
                Action::Arm { kind: TimerKind::HidePill, gen: 2, after: ERROR_PILL_SHOWN },
                Action::Arm { kind: TimerKind::ClearTray, gen: 2, after: ERROR_TRAY_SHOWN },
            ]
        );
    }

    #[test]
    fn start_failed_while_processing_is_noop() {
        assert_noop(
            &processing(),
            Event::StartFailed { gen: 1, message: "x".into() },
        );
    }

    #[test]
    fn job_finished_ok_returns_to_idle_and_hides_pill() {
        let (next, actions) = step(
            &processing(),
            Event::JobFinished { gen: 1, error: None },
            Instant::now(),
        );
        assert_eq!(next.phase, Phase::Idle);
        assert_eq!(next.pill, None);
        assert_eq!(next.started_at_ms, None);
        assert_eq!(actions, vec![Action::SyncPill, Action::Tray(TrayView::Clear)]);
    }

    #[test]
    fn job_finished_err_shows_error_pill_and_arms_timers() {
        let (next, actions) = step(
            &processing(),
            Event::JobFinished { gen: 1, error: Some("api".into()) },
            Instant::now(),
        );
        assert_eq!(next.phase, Phase::Idle);
        assert_eq!(next.gen, 1);
        assert_eq!(next.pill, Some(PillView::Error("api".into())));
        assert_eq!(
            actions,
            vec![
                Action::Tray(TrayView::Error("api".into())),
                Action::SyncPill,
                Action::Arm { kind: TimerKind::HidePill, gen: 1, after: ERROR_PILL_SHOWN },
                Action::Arm { kind: TimerKind::ClearTray, gen: 1, after: ERROR_TRAY_SHOWN },
            ]
        );
    }

    #[test]
    fn job_finished_outside_processing_is_noop() {
        let t0 = Instant::now();
        assert_noop(&holding(t0), Event::JobFinished { gen: 1, error: None });
    }

    #[test]
    fn cancelled_job_result_is_dropped() {
        // Cancel during Processing bumps the gen, so the late result is stale.
        let (cancelled, _) = step(&processing(), Event::PillCancel, Instant::now());
        assert_noop(&cancelled, Event::JobFinished { gen: 1, error: Some("late".into()) });
    }

    #[test]
    fn new_session_from_idle_resets_was_locked() {
        let t0 = Instant::now();
        let (done, _) = step(
            &step(&locked(t0), Event::PillAccept, t0).0,
            Event::JobFinished { gen: 1, error: None },
            t0,
        );
        assert!(done.was_locked, "flag survives until the next session");
        let (next, _) = step(&done, Event::Pressed, t0 + ms(10));
        assert!(!next.was_locked);
        assert_eq!(next.started_at_ms, None);
        assert_eq!(next.gen, 2);

        // The new hold-release must not recapture.
        let (_, actions) = step(&next, Event::Released, t0 + ms(10) + TAP_WINDOW);
        assert!(actions.contains(&Action::Accept { gen: 2, recapture: false }));
    }
}

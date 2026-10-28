use std::sync::atomic::AtomicBool;
use std::time::Instant;
use std::sync::mpsc::Sender;
use std::sync::Mutex;

use tauri::menu::{CheckMenuItem, MenuItem};

use crate::audio::RecorderCmd;
use crate::dictation_hotkey::DictationRuntime;
use crate::focus::FocusedApp;

/// The tray's menu item handles, kept around so `save_ui_language` can
/// relabel them in place (see `tray::update_language`) instead of tearing
/// down and rebuilding the whole tray icon just to change three strings.
pub struct TrayMenuItems {
    pub translate: MenuItem<tauri::Wry>,
    pub settings: MenuItem<tauri::Wry>,
    pub quit: MenuItem<tauri::Wry>,
}

/// The tray's tone submenu checkboxes — `tray::update_tone` flips these
/// in place (and off each other) instead of rebuilding the menu.
pub struct TrayToneItems {
    pub professional: CheckMenuItem<tauri::Wry>,
    pub casual: CheckMenuItem<tauri::Wry>,
}

/// Result of the press-time task of a hold-to-talk job: the app that was
/// frontmost, or why the microphone did not open.
pub type HotkeyStart = tauri::async_runtime::JoinHandle<Result<Option<FocusedApp>, String>>;

/// Where the translation gets pasted once the user submits.
#[derive(Debug, Clone)]
pub enum CapturedTarget {
    /// Another app, with the moment it was captured so a stale one can be
    /// dropped (see `shortcut::next_target`).
    App { app: FocusedApp, at: Instant },
    /// The shortcut fired while TypeLang's own Settings window had focus: it
    /// is focused natively and pasted into, never through an Apple Event.
    SelfSettings,
}

/// Shared app state. Holds the target that was in front right before the
/// popup opened, so the translate command knows where to paste once the user
/// submits.
#[derive(Default)]
pub struct AppState {
    pub captured_focus: Mutex<Option<CapturedTarget>>,
    pub tray_items: Mutex<Option<TrayMenuItems>>,
    pub tray_tone_items: Mutex<Option<TrayToneItems>>,
    /// Process-group id of the local-services command TypeLang spawned (see
    /// `services.rs`). `None` when nothing was started by us — including when
    /// the services were already running — so quit never kills them.
    pub services_pgid: Mutex<Option<i32>>,
    /// Channel to the recorder thread that owns the microphone stream (see
    /// `audio.rs`); `None` until the first recording starts it.
    pub recorder: Mutex<Option<Sender<RecorderCmd>>>,
    /// True from the moment a recording is requested until it is stopped or
    /// cancelled; lets a cancel that lands while a start is in flight win.
    pub recording_wanted: AtomicBool,
    /// State of the dictation shortcut (machine, in-flight start task, pill
    /// contents) — see `dictation_hotkey.rs`.
    pub dictation: Mutex<DictationRuntime>,
}

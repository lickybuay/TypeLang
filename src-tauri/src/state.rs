use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::Instant;
use std::sync::mpsc::Sender;
use std::sync::Mutex;

use tauri::menu::{CheckMenuItem, MenuItem};

use crate::audio::RecorderCmd;
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

/// Shared app state. Holds the app that had focus right before the popup
/// opened, so the translate command knows where to paste once the user
/// submits.
#[derive(Default)]
pub struct AppState {
    pub captured_focus: Mutex<Option<FocusedApp>>,
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
    /// True while a hold-to-talk dictation job is in flight (recording,
    /// transcribing or pasting) — see `dictation_hotkey.rs`.
    pub hotkey_busy: AtomicBool,
    /// Id of the newest hotkey job; each press bumps it. A job whose id is no
    /// longer current was superseded and must not touch the tray or paste.
    pub hotkey_gen: AtomicU64,
    /// When the current hotkey job started; lets a press recover from a job
    /// whose release never arrived.
    pub hotkey_started: Mutex<Option<Instant>>,
    /// The press-time task (focus capture + recorder start) of the current
    /// hotkey job; the release awaits it so stop can never overtake start.
    pub hotkey_start: Mutex<Option<HotkeyStart>>,
}

use std::sync::Mutex;

use tauri::menu::{CheckMenuItem, MenuItem};

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

/// Shared app state. Holds the app that had focus right before the popup
/// opened, so the translate command knows where to paste once the user
/// submits.
#[derive(Default)]
pub struct AppState {
    pub captured_focus: Mutex<Option<FocusedApp>>,
    pub tray_items: Mutex<Option<TrayMenuItems>>,
    pub tray_tone_items: Mutex<Option<TrayToneItems>>,
}

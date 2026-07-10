use std::sync::Mutex;

use crate::focus::FocusedApp;

/// Shared app state. Holds the app that had focus right before the popup
/// opened, so the translate command knows where to paste once the user
/// submits.
#[derive(Default)]
pub struct AppState {
    pub captured_focus: Mutex<Option<FocusedApp>>,
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{capture, restore};

/// The app that had focus right before the popup was shown, so we can
/// hand focus back to it before pasting the translation.
#[derive(Debug, Clone)]
pub struct FocusedApp {
    pub name: String,
}

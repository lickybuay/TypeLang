#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{capture, restore};

/// The app that had focus right before the popup was shown, so we can
/// hand focus back to it before pasting the translation.
#[derive(Debug, Clone)]
pub struct FocusedApp {
    pub name: String,
    /// Stable across System Events' process names and `activate` names, so
    /// preferred over `name` whenever both sides have it.
    pub bundle_id: Option<String>,
}

impl FocusedApp {
    pub fn is_same(&self, other: &FocusedApp) -> bool {
        match (&self.bundle_id, &other.bundle_id) {
            (Some(a), Some(b)) => a == b,
            _ => self.name == other.name,
        }
    }
}

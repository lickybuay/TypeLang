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
    /// Unix process id. Unlike the bundle id it is also known for TypeLang
    /// itself under `tauri dev`, so it is what identifies "we are frontmost".
    pub pid: Option<i32>,
}

impl FocusedApp {
    pub fn is_same(&self, other: &FocusedApp) -> bool {
        match (&self.bundle_id, &other.bundle_id) {
            (Some(a), Some(b)) => a == b,
            _ => match (self.pid, other.pid) {
                (Some(a), Some(b)) => a == b,
                _ => self.name == other.name,
            },
        }
    }

    /// True when this is TypeLang's own process. Any Apple Event aimed at it
    /// from the main thread would deadlock, so callers must never restore it.
    pub fn is_self(&self) -> bool {
        self.pid == Some(std::process::id() as i32)
    }

    /// False only when the pid is known and that process no longer exists.
    /// Activating a bundle id whose app has quit would launch it again, and
    /// the text would land in a freshly started app, so callers check this
    /// before restoring. Without a pid there is nothing to check.
    pub fn is_alive(&self) -> bool {
        let Some(pid) = self.pid else {
            return true;
        };
        std::process::Command::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(pid: Option<i32>) -> FocusedApp {
        FocusedApp {
            name: "App".to_string(),
            bundle_id: None,
            pid,
        }
    }

    #[test]
    fn is_self_true_for_own_pid() {
        assert!(app(Some(std::process::id() as i32)).is_self());
    }

    #[test]
    fn is_alive_true_for_own_pid_and_when_pid_unknown() {
        assert!(app(Some(std::process::id() as i32)).is_alive());
        assert!(app(None).is_alive());
    }

    #[test]
    fn is_alive_false_for_a_pid_that_does_not_exist() {
        assert!(!app(Some(i32::MAX)).is_alive());
    }

    #[test]
    fn is_self_false_for_other_or_missing_pid() {
        assert!(!app(Some(std::process::id() as i32 + 1)).is_self());
        assert!(!app(None).is_self());
    }
}

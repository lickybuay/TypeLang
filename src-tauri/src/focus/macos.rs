use super::FocusedApp;

/// Captures the bundle id and name of the frontmost app via System Events.
/// This must be called BEFORE the popup window is shown, otherwise the popup
/// itself becomes the frontmost app and we lose the real target.
pub fn capture() -> Result<FocusedApp, String> {
    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to set p to first application process whose frontmost is true",
            "-e",
            "tell application \"System Events\" to return (bundle identifier of p as text) & linefeed & (name of p)",
        ])
        .output()
        .map_err(|e| format!("failed to run osascript: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let (bundle_id, name) = stdout.split_once('\n').unwrap_or(("", stdout.as_ref()));
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("osascript returned an empty app name".to_string());
    }
    let bundle_id = Some(bundle_id.trim())
        .filter(|b| !b.is_empty() && *b != "missing value")
        .map(str::to_string);
    Ok(FocusedApp { name, bundle_id })
}

/// Reactivates the previously captured app so the simulated paste keystroke
/// lands in the right place instead of in the popup/settings window. Checks
/// that the app really came to the front and retries once if it did not.
pub fn restore(app: &FocusedApp) -> Result<(), String> {
    activate(app)?;
    if !is_frontmost(app) {
        activate(app)?;
        std::thread::sleep(std::time::Duration::from_millis(150));
        if !is_frontmost(app) {
            return Err(format!("No se pudo volver a {}", app.name));
        }
    }
    Ok(())
}

fn is_frontmost(app: &FocusedApp) -> bool {
    capture().is_ok_and(|f| f.is_same(app))
}

/// Escapes `\` and `"` so an app name can sit inside an AppleScript string.
fn escape_applescript(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn activate(app: &FocusedApp) -> Result<(), String> {
    let script = match &app.bundle_id {
        Some(id) => format!(
            "tell application id \"{}\" to activate",
            escape_applescript(id)
        ),
        None => format!(
            "tell application \"{}\" to activate",
            escape_applescript(&app.name)
        ),
    };
    let status = std::process::Command::new("osascript")
        .args(["-e", &script])
        .status()
        .map_err(|e| format!("failed to run osascript: {e}"))?;

    if !status.success() {
        return Err(format!("osascript failed to activate {}", app.name));
    }
    Ok(())
}

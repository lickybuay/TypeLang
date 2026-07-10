use super::FocusedApp;

/// Captures the name of the frontmost app via System Events. This must be
/// called BEFORE the popup window is shown, otherwise the popup itself
/// becomes the frontmost app and we lose the real target.
pub fn capture() -> Result<FocusedApp, String> {
    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to get name of first application process whose frontmost is true",
        ])
        .output()
        .map_err(|e| format!("failed to run osascript: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }

    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() {
        return Err("osascript returned an empty app name".to_string());
    }
    Ok(FocusedApp { name })
}

/// Reactivates the previously captured app so the simulated paste keystroke
/// lands in the right place instead of in the popup/settings window.
pub fn restore(app: &FocusedApp) -> Result<(), String> {
    let script = format!("tell application \"{}\" to activate", app.name);
    let status = std::process::Command::new("osascript")
        .args(["-e", &script])
        .status()
        .map_err(|e| format!("failed to run osascript: {e}"))?;

    if !status.success() {
        return Err(format!("osascript failed to activate {}", app.name));
    }
    Ok(())
}

use super::FocusedApp;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);
const ACTIVATE_TIMEOUT: Duration = Duration::from_secs(3);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Runs `osascript` and kills it if it outlives `timeout`: a hung Apple Event
/// would otherwise block the calling thread (often the main one) for as long
/// as macOS's own Apple Event timeout, 120 s.
fn run_osascript(args: &[&str], timeout: Duration) -> Result<Output, String> {
    let mut child = Command::new("osascript")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run osascript: {e}"))?;

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|e| format!("failed to read osascript output: {e}"));
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("osascript timed out after {}s", timeout.as_secs()));
            }
            Ok(None) => std::thread::sleep(POLL_INTERVAL),
            Err(e) => return Err(format!("failed to wait for osascript: {e}")),
        }
    }
}

/// Captures the bundle id, name and pid of the frontmost app via System Events.
/// This must be called BEFORE the popup window is shown, otherwise the popup
/// itself becomes the frontmost app and we lose the real target.
pub fn capture() -> Result<FocusedApp, String> {
    let output = run_osascript(
        &[
            "-e",
            "tell application \"System Events\" to set p to first application process whose frontmost is true",
            "-e",
            "tell application \"System Events\" to return (bundle identifier of p as text) & linefeed & (name of p) & linefeed & (unix id of p as text)",
        ],
        CAPTURE_TIMEOUT,
    )?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.splitn(3, '\n');
    let bundle_id = lines.next().unwrap_or("").trim();
    let name = lines.next().unwrap_or("").trim().to_string();
    let pid = lines.next().and_then(|p| p.trim().parse::<i32>().ok());
    if name.is_empty() {
        return Err("osascript returned an empty app name".to_string());
    }
    let bundle_id = Some(bundle_id)
        .filter(|b| !b.is_empty() && *b != "missing value")
        .map(str::to_string);
    Ok(FocusedApp { name, bundle_id, pid })
}

/// Reactivates the previously captured app so the simulated paste keystroke
/// lands in the right place instead of in the popup/settings window. Checks
/// that the app really came to the front and retries once if it did not.
/// Never targets TypeLang itself: an Apple Event to our own process from the
/// main thread deadlocks, so callers handle a self target before getting here.
pub fn restore(app: &FocusedApp) -> Result<(), String> {
    if app.is_self() {
        return Err("TypeLang no puede restaurar el foco a sí misma".to_string());
    }
    if !app.is_alive() {
        return Err(format!("{} ya no está abierta", app.name));
    }
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
    let output = run_osascript(&["-e", &script], ACTIVATE_TIMEOUT)?;

    if !output.status.success() {
        return Err(format!("osascript failed to activate {}", app.name));
    }
    Ok(())
}

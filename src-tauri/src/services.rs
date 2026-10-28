//! Optional autostart of the user's local AI services (whisper + llama-server).
//!
//! The configured command is expected to stay in the foreground until killed.
//! We run it through a login shell in its own process group, remember that
//! group id, and signal the whole group on quit so the script's children go
//! down with it. If the services already answer when we start, they're not
//! ours: we neither spawn a second copy nor touch them on quit.

use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::{settings_store, state::AppState};

const HEALTH_TIMEOUT: Duration = Duration::from_millis(1500);

/// Same `/v1` normalization the local provider applies to the configured URL
/// (see `llm::lmstudio`), so the probe hits the endpoint translations use.
fn models_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{trimmed}/models")
    } else {
        format!("{trimmed}/v1/models")
    }
}

async fn already_running(base_url: &str) -> bool {
    let Ok(client) = reqwest::Client::builder().timeout(HEALTH_TIMEOUT).build() else {
        return false;
    };
    match client.get(models_url(base_url)).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Starts the configured command in the background if autostart is enabled,
/// nothing of ours is running yet, and the services aren't already up.
/// Never blocks the caller: the health check and spawn run on the async
/// runtime.
pub fn start_if_configured(app: &AppHandle) {
    let settings = match settings_store::load(app) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[services] could not load settings: {e}");
            return;
        }
    };
    let command = settings.autostart_command.trim().to_string();
    if !settings.autostart_enabled || command.is_empty() {
        return;
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if app.state::<AppState>().services_pgid.lock().unwrap().is_some() {
            return;
        }
        if already_running(&settings.lmstudio_base_url).await {
            println!("[services] already running, leaving them alone");
            return;
        }

        // Held across the spawn so two concurrent starts (launch + a quick
        // Settings toggle) can't both launch a copy.
        let state = app.state::<AppState>();
        let mut pgid = state.services_pgid.lock().unwrap();
        if pgid.is_some() {
            return;
        }
        // The health check took up to 1.5 s: the user may have toggled
        // autostart off (or edited the command) meanwhile, and `stop` found
        // nothing to stop. Re-read settings so that doesn't get overridden.
        let command = match settings_store::load(&app) {
            Ok(s) if s.autostart_enabled && !s.autostart_command.trim().is_empty() => {
                s.autostart_command.trim().to_string()
            }
            Ok(_) => return,
            Err(e) => {
                eprintln!("[services] could not reload settings: {e}");
                return;
            }
        };
        match spawn(&app, &command) {
            Ok(child) => {
                let id = child.id() as i32;
                println!("[services] started `{command}` (pgid {id})");
                *pgid = Some(id);
                // Started while the lock is still held: the reaper can't
                // clear the pgid before it's been stored.
                reap(app.clone(), child, id);
            }
            Err(e) => eprintln!("[services] failed to start: {e}"),
        }
    });
}

/// Waits for the child on a dedicated thread so a script that exits on its
/// own is reaped (no zombie) and its pgid is forgotten — otherwise `stop`
/// could later signal a pgid the OS has recycled for an unrelated process.
/// Clears the stored id only if it's still this child's, so a newer start
/// isn't wiped; when it does clear it, it also terminates any group members
/// that outlived the script.
fn reap(app: AppHandle, mut child: std::process::Child, pgid: i32) {
    std::thread::spawn(move || {
        let status = child.wait();
        match status {
            Ok(status) => println!("[services] exited with {status} (pgid {pgid})"),
            Err(e) => eprintln!("[services] wait failed: {e} (pgid {pgid})"),
        }
        let state = app.state::<AppState>();
        let mut stored = state.services_pgid.lock().unwrap();
        if *stored == Some(pgid) {
            // The leader is gone but whisper/llama-server may have outlived
            // it; sweep the group (usually "no such process") under the same
            // lock so `stop` can't signal it at the same time.
            kill_group(pgid);
            *stored = None;
        }
    });
}

/// Stops the services TypeLang started, if any. A no-op when we don't own a
/// process group, so services launched by hand are never touched.
pub fn stop(app: &AppHandle) {
    let Some(pgid) = app.state::<AppState>().services_pgid.lock().unwrap().take() else {
        return;
    };
    kill_group(pgid);
}

/// `kill -TERM -<pgid>` signals the whole group: the script plus the servers
/// it launched.
#[cfg(unix)]
fn kill_group(pgid: i32) {
    match std::process::Command::new("/bin/kill")
        .args(["-TERM", &format!("-{pgid}")])
        .status()
    {
        Ok(status) if status.success() => println!("[services] stopped (pgid {pgid})"),
        Ok(status) => eprintln!("[services] kill exited with {status} (pgid {pgid})"),
        Err(e) => eprintln!("[services] could not run kill: {e}"),
    }
}

#[cfg(not(unix))]
fn kill_group(_pgid: i32) {}

/// Runs `command` in a login shell as the leader of a new process group (so
/// its pgid equals the child's pid) with output appended to a log file.
#[cfg(unix)]
fn spawn(app: &AppHandle, command: &str) -> Result<std::process::Child, String> {
    use std::fs::OpenOptions;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let home = app.path().home_dir().map_err(|e| e.to_string())?;
    let log_path = home.join("Library/Logs/typelang-services.log");
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("{}: {e}", log_path.display()))?;
    let log_err = log.try_clone().map_err(|e| e.to_string())?;

    Command::new("/bin/zsh")
        .arg("-lc")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_err)
        .process_group(0)
        .spawn()
        .map_err(|e| e.to_string())
}

#[cfg(not(unix))]
fn spawn(_app: &AppHandle, _command: &str) -> Result<std::process::Child, String> {
    Err("autostart is only supported on macOS".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_url_appends_v1_when_missing() {
        assert_eq!(models_url("http://localhost:8091"), "http://localhost:8091/v1/models");
    }

    #[test]
    fn models_url_strips_trailing_slash() {
        assert_eq!(models_url("http://localhost:8091/"), "http://localhost:8091/v1/models");
        assert_eq!(models_url("http://localhost:8091///"), "http://localhost:8091/v1/models");
    }

    #[test]
    fn models_url_keeps_existing_v1() {
        assert_eq!(models_url("http://127.0.0.1:8091/v1"), "http://127.0.0.1:8091/v1/models");
        assert_eq!(models_url("http://127.0.0.1:8091/v1/"), "http://127.0.0.1:8091/v1/models");
    }

    #[test]
    fn models_url_trims_whitespace() {
        assert_eq!(
            models_url("  http://127.0.0.1:8091/v1 \n"),
            "http://127.0.0.1:8091/v1/models"
        );
        assert_eq!(models_url(" http://localhost:8091/ "), "http://localhost:8091/v1/models");
    }

    #[cfg(unix)]
    #[test]
    fn kill_group_terminates_the_group_leader() {
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};
        use std::time::Instant;

        let mut child = Command::new("/bin/zsh")
            .args(["-c", "sleep 30 & sleep 30"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .expect("spawn zsh");
        let pgid = child.id() as i32;

        kill_group(pgid);

        let deadline = Instant::now() + Duration::from_secs(3);
        let exited = loop {
            if child.try_wait().expect("try_wait").is_some() {
                break true;
            }
            if Instant::now() >= deadline {
                break false;
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        if !exited {
            let _ = child.kill();
            let _ = child.wait();
        }
        assert!(exited, "process group leader still alive after SIGTERM to its group");
    }
}

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::settings::{self, Settings};

pub const AUTOSTART_ARG: &str = "--shardx-autostart";

#[derive(Debug, Clone, Serialize)]
pub struct StartupStatus {
    pub supported: bool,
    pub configured: bool,
    pub registered: bool,
    pub matches_configuration: bool,
    pub start_minimized: bool,
    pub launched_for_autostart: bool,
    pub api_enabled: bool,
    pub api_mode: &'static str,
    pub mcp_mode: &'static str,
}

pub fn args_request_autostart<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().any(|arg| arg.as_ref() == AUTOSTART_ARG)
}

pub fn launched_for_autostart() -> bool {
    std::env::args().any(|arg| arg == AUTOSTART_ARG)
}

fn registration_enabled(app: &AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

fn set_registration(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        // `enable` rewrites the executable path, which keeps the entry correct
        // after an in-place update or when a portable build has moved.
        manager.enable().map_err(|e| e.to_string())?;
        if !manager.is_enabled().map_err(|e| e.to_string())? {
            return Err("the operating system did not enable the startup entry".into());
        }
    } else {
        // A Windows user can disable an existing Run entry in Task Manager.
        // In that state `is_enabled` is false although the entry still exists,
        // so attempt removal and accept a missing-entry error only when the
        // final observed state is disabled.
        if let Err(error) = manager.disable() {
            if manager.is_enabled().map_err(|e| e.to_string())? {
                return Err(error.to_string());
            }
        }
        if manager.is_enabled().map_err(|e| e.to_string())? {
            return Err("the operating system did not disable the startup entry".into());
        }
    }
    Ok(())
}

pub fn status(app: &AppHandle) -> Result<StartupStatus, String> {
    let configured = settings::load().map_err(|e| e.to_string())?;
    let registered = registration_enabled(app)?;
    Ok(StartupStatus {
        supported: true,
        configured: configured.launch_at_login,
        registered,
        matches_configuration: configured.launch_at_login == registered,
        start_minimized: configured.start_minimized,
        launched_for_autostart: launched_for_autostart(),
        api_enabled: configured.api_enabled,
        api_mode: "launcher_embedded",
        mcp_mode: "client_spawned",
    })
}

/// Persist all Launcher settings and apply an explicit launch-at-login change
/// transactionally. Unrelated Settings saves do not override a user disabling
/// the entry in the operating system's own startup UI.
pub fn save(app: &AppHandle, next: &Settings) -> Result<(), String> {
    let previous = settings::load().map_err(|e| e.to_string())?;
    let registration_changed = previous.launch_at_login != next.launch_at_login;
    let previous_registered = if registration_changed {
        Some(registration_enabled(app)?)
    } else {
        None
    };

    if registration_changed {
        set_registration(app, next.launch_at_login)?;
    }

    if let Err(error) = settings::save(next) {
        if let Some(was_registered) = previous_registered {
            let _ = set_registration(app, was_registered);
        }
        return Err(error.to_string());
    }

    Ok(())
}

/// Apply an explicit startup configuration request even when the saved value
/// already matches. This reconciles an entry changed in the OS startup UI.
pub fn configure(app: &AppHandle, next: &Settings) -> Result<(), String> {
    let previous_registered = registration_enabled(app)?;
    set_registration(app, next.launch_at_login)?;

    if let Err(error) = settings::save(next) {
        let _ = set_registration(app, previous_registered);
        return Err(error.to_string());
    }

    Ok(())
}

/// Refresh an already-enabled entry with the current executable path. If the
/// OS reports the entry disabled (for example via Task Manager), respect that
/// external choice rather than silently re-enabling it.
pub fn refresh_registration_path(app: &AppHandle, configured: &Settings) -> Result<(), String> {
    if configured.launch_at_login && registration_enabled(app)? {
        set_registration(app, true)?;
    }
    Ok(())
}

pub fn should_start_hidden(configured: &Settings, autostart_launch: bool) -> bool {
    autostart_launch && configured.launch_at_login && configured.start_minimized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_only_the_dedicated_autostart_argument() {
        assert!(args_request_autostart(["launcher.exe", AUTOSTART_ARG]));
        assert!(!args_request_autostart(["launcher.exe", "--background"]));
        assert!(!args_request_autostart([
            "launcher.exe",
            "--shardx-autostart-extra"
        ]));
    }

    #[test]
    fn hides_only_an_enabled_minimized_autostart_launch() {
        let mut configured = Settings::default();
        configured.launch_at_login = true;
        configured.start_minimized = true;
        assert!(should_start_hidden(&configured, true));

        configured.start_minimized = false;
        assert!(!should_start_hidden(&configured, true));
        configured.start_minimized = true;
        configured.launch_at_login = false;
        assert!(!should_start_hidden(&configured, true));
        assert!(!should_start_hidden(&configured, false));
    }
}

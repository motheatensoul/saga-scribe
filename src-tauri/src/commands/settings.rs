use crate::settings::{Settings, SettingsManager};
use tauri::AppHandle;

#[tauri::command]
pub fn load_settings(app: AppHandle) -> Result<Settings, String> {
    let manager = SettingsManager::new(&app)?;
    Ok(manager.load())
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    let manager = SettingsManager::new(&app)?;
    manager.save(&settings)
}

#[tauri::command]
pub fn get_system_theme() -> String {
    // Try to detect system theme preference

    #[cfg(target_os = "linux")]
    {
        use log::debug;
        use std::process::Command;

        // Try xdg-desktop-portal (standard, DE-agnostic way)
        // https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Settings.html

        // Try busctl first (more common, comes with systemd)
        if let Ok(output) = Command::new("busctl")
            .args([
                "--user",
                "call",
                "org.freedesktop.portal.Desktop",
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.Settings",
                "Read",
                "ss",
                "org.freedesktop.appearance",
                "color-scheme",
            ])
            .output()
        {
            if output.status.success() {
                let result = String::from_utf8_lossy(&output.stdout);
                // Response format: "v u X" where X is 0 (no pref), 1 (dark), 2 (light)
                if result.contains("u 1") || result.contains("1") {
                    debug!("Detected dark theme via xdg-desktop-portal (busctl)");
                    return "dark".to_string();
                } else if result.contains("u 2") || result.contains("2") {
                    debug!("Detected light theme via xdg-desktop-portal (busctl)");
                    return "light".to_string();
                }
            }
        } else {
            debug!("busctl command failed, trying dbus-send");
        }

        // Fallback: Try dbus-send
        if let Ok(output) = Command::new("dbus-send")
            .args([
                "--session",
                "--print-reply=literal",
                "--dest=org.freedesktop.portal.Desktop",
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.Settings.Read",
                "string:org.freedesktop.appearance",
                "string:color-scheme",
            ])
            .output()
        {
            if output.status.success() {
                let result = String::from_utf8_lossy(&output.stdout);
                // Response format: "variant uint32 X" where X is 0 (no pref), 1 (dark), 2 (light)
                if result.contains("uint32 1") {
                    debug!("Detected dark theme via xdg-desktop-portal (dbus-send)");
                    return "dark".to_string();
                } else if result.contains("uint32 2") {
                    debug!("Detected light theme via xdg-desktop-portal (dbus-send)");
                    return "light".to_string();
                }
            }
        } else {
            debug!("dbus-send command failed, trying gsettings");
        }

        // Fallback: Try gsettings for GNOME users (legacy)
        if let Ok(output) = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .output()
        {
            if output.status.success() {
                let theme = String::from_utf8_lossy(&output.stdout);
                let theme = theme.trim().trim_matches('\'');
                if theme.contains("dark") {
                    debug!("Detected dark theme via gsettings color-scheme");
                    return "dark".to_string();
                } else if theme.contains("light") || !theme.is_empty() {
                    debug!("Detected light theme via gsettings color-scheme");
                    return "light".to_string();
                }
            }
        } else {
            debug!("gsettings color-scheme query failed, trying gtk-theme");
        }

        // Last resort: Try gtk-theme-name
        if let Ok(output) = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
            .output()
        {
            if output.status.success() {
                let theme = String::from_utf8_lossy(&output.stdout);
                let theme = theme.trim().trim_matches('\'').to_lowercase();
                if theme.contains("dark") {
                    debug!("Detected dark theme via gsettings gtk-theme");
                    return "dark".to_string();
                }
            }
        } else {
            debug!("gsettings gtk-theme query failed");
        }

        debug!("All Linux theme detection methods failed, defaulting to light");
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        if let Ok(output) = Command::new("defaults")
            .args(&["read", "-g", "AppleInterfaceStyle"])
            .output()
        {
            if output.status.success() {
                let theme = String::from_utf8_lossy(&output.stdout);
                if theme.trim().eq_ignore_ascii_case("dark") {
                    return "dark".to_string();
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows Registry check for dark mode
        // HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme
        use std::process::Command;

        if let Ok(output) = Command::new("reg")
            .args(&[
                "query",
                "HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
                "/v",
                "AppsUseLightTheme",
            ])
            .output()
        {
            if output.status.success() {
                let result = String::from_utf8_lossy(&output.stdout);
                // If AppsUseLightTheme is 0x0, dark mode is enabled
                if result.contains("0x0") {
                    return "dark".to_string();
                }
            }
        }
    }

    // Default to light if detection fails
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        use log::debug;
        debug!("Unknown platform, defaulting to light theme");
    }

    "light".to_string()
}

#[tauri::command]
pub async fn set_window_theme(app: tauri::AppHandle, theme: String) -> Result<(), String> {
    use tauri::Manager;

    // Get the main window
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Failed to get main window".to_string())?;

    // Determine the theme to apply
    let effective_theme = if theme == "system" {
        // Detect system theme
        get_system_theme()
    } else {
        theme
    };

    // Map to Tauri theme enum
    let window_theme = if effective_theme == "dark" {
        tauri::Theme::Dark
    } else {
        tauri::Theme::Light
    };

    // Set the window theme (this updates decorations on Linux/GNOME)
    window
        .set_theme(Some(window_theme))
        .map_err(|e| format!("Failed to set window theme: {}", e))?;

    Ok(())
}

//! System notifications for long-running work (imports, exports) that finishes while
//! the window is in the background.
//!
//! Each platform's own service, through crates Tauri already builds on: the
//! freedesktop notification service over D-Bus on Linux (the Flatpak may talk to
//! `org.freedesktop.Notifications`), toast notifications on Windows and the
//! UserNotifications framework on macOS. A notification that can't be shown is only
//! logged: the in-app log has the same message.

use tauri::Manager;

/// Shows a system notification, unless the window has focus (the user sees the
/// in-app message then).
#[tauri::command(async)]
pub async fn notify(window: tauri::WebviewWindow, title: String, body: String) -> Result<(), String> {
    if window.is_focused().unwrap_or(true) {
        return Ok(());
    }
    let app_id = window.config().identifier.clone();
    let shown =
        tauri::async_runtime::spawn_blocking(move || platform::show(&app_id, &title, &body)).await;
    match shown {
        Ok(Ok(())) => {}
        Ok(Err(e)) => log::warn!("Could not show a system notification: {e}"),
        Err(e) => log::warn!("Could not show a system notification: {e}"),
    }
    Ok(())
}

#[cfg(target_os = "linux")]
mod platform {
    use dbus::arg::{PropMap, RefArg, Variant};
    use dbus::blocking::Connection;
    use std::time::Duration;

    pub fn show(_app_id: &str, title: &str, body: &str) -> Result<(), String> {
        // Inside the Flatpak, icon and desktop file are named after the app ID;
        // the other Linux packages name them after the binary.
        let name = std::env::var("FLATPAK_ID").unwrap_or_else(|_| env!("CARGO_PKG_NAME").into());
        let conn = Connection::new_session().map_err(|e| e.to_string())?;
        let proxy = conn.with_proxy(
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            Duration::from_secs(5),
        );
        let mut hints = PropMap::new();
        hints.insert(
            "desktop-entry".into(),
            Variant(Box::new(name.clone()) as Box<dyn RefArg>),
        );
        let actions: Vec<&str> = Vec::new();
        let (_id,): (u32,) = proxy
            .method_call(
                "org.freedesktop.Notifications",
                "Notify",
                (
                    "Saga Scribe",
                    0u32, // replaces no earlier notification
                    name.as_str(),
                    title,
                    body,
                    actions,
                    hints,
                    -1i32, // the server's default timeout
                ),
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(windows)]
mod platform {
    use windows::core::HSTRING;
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

    /// PowerShell's app ID, for builds run from `target/` (`tauri dev`): toasts need
    /// an app ID with a Start menu shortcut, which only the installers create.
    const DEV_APP_ID: &str =
        r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe";

    pub fn show(app_id: &str, title: &str, body: &str) -> Result<(), String> {
        let app_id = if running_from_target_dir() { DEV_APP_ID } else { app_id };
        let xml = format!(
            r#"<toast><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual></toast>"#,
            escape(title),
            escape(body)
        );
        let show = || -> windows::core::Result<()> {
            let doc = XmlDocument::new()?;
            doc.LoadXml(&HSTRING::from(xml))?;
            let toast = ToastNotification::CreateToastNotification(&doc)?;
            ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_id))?
                .Show(&toast)
        };
        show().map_err(|e| e.to_string())
    }

    fn running_from_target_dir() -> bool {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.to_path_buf()))
            .is_some_and(|dir| {
                (dir.ends_with("debug") || dir.ends_with("release"))
                    && dir.parent().is_some_and(|p| p.ends_with("target"))
            })
    }

    fn escape(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSBundle, NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
        UNUserNotificationCenter,
    };

    pub fn show(_app_id: &str, title: &str, body: &str) -> Result<(), String> {
        // The notification center raises an exception outside an app bundle
        // (`tauri dev` runs the bare binary).
        if NSBundle::mainBundle().bundleIdentifier().is_none() {
            return Err("not running from an app bundle".into());
        }
        let (title, body) = (title.to_owned(), body.to_owned());
        // The first time, macOS asks the user; afterwards this answers at once.
        let then_show = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
            if !granted.as_bool() {
                return;
            }
            let content = UNMutableNotificationContent::new();
            content.setTitle(&NSString::from_str(&title));
            content.setBody(&NSString::from_str(&body));
            let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
                &NSString::from_str("saga-scribe.task"),
                &content,
                None,
            );
            UNUserNotificationCenter::currentNotificationCenter()
                .addNotificationRequest_withCompletionHandler(&request, None);
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .requestAuthorizationWithOptions_completionHandler(
                UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
                &then_show,
            );
        Ok(())
    }
}

#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
mod platform {
    pub fn show(_app_id: &str, _title: &str, _body: &str) -> Result<(), String> {
        Err("not supported on this platform".into())
    }
}

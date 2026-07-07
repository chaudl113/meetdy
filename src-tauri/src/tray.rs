use crate::settings;
use crate::tray_i18n::get_tray_translations;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIcon;
use tauri::{AppHandle, Manager, Theme};

#[derive(Clone, Debug, PartialEq)]
pub enum TrayIconState {
    Idle,
    Recording,
    Transcribing,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AppTheme {
    Dark,
    Light,
    Colored, // Pink/colored theme for Linux
}

/// Gets the current app theme, with Linux defaulting to Colored theme
pub fn get_current_theme(app: &AppHandle) -> AppTheme {
    if cfg!(target_os = "linux") {
        // On Linux, always use the colored theme
        AppTheme::Colored
    } else {
        // On other platforms, map system theme to our app theme
        if let Some(main_window) = app.get_webview_window("main") {
            match main_window.theme().unwrap_or(Theme::Dark) {
                Theme::Light => AppTheme::Light,
                Theme::Dark => AppTheme::Dark,
                _ => AppTheme::Dark, // Default fallback
            }
        } else {
            AppTheme::Dark
        }
    }
}

/// Gets the appropriate icon path for the given theme and state
pub fn get_icon_path(theme: AppTheme, state: TrayIconState) -> &'static str {
    match (theme, state) {
        // Dark theme uses light icons
        (AppTheme::Dark, TrayIconState::Idle) => "resources/tray_idle.png",
        (AppTheme::Dark, TrayIconState::Recording) => "resources/tray_recording.png",
        (AppTheme::Dark, TrayIconState::Transcribing) => "resources/tray_transcribing.png",
        // Light theme uses dark icons
        (AppTheme::Light, TrayIconState::Idle) => "resources/tray_idle_dark.png",
        (AppTheme::Light, TrayIconState::Recording) => "resources/tray_recording_dark.png",
        (AppTheme::Light, TrayIconState::Transcribing) => "resources/tray_transcribing_dark.png",
        // Colored theme uses pink icons (for Linux)
        (AppTheme::Colored, TrayIconState::Idle) => "resources/meetdy.png",
        (AppTheme::Colored, TrayIconState::Recording) => "resources/recording.png",
        (AppTheme::Colored, TrayIconState::Transcribing) => "resources/transcribing.png",
    }
}

pub fn change_tray_icon(app: &AppHandle, icon: TrayIconState) {
    let tray = app.state::<TrayIcon>();
    let theme = get_current_theme(app);

    let icon_path = get_icon_path(theme, icon.clone());

    let _ = (|| -> Option<()> {
        let path = app
            .path()
            .resolve(icon_path, tauri::path::BaseDirectory::Resource)
            .ok()?;
        let image = Image::from_path(path).ok()?;
        tray.set_icon(Some(image)).ok()
    })();

    // Update menu based on state
    update_tray_menu(app, &icon, None);
}

pub fn update_tray_menu(app: &AppHandle, state: &TrayIconState, locale: Option<&str>) {
    let settings = settings::get_settings(app);

    let locale = locale.unwrap_or(&settings.app_language);
    let strings = get_tray_translations(Some(locale.to_string()));

    // Platform-specific accelerators
    #[cfg(target_os = "macos")]
    let (settings_accelerator, quit_accelerator) = (Some("Cmd+,"), Some("Cmd+Q"));
    #[cfg(not(target_os = "macos"))]
    let (settings_accelerator, quit_accelerator) = (Some("Ctrl+,"), Some("Ctrl+Q"));

    // Create common menu items — use let … else to bail early on any failure
    let version_label = if cfg!(debug_assertions) {
        format!("Meetdy v{} (Dev)", env!("CARGO_PKG_VERSION"))
    } else {
        format!("Meetdy v{}", env!("CARGO_PKG_VERSION"))
    };
    let Ok(version_i) = MenuItem::with_id(app, "version", &version_label, false, None::<&str>)
    else {
        return;
    };
    let Ok(settings_i) = MenuItem::with_id(
        app,
        "settings",
        &strings.settings,
        true,
        settings_accelerator,
    ) else {
        return;
    };
    let Ok(check_updates_i) = MenuItem::with_id(
        app,
        "check_updates",
        &strings.check_updates,
        settings.update_checks_enabled,
        None::<&str>,
    ) else {
        return;
    };
    let Ok(quit_i) = MenuItem::with_id(app, "quit", &strings.quit, true, quit_accelerator) else {
        return;
    };
    let separator = || {
        let Ok(sep) = PredefinedMenuItem::separator(app) else {
            return None;
        };
        Some(sep)
    };

    let menu = match state {
        TrayIconState::Recording | TrayIconState::Transcribing => {
            let Ok(cancel_i) =
                MenuItem::with_id(app, "cancel", &strings.cancel, true, None::<&str>)
            else {
                return;
            };
            let sep = separator();
            let Some(sep) = sep else {
                return;
            };
            let sep2 = separator();
            let Some(sep2) = sep2 else {
                return;
            };
            let sep3 = separator();
            let Some(sep3) = sep3 else {
                return;
            };
            let Ok(m) = Menu::with_items(
                app,
                &[
                    &version_i,
                    &sep,
                    &cancel_i,
                    &sep2,
                    &settings_i,
                    &check_updates_i,
                    &sep3,
                    &quit_i,
                ],
            ) else {
                return;
            };
            m
        }
        TrayIconState::Idle => {
            let sep = separator();
            let Some(sep) = sep else {
                return;
            };
            let sep2 = separator();
            let Some(sep2) = sep2 else {
                return;
            };
            let Ok(m) = Menu::with_items(
                app,
                &[
                    &version_i,
                    &sep,
                    &settings_i,
                    &check_updates_i,
                    &sep2,
                    &quit_i,
                ],
            ) else {
                return;
            };
            m
        }
    };

    let tray = app.state::<TrayIcon>();
    let _ = tray.set_menu(Some(menu));
    let _ = tray.set_icon_as_template(true);
}

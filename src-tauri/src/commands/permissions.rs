use crate::audio_toolkit::has_screen_recording_permission;

#[tauri::command]
#[specta::specta]
pub fn check_screen_recording_permission() -> bool {
    has_screen_recording_permission()
}

#[tauri::command]
#[specta::specta]
pub fn request_screen_recording_permission_cmd() -> bool {
    #[cfg(target_os = "macos")]
    {
        use crate::audio_toolkit::request_screen_recording_permission;
        match request_screen_recording_permission() {
            Ok(granted) => granted,
            Err(_) => false,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

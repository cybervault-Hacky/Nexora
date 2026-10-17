mod commands;

/// Starts the Nexora desktop shell.
///
/// The foundation build exposes exactly one command (`get_app_status`). The
/// inference supervisor, model manager, and tool gateway are deliberately not
/// present yet; adding them requires the review gates documented in Phase 2.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::get_app_status])
        .run(tauri::generate_context!())
        .expect("Nexora failed to start");
}

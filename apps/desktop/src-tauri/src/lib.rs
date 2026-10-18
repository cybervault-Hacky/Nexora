mod history;
mod inference;
mod workspace;

use tauri::Manager;

/// Starts the Nexora desktop shell.
///
/// The only capabilities exposed to the webview are the read-only status
/// snapshot, model/runtime configuration with validation, engine start/stop,
/// and one streaming generation request. There is no shell, no filesystem
/// browsing, and no network access from the UI.
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            std::fs::create_dir_all(&config_dir)?;
            app.manage(inference::InferenceState::load(config_dir.clone()));
            app.manage(history::commands::HistoryState::new(
                app.path().app_data_dir()?,
            ));
            app.manage(workspace::commands::WorkspaceState::new());
            app.manage(workspace::commands::WorkspaceSettingsState::new(
                config_dir.clone(),
            ));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if let Some(state) = window.try_state::<inference::InferenceState>() {
                    if let Ok(mut supervisor) = state.supervisor.lock() {
                        supervisor.stop();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            inference::commands::get_app_status,
            inference::commands::get_system_info,
            inference::commands::get_settings,
            inference::commands::save_settings,
            inference::commands::validate_model,
            inference::commands::probe_runtime,
            inference::commands::load_model,
            inference::commands::unload_model,
            inference::commands::start_generation,
            inference::commands::cancel_generation,
            history::commands::list_conversations,
            history::commands::load_conversation,
            history::commands::create_conversation,
            history::commands::save_conversation,
            history::commands::rename_conversation,
            history::commands::delete_conversation,
            workspace::commands::set_workspace,
            workspace::commands::scan_workspace_command,
            workspace::commands::scan_current_workspace,
            workspace::commands::read_workspace_file_command,
            workspace::commands::get_workspace_context,
            workspace::commands::close_workspace,
            workspace::commands::get_workspace,
            workspace::commands::pick_workspace_dialog,
            workspace::commands::get_workspace_settings,
            workspace::commands::save_workspace_settings,
            workspace::commands::create_patch_proposal,
            workspace::commands::validate_patch_proposal,
            workspace::commands::apply_patch_proposal,
            workspace::commands::get_patch_diff,
        ])
        .run(tauri::generate_context!())
        .expect("Nexora failed to start");
}

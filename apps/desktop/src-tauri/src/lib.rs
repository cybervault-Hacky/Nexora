mod auth;
mod export;
mod history;
mod inference;
mod licensing;
mod usage;
mod workspace;

use tauri::Manager;

/// Stops all desktop child processes and cancels in-flight background work
/// without panicking, even if a state mutex was poisoned earlier.
fn shutdown_desktop_resources(app: &tauri::AppHandle) {
    if let Some(auth_state) = app.try_state::<auth::commands::AuthState>() {
        auth_state
            .google_cancel
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    if let Some(export_state) = app.try_state::<export::ExportState>() {
        if let Ok(slot) = export_state.active.lock() {
            if let Some(flag) = slot.as_ref() {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
    }

    if let Some(state) = app.try_state::<inference::InferenceState>() {
        state.shutdown();
    }
}

/// Starts the Nexora desktop shell.
///
/// The only capabilities exposed to the webview are the read-only status
/// snapshot, model/runtime configuration with validation, engine start/stop,
/// one streaming generation request, and a read-only daily usage snapshot.
/// There is no shell, no filesystem browsing, and no network access from the
/// UI.
pub fn run() {
    let app = match tauri::Builder::default()
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&config_dir)?;
            std::fs::create_dir_all(&data_dir)?;

            app.manage(inference::InferenceState::load(config_dir.clone()));
            app.manage(usage::UsageState::load(&config_dir));
            app.manage(history::commands::HistoryState::new(data_dir));
            app.manage(workspace::commands::WorkspaceState::new());
            app.manage(export::ExportState::new());
            app.manage(workspace::commands::WorkspaceSettingsState::new(
                config_dir.clone(),
            ));
            app.manage(workspace::commands::SymbolIndexState::new());
            app.manage(workspace::commands::AnalysisState::new());
            app.manage(auth::commands::AuthState::new());
            app.manage(licensing::commands::LicenceState::new(Some(
                config_dir.clone(),
            )));
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                shutdown_desktop_resources(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            inference::commands::get_app_status,
            inference::commands::get_system_info,
            inference::commands::get_disk_space,
            inference::commands::get_settings,
            inference::commands::get_model_profile,
            inference::commands::select_model,
            inference::commands::get_model_catalog,
            inference::commands::check_model_readiness,
            inference::commands::save_settings,
            inference::commands::validate_model,
            inference::commands::probe_runtime,
            inference::commands::load_model,
            inference::commands::unload_model,
            inference::commands::start_generation,
            usage::commands::get_usage_status,
            inference::commands::cancel_generation,
            inference::commands::start_model_download,
            inference::commands::cancel_model_download,
            inference::commands::verify_model,
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
            workspace::commands::create_patch_batch,
            workspace::commands::validate_patch_batch,
            workspace::commands::apply_patch_batch,
            workspace::commands::validate_patch_detailed,
            workspace::commands::get_symbol_index,
            workspace::commands::refresh_symbol_index,
            workspace::commands::search_symbols,
            workspace::commands::clear_symbol_index,
            workspace::commands::get_dependency_graph,
            workspace::commands::refresh_dependency_graph,
            workspace::commands::get_file_relationships,
            workspace::commands::get_change_impact,
            workspace::commands::clear_analysis_graph,
            export::commands::plan_workspace_export,
            export::commands::suggest_export_path,
            export::commands::export_single_file,
            export::commands::export_zip,
            export::commands::cancel_export,
            auth::commands::auth_get_session,
            auth::commands::auth_refresh_session,
            auth::commands::auth_login,
            auth::commands::auth_register,
            auth::commands::auth_verify_email,
            auth::commands::auth_resend_verification,
            auth::commands::auth_recover_password,
            auth::commands::auth_reset_password,
            auth::commands::auth_logout,
            auth::commands::auth_change_password,
            auth::commands::get_developer_diagnostics,
            auth::commands::auth_google_begin,
            auth::commands::auth_google_cancel,
            auth::commands::account_get_profile,
            auth::commands::account_update_profile,
            auth::commands::account_link_google,
            auth::commands::account_export_data,
            auth::commands::account_delete,
            auth::commands::billing_list_plans,
            auth::commands::billing_get_status,
            auth::commands::billing_create_checkout,
            auth::commands::billing_verify_order,
            auth::commands::billing_cancel_subscription,
            auth::commands::billing_change_plan,
            auth::commands::billing_open_portal,
            auth::commands::usage_get_server_summary,
            licensing::commands::licence_get_status,
            licensing::commands::licence_activate,
            licensing::commands::licence_deactivate,
        ])
        .build(tauri::generate_context!())
    {
        Ok(app) => app,
        Err(error) => {
            eprintln!("nexora: failed to initialize desktop application: {error}");
            std::process::exit(1);
        }
    };

    app.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            shutdown_desktop_resources(app_handle);
        }
    });
}

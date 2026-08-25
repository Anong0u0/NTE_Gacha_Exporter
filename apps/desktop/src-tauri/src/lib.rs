mod admin;
#[cfg(feature = "agent-smoke")]
mod agent_smoke;
mod assets_commands;
mod capture;
mod diagnostic;
mod error;
mod instance;
mod lifecycle;
mod state;
mod store_commands;
mod system_commands;
mod update_commands;
mod update_service;
mod windivert_commands;
mod window_commands;

use nte_store::JsonStore;
use tauri::{Manager, RunEvent};

use crate::admin::{pending_admin_capture_from_args, pending_admin_diagnostic_from_args};
use crate::state::{AppState, portable_root};

pub fn run() {
    let pending_admin_capture = pending_admin_capture_from_args()
        .unwrap_or_else(|error| panic!("failed to read pending admin capture: {error:?}"));
    let pending_admin_diagnostic = pending_admin_diagnostic_from_args()
        .unwrap_or_else(|error| panic!("failed to read pending admin diagnostic: {error:?}"));
    let root =
        portable_root().unwrap_or_else(|error| panic!("failed to resolve portable root: {error}"));
    let admin_handoff = pending_admin_capture.is_some() || pending_admin_diagnostic.is_some();
    let _instance_guard = match instance::acquire(&root, admin_handoff)
        .unwrap_or_else(|error| panic!("failed to acquire application instance: {error:#}"))
    {
        instance::AcquireResult::Primary(guard) => guard,
        instance::AcquireResult::Existing => return,
    };
    let assets_root = root.clone();
    let store_root = root.clone();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("nteasset", move |_ctx, request| {
            assets_commands::assets_protocol_response(&assets_root, request)
        })
        .setup(move |app| {
            let _ = nte_update::cleanup_update_artifacts_after_success(&store_root);
            let store = JsonStore::open_with_defaults(
                store_root.clone(),
                system_commands::store_defaults(),
            )
            .map_err(|err| format!("failed to open JSON store: {err}"))?;
            app.manage(AppState::new(
                store,
                pending_admin_capture.clone(),
                pending_admin_diagnostic.clone(),
            ));
            #[cfg(feature = "agent-smoke")]
            agent_smoke::maybe_start(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            store_commands::get_settings,
            store_commands::update_settings,
            store_commands::list_profiles,
            store_commands::create_profile,
            store_commands::set_active_profile,
            store_commands::rename_profile,
            store_commands::delete_profile,
            store_commands::import_public_json,
            store_commands::import_raw_jsonl,
            store_commands::profile_analysis_view,
            store_commands::dashboard_overview,
            store_commands::pool_kind_detail,
            store_commands::dashboard_selection_detail,
            store_commands::dashboard_scope_detail,
            store_commands::list_records,
            store_commands::record_page,
            store_commands::record_filter_options,
            store_commands::export_public_json,
            store_commands::export_csv,
            store_commands::create_backup,
            store_commands::restore_backup,
            update_commands::updater_status,
            update_commands::updater_check,
            update_commands::updater_download_and_stage,
            update_commands::updater_install_staged,
            assets_commands::assets_resolve_refs,
            windivert_commands::windivert_status,
            windivert_commands::windivert_install,
            system_commands::maps_list,
            system_commands::ui_locale_list,
            system_commands::system_locale,
            system_commands::open_about_link,
            system_commands::doctor_run,
            admin::request_admin_capture_start,
            admin::request_admin_diagnostic_start,
            admin::take_pending_admin_capture,
            admin::take_pending_admin_diagnostic,
            capture::capture_start,
            capture::capture_status,
            capture::capture_stop,
            diagnostic::diagnostic_start,
            diagnostic::diagnostic_status,
            diagnostic::diagnostic_cancel
        ])
        .build(tauri::generate_context!())
        .expect("error while running Tauri application");
    app.run(|app, event| {
        if matches!(event, RunEvent::ExitRequested { .. }) {
            app.state::<AppState>().shutdown();
        }
    });
}

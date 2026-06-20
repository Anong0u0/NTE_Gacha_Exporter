mod admin;
#[cfg(feature = "agent-smoke")]
mod agent_smoke;
mod assets_commands;
mod capture;
mod error;
mod state;
mod store_commands;
mod system_commands;
mod update_commands;

use nte_store::JsonStore;
use tauri::Manager;

use crate::admin::pending_admin_capture_from_args;
use crate::state::{AppState, portable_root};

pub fn run() {
    let pending_admin_capture = pending_admin_capture_from_args()
        .unwrap_or_else(|error| panic!("failed to read pending admin capture: {error:?}"));
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("nteasset", |_ctx, request| {
            let root = portable_root()
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| ".".into()));
            assets_commands::assets_protocol_response(&root, request)
        })
        .setup(move |app| {
            let root =
                portable_root().map_err(|err| format!("failed to resolve portable root: {err}"))?;
            let store =
                JsonStore::open(root).map_err(|err| format!("failed to open JSON store: {err}"))?;
            app.manage(AppState::new(store, pending_admin_capture.clone()));
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
            store_commands::dashboard_overview,
            store_commands::pool_kind_detail,
            store_commands::list_records,
            store_commands::record_filter_options,
            store_commands::export_public_json,
            store_commands::export_csv,
            store_commands::create_backup,
            store_commands::restore_backup,
            update_commands::updater_status,
            update_commands::updater_check,
            update_commands::updater_download_and_stage,
            update_commands::updater_install_staged,
            assets_commands::assets_pack_status,
            assets_commands::assets_pack_check,
            assets_commands::assets_pack_download_and_install,
            assets_commands::assets_pack_remove,
            assets_commands::assets_resolve_refs,
            system_commands::maps_list,
            system_commands::doctor_run,
            system_commands::runtime_ping,
            admin::request_admin_capture_start,
            admin::take_pending_admin_capture,
            capture::capture_start,
            capture::capture_status,
            capture::capture_stop
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}

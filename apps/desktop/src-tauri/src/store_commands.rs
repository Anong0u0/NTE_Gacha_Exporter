use std::fs;
use std::path::Path;

use nte_capture::{build_capture_document, read_raw_capture};
use nte_core::{
    BackupReport, DashboardOverview, DashboardSelection, DashboardSelectionDetail, ImportReport,
    PoolKind, PoolKindDetail, Profile, RecordFilter, RecordFilterOptions, RecordList,
    RestoreReport, Settings, SettingsPatch,
};
use nte_store::load_locale_or_settings;
use tauri::State;

use crate::error::{ApiError, api_error};
use crate::state::{AppState, with_store};

#[tauri::command]
pub(crate) fn get_settings(state: State<'_, AppState>) -> Result<Settings, ApiError> {
    with_store(&state, |store| store.settings())
}

#[tauri::command]
pub(crate) fn update_settings(
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<Settings, ApiError> {
    with_store(&state, |store| store.update_settings(patch))
}

#[tauri::command]
pub(crate) fn list_profiles(state: State<'_, AppState>) -> Result<Vec<Profile>, ApiError> {
    with_store(&state, |store| store.list_profiles())
}

#[tauri::command]
pub(crate) fn create_profile(
    state: State<'_, AppState>,
    name: String,
) -> Result<Profile, ApiError> {
    with_store(&state, |store| store.create_profile(&name))
}

#[tauri::command]
pub(crate) fn set_active_profile(
    state: State<'_, AppState>,
    profile_name: String,
) -> Result<Settings, ApiError> {
    with_store(&state, |store| store.set_active_profile(&profile_name))
}

#[tauri::command]
pub(crate) fn rename_profile(
    state: State<'_, AppState>,
    old_name: String,
    new_name: String,
) -> Result<Profile, ApiError> {
    with_store(&state, |store| store.rename_profile(&old_name, &new_name))
}

#[tauri::command]
pub(crate) fn delete_profile(
    state: State<'_, AppState>,
    profile_name: String,
) -> Result<Settings, ApiError> {
    with_store(&state, |store| store.delete_profile(&profile_name))
}

#[tauri::command]
pub(crate) fn import_public_json(
    state: State<'_, AppState>,
    profile_name: String,
    path: String,
) -> Result<ImportReport, ApiError> {
    let text = fs::read_to_string(&path).map_err(api_error)?;
    with_store(&state, |store| {
        store.import_public_document(&profile_name, &text, "public_json", Some(&path))
    })
}

#[tauri::command]
pub(crate) fn import_raw_jsonl(
    state: State<'_, AppState>,
    profile_name: String,
    path: String,
    locale: Option<String>,
) -> Result<ImportReport, ApiError> {
    let locale = with_store(&state, |store| load_locale_or_settings(store, locale))?;
    let rows = read_raw_capture(Path::new(&path)).map_err(api_error)?;
    let document = build_capture_document(&rows.rows, &locale).map_err(api_error)?;
    let document_text = serde_json::to_string(&document).map_err(api_error)?;
    with_store(&state, |store| {
        store.import_public_document(&profile_name, &document_text, "raw_jsonl", Some(&path))
    })
}

#[tauri::command]
pub(crate) fn dashboard_overview(
    state: State<'_, AppState>,
    profile_name: String,
    locale: Option<String>,
) -> Result<DashboardOverview, ApiError> {
    with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.dashboard_overview(&profile_name, &locale)
    })
}

#[tauri::command]
pub(crate) fn pool_kind_detail(
    state: State<'_, AppState>,
    profile_name: String,
    pool_kind: PoolKind,
    locale: Option<String>,
) -> Result<PoolKindDetail, ApiError> {
    with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.pool_kind_detail(&profile_name, &locale, pool_kind)
    })
}

#[tauri::command]
pub(crate) fn dashboard_selection_detail(
    state: State<'_, AppState>,
    profile_name: String,
    selection: DashboardSelection,
    locale: Option<String>,
) -> Result<DashboardSelectionDetail, ApiError> {
    with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.dashboard_selection_detail(&profile_name, &locale, &selection)
    })
}

#[tauri::command]
pub(crate) fn list_records(
    state: State<'_, AppState>,
    profile_name: String,
    filter: RecordFilter,
    locale: Option<String>,
) -> Result<RecordList, ApiError> {
    with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.list_records(&profile_name, &locale, &filter)
    })
}

#[tauri::command]
pub(crate) fn record_filter_options(
    state: State<'_, AppState>,
    profile_name: String,
    locale: Option<String>,
) -> Result<RecordFilterOptions, ApiError> {
    with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.record_filter_options(&profile_name, &locale)
    })
}

#[tauri::command]
pub(crate) fn export_public_json(
    state: State<'_, AppState>,
    profile_name: String,
    path: String,
    locale: Option<String>,
) -> Result<(), ApiError> {
    with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.export_public_json(&profile_name, &locale, path)
    })
}

#[tauri::command]
pub(crate) fn export_csv(
    state: State<'_, AppState>,
    profile_name: String,
    path: String,
    locale: Option<String>,
) -> Result<(), ApiError> {
    with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.export_csv(&profile_name, &locale, path)
    })
}

#[tauri::command]
pub(crate) fn create_backup(
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<BackupReport, ApiError> {
    with_store(&state, |store| {
        store.create_data_backup_report(path.as_deref())
    })
}

#[tauri::command]
pub(crate) fn restore_backup(
    state: State<'_, AppState>,
    path: String,
) -> Result<RestoreReport, ApiError> {
    with_store(&state, |store| store.restore_data_backup_report(path))
}

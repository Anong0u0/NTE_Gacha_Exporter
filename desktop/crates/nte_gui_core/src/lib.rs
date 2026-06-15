mod analysis;
mod export;
mod maps;
mod model;
mod public_json;
mod rules;
mod store;
mod updater;

pub use maps::{available_locales, load_map};
pub use model::{
    BackupReport, DashboardOverview, DisplayRecord, FiveStarRecord, FiveStarResult, GuiError,
    ImportReport, InternalRecord, ItemRank, MapLocaleList, PoolKind, PoolKindDetail,
    PoolKindSummary, Profile, RarityBucket, RecordFilter, RecordFilterOptions, RecordList,
    RecordPoolOption, RecordSortKey, RecordTypeOption, RestoreReport, Settings, SettingsPatch,
    SortDirection, UpdateChannel, UpdateCheckReport, UpdateInstallPlan, UpdateManifest,
    UpdatePackage, UpdateStageReport, UpdateStatus,
};
pub use rules::{classify_pool_id, rule_for, GachaRule};
pub use store::{load_locale_or_settings, DataBackup, JsonStore};
pub use updater::{
    apply_staged_update, check_update_manifest, prepare_update_install, stage_update_archive,
    update_status,
};

#[cfg(test)]
mod store_tests;
#[cfg(test)]
mod updater_tests;

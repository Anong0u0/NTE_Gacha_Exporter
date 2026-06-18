mod analysis;
mod capture_document;
mod capture_live;
mod capture_net;
mod capture_protocol;
mod capture_raw;
mod derived;
mod export;
mod maps;
mod model;
mod public_json;
mod rules;
mod store;
mod updater;

pub use capture_document::{build_capture_document, RawReplayResult};
pub use capture_live::{
    capture_live, CaptureCounters, CaptureOptions, CaptureProgress, CaptureResult, CaptureTarget,
};
pub use capture_net::{
    candidate_ports, capture_doctor, find_process_pid, is_admin, CaptureDoctorReport,
};
pub use capture_raw::read_raw_capture;
pub use derived::derive_records;
pub use maps::{available_locales, load_map};
pub use model::{
    BackupReport, BannerResolutionStatus, BannerSummary, DashboardOverview, DisplayRecord,
    FiveStarRecord, FiveStarResult, FourStarRecord, GachaRuleView, GuiError, ImportReport,
    InternalRecord, ItemRank, MapLocaleList, PhaseSummary, PoolKind, PoolKindDetail,
    PoolKindSummary, Profile, RarityBucket, RateUpResult, RecordBannerOption, RecordDerived,
    RecordFilter, RecordFilterOptions, RecordList, RecordPoolOption, RecordSortKey,
    RecordTypeOption, ResolvedBanner, ResourcePoolKindSummary, ResourceSummary, RestoreReport,
    RuleResolutionStatus, Settings, SettingsPatch, SortDirection, TimeBucketSummary, TimeStats,
    UpdateChannel, UpdateCheckReport, UpdateInstallPlan, UpdateManifest, UpdatePackage,
    UpdateStageReport, UpdateStatus,
};
pub use rules::{
    classify_pool_id, derive_pool_kind_hits, fallback_rule_for, rule_for, rule_for_record,
    rule_for_resolved_banner, DerivedHit, GachaRule, PoolKindDerivedStats, RuleResolution,
};
pub use store::{load_locale_or_settings, DataBackup, JsonStore};
pub use updater::{
    apply_staged_update, check_update_manifest, prepare_update_install, stage_update_archive,
    update_status,
};

#[cfg(test)]
mod store_tests;
#[cfg(test)]
mod updater_tests;

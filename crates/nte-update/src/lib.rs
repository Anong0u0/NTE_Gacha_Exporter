mod updater;

pub use updater::{
    apply_staged_update, check_update_manifest, cleanup_update_artifacts_after_success,
    prepare_update_install, stage_update_archive, update_status,
};

#[cfg(test)]
mod updater_tests;

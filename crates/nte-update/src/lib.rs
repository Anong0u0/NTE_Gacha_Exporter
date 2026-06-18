mod updater;

pub use updater::{
    apply_staged_update, check_update_manifest, prepare_update_install, stage_update_archive,
    update_status,
};

#[cfg(test)]
mod updater_tests;

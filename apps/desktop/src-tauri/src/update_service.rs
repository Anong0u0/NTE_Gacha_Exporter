use std::path::Path;

use nte_core::{UpdateChannel, UpdateCheckReport, UpdatePackage, UpdateStageReport};
use nte_update::{check_update_manifest, stage_update_archive};

use crate::error::{ApiError, api_error};

mod download;
mod github;
mod types;

pub(crate) fn check_for_update(
    requested_channel: UpdateChannel,
    current_version: &str,
) -> Result<UpdateCheckReport, ApiError> {
    let fetched = github::fetch_update_manifest(requested_channel, current_version)?;
    check_update_manifest(
        fetched.manifest,
        current_version,
        requested_channel,
        fetched.changelog,
    )
    .map_err(api_error)
}

pub(crate) fn download_and_stage_update(
    root: &Path,
    package: UpdatePackage,
) -> Result<UpdateStageReport, ApiError> {
    let archive_path = download::download_update_archive(root, &package)?;
    stage_update_archive(root, &package, archive_path).map_err(api_error)
}

#[cfg(test)]
mod tests;

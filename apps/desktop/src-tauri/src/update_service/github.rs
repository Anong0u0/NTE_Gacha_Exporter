use std::fs;

use nte_core::{UpdateChangelogEntry, UpdateChannel, UpdateManifest};
use semver::Version;

use super::download::{http_get_json, read_text_limited};
use super::types::{
    FetchedUpdateManifest, GITHUB_RELEASES_API, GITHUB_RELEASES_PAGE_SIZE, GithubRelease,
    MAX_UPDATE_JSON_BYTES, UPDATE_MANIFEST_ASSET, normalize_version,
};
use crate::error::{ApiError, api_error, api_error_message};

pub(super) fn fetch_update_manifest(
    channel: UpdateChannel,
    current_version: &str,
) -> Result<FetchedUpdateManifest, ApiError> {
    if let Ok(source) = std::env::var("NTE_GACHA_EXPORTER_UPDATE_MANIFEST") {
        if !source.trim().is_empty() {
            if source.starts_with("http://") || source.starts_with("https://") {
                return Ok(FetchedUpdateManifest {
                    manifest: http_get_json(&source)?,
                    changelog: Vec::new(),
                });
            }
            let file = fs::File::open(source).map_err(api_error)?;
            let text = read_text_limited(
                file,
                MAX_UPDATE_JSON_BYTES,
                "update_response_too_large",
                "update JSON file",
            )?;
            return Ok(FetchedUpdateManifest {
                manifest: serde_json::from_str(&text).map_err(api_error)?,
                changelog: Vec::new(),
            });
        }
    }

    let releases: Vec<GithubRelease> = http_get_json(&format!(
        "{GITHUB_RELEASES_API}?per_page={GITHUB_RELEASES_PAGE_SIZE}&page=1"
    ))?;
    collect_update_manifest_and_changelog(releases, channel, current_version)
}

fn collect_update_manifest_and_changelog(
    releases: Vec<GithubRelease>,
    channel: UpdateChannel,
    current_version: &str,
) -> Result<FetchedUpdateManifest, ApiError> {
    collect_update_manifest_and_changelog_with_resolver(releases, channel, current_version, |url| {
        http_get_json(url)
    })
}

pub(super) fn collect_update_manifest_and_changelog_with_resolver(
    releases: Vec<GithubRelease>,
    channel: UpdateChannel,
    current_version: &str,
    mut resolve_manifest: impl FnMut(&str) -> Result<UpdateManifest, ApiError>,
) -> Result<FetchedUpdateManifest, ApiError> {
    let current = parse_update_version(current_version)?;
    let target = releases
        .iter()
        .filter(|release| release_matches_channel(release, channel))
        .filter_map(|release| Some((release, release.tag_version()?)))
        .max_by(|(_, left), (_, right)| left.cmp(right))
        .ok_or_else(|| {
            api_error_message("update_release_missing", "no matching GitHub release found")
        })?;

    let manifest_url = release_manifest_url(target.0).ok_or_else(|| {
        api_error_message("update_manifest_missing", "release update manifest missing")
    })?;
    let manifest = resolve_manifest(&manifest_url)?;
    if !manifest_matches_channel(&manifest, channel) {
        return Err(api_error_message(
            "update_release_missing",
            "no matching GitHub release found",
        ));
    }

    let target_version = parse_update_version(&manifest.version)?;
    let mut changelog = releases
        .into_iter()
        .filter(|release| release_matches_channel(release, channel))
        .filter_map(|release| {
            let version = release.tag_version()?;
            (version > current
                && version <= target_version
                && version_matches_channel(&version, channel))
            .then_some((version, release))
        })
        .filter_map(|(version, release)| {
            let release_notes = first_release_notes_section(&release.body.unwrap_or_default());
            (!release_notes.is_empty()).then_some((
                version,
                UpdateChangelogEntry {
                    version: normalize_version(&release.tag_name).to_string(),
                    release_notes,
                },
            ))
        })
        .collect::<Vec<_>>();
    changelog.sort_by(|(left, _), (right, _)| right.cmp(left));
    let changelog = changelog.into_iter().map(|(_, entry)| entry).collect();

    Ok(FetchedUpdateManifest {
        manifest,
        changelog,
    })
}

fn release_manifest_url(release: &GithubRelease) -> Option<String> {
    release
        .assets
        .iter()
        .find(|asset| asset.name == UPDATE_MANIFEST_ASSET)
        .map(|asset| asset.browser_download_url.clone())
}

fn manifest_matches_channel(manifest: &UpdateManifest, channel: UpdateChannel) -> bool {
    match channel {
        UpdateChannel::Stable => {
            manifest.channel == UpdateChannel::Stable
                && parse_update_version(&manifest.version)
                    .is_ok_and(|version| version_matches_channel(&version, channel))
        }
        UpdateChannel::Beta => true,
    }
}

fn version_matches_channel(version: &Version, channel: UpdateChannel) -> bool {
    channel == UpdateChannel::Beta || version.pre.is_empty()
}

fn parse_update_version(value: &str) -> Result<Version, ApiError> {
    Version::parse(normalize_version(value)).map_err(|_| {
        api_error_message(
            "invalid_update_version",
            format!("invalid update version: {value}"),
        )
    })
}

fn release_matches_channel(release: &GithubRelease, channel: UpdateChannel) -> bool {
    !release.draft && (channel == UpdateChannel::Beta || !release.prerelease)
}

pub(super) fn first_release_notes_section(body: &str) -> String {
    let mut lines = body
        .lines()
        .skip_while(|line| !is_release_notes_heading(line));
    if lines.next().is_none() {
        return body.trim().to_string();
    }

    lines
        .take_while(|line| !is_release_notes_heading(line))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn is_release_notes_heading(line: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix("###") else {
        return false;
    };
    !rest.starts_with('#') && rest.chars().next().is_none_or(char::is_whitespace)
}

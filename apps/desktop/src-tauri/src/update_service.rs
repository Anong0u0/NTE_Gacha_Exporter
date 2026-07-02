use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use nte_core::{
    UpdateChangelogEntry, UpdateChannel, UpdateCheckReport, UpdateManifest, UpdatePackage,
    UpdateStageReport,
};
use nte_update::{check_update_manifest, stage_update_archive};
use semver::Version;
use serde::Deserialize;

use crate::error::{ApiError, api_error, api_error_message};

const GITHUB_RELEASES_API: &str =
    "https://api.github.com/repos/Anong0u0/nte_gacha_exporter/releases";
const UPDATE_MANIFEST_ASSET: &str = "nte-gacha-exporter-update.json";
const USER_AGENT: &str = "nte-gacha-exporter-updater";
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(30);
const HTTP_WRITE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_UPDATE_JSON_BYTES: u64 = 1024 * 1024;
const READ_BUFFER_BYTES: usize = 64 * 1024;
const GITHUB_RELEASES_PAGE_SIZE: u32 = 100;

pub(crate) fn check_for_update(
    requested_channel: UpdateChannel,
    current_version: &str,
) -> Result<UpdateCheckReport, ApiError> {
    let fetched = fetch_update_manifest(requested_channel, current_version)?;
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
    let archive_path = download_update_archive(root, &package)?;
    stage_update_archive(root, &package, archive_path).map_err(api_error)
}

fn fetch_update_manifest(
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

fn collect_update_manifest_and_changelog_with_resolver(
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

fn normalize_version(value: &str) -> &str {
    value.trim().strip_prefix('v').unwrap_or(value.trim())
}

fn release_matches_channel(release: &GithubRelease, channel: UpdateChannel) -> bool {
    !release.draft && (channel == UpdateChannel::Beta || !release.prerelease)
}

fn http_get_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, ApiError> {
    let response = http_agent()
        .get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(api_error)?;
    let text = read_text_limited(
        response.into_reader(),
        MAX_UPDATE_JSON_BYTES,
        "update_response_too_large",
        "update JSON response",
    )?;
    serde_json::from_str(&text).map_err(api_error)
}

fn download_update_archive(root: &Path, package: &UpdatePackage) -> Result<PathBuf, ApiError> {
    let downloads = root.join("update").join("downloads").join(&package.version);
    fs::create_dir_all(&downloads).map_err(api_error)?;
    let path = downloads.join(&package.asset_name);
    let tmp_path = downloads.join(format!("{}.tmp", package.asset_name));
    let response = http_agent()
        .get(&package.download_url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(api_error)?;
    if let Some(content_length) = response.header("Content-Length") {
        let size = content_length.trim().parse::<u64>().map_err(api_error)?;
        if size != package.size {
            return Err(api_error_message(
                "update_archive_size_mismatch",
                format!(
                    "update archive content length mismatch: expected {}, got {size}",
                    package.size
                ),
            ));
        }
    }
    let mut reader = response.into_reader();
    let mut file = fs::File::create(&tmp_path).map_err(api_error)?;
    let result = copy_limited(&mut reader, &mut file, package.size);
    if let Err(error) = result {
        let _ = fs::remove_file(&tmp_path);
        return Err(error);
    }
    if let Err(error) = file.flush() {
        let _ = fs::remove_file(&tmp_path);
        return Err(api_error(error));
    }
    if let Err(error) = fs::rename(&tmp_path, &path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(api_error(error));
    }
    Ok(path)
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(HTTP_CONNECT_TIMEOUT)
        .timeout_read(HTTP_READ_TIMEOUT)
        .timeout_write(HTTP_WRITE_TIMEOUT)
        .build()
}

fn read_text_limited(
    mut reader: impl Read,
    max_bytes: u64,
    error_code: &str,
    label: &str,
) -> Result<String, ApiError> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; READ_BUFFER_BYTES];
    loop {
        let read = reader.read(&mut buffer).map_err(api_error)?;
        if read == 0 {
            break;
        }
        let next_len = bytes.len() as u64 + read as u64;
        if next_len > max_bytes {
            return Err(api_error_message(
                error_code,
                format!("{label} exceeds {max_bytes} bytes"),
            ));
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    String::from_utf8(bytes).map_err(api_error)
}

fn first_release_notes_section(body: &str) -> String {
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

fn copy_limited(
    reader: &mut impl Read,
    writer: &mut impl Write,
    max_bytes: u64,
) -> Result<(), ApiError> {
    let mut written = 0_u64;
    let mut buffer = [0_u8; READ_BUFFER_BYTES];
    loop {
        let read = reader.read(&mut buffer).map_err(api_error)?;
        if read == 0 {
            break;
        }
        written += read as u64;
        if written > max_bytes {
            return Err(api_error_message(
                "update_archive_too_large",
                format!("update archive exceeds {max_bytes} bytes"),
            ));
        }
        writer.write_all(&buffer[..read]).map_err(api_error)?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    body: Option<String>,
    assets: Vec<GithubAsset>,
}

impl GithubRelease {
    fn tag_version(&self) -> Option<Version> {
        Version::parse(normalize_version(&self.tag_name)).ok()
    }
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug)]
struct FetchedUpdateManifest {
    manifest: UpdateManifest,
    changelog: Vec<UpdateChangelogEntry>,
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::io::Cursor;

    use super::*;

    #[test]
    fn read_text_limited_rejects_large_json_response() {
        let bytes = vec![b'a'; 8];
        let error = read_text_limited(
            Cursor::new(bytes),
            7,
            "update_response_too_large",
            "update JSON response",
        )
        .expect_err("oversized response should fail");

        assert_eq!(
            api_error_code(error).as_deref(),
            Some("update_response_too_large")
        );
    }

    #[test]
    fn copy_limited_rejects_archive_larger_than_expected() {
        let mut reader = Cursor::new(vec![1_u8; 9]);
        let mut writer = Vec::new();

        let error =
            copy_limited(&mut reader, &mut writer, 8).expect_err("oversized archive should fail");

        assert_eq!(
            api_error_code(error).as_deref(),
            Some("update_archive_too_large")
        );
    }

    #[test]
    fn copy_limited_accepts_exact_expected_archive_size() {
        let bytes = vec![1_u8, 2, 3, 4];
        let mut reader = Cursor::new(bytes.clone());
        let mut writer = Vec::new();

        copy_limited(&mut reader, &mut writer, bytes.len() as u64)
            .expect("exact archive size should copy");

        assert_eq!(writer, bytes);
    }

    #[test]
    fn copy_limited_allows_short_archive_for_stage_validation() {
        let bytes = vec![1_u8, 2];
        let mut reader = Cursor::new(bytes.clone());
        let mut writer = Vec::new();

        copy_limited(&mut reader, &mut writer, 4).expect("short archive should copy");

        assert_eq!(writer, bytes);
    }

    #[test]
    fn first_release_notes_section_keeps_only_first_level_three_section() {
        let body = r#"
### 更新日誌 v1.0.0 -> v1.1.0:
* feat: one
* fix: two

### Download / 下載說明
* nte-gacha-exporter.zip

### Do not download / 不要下載
* Source code
"#;

        let notes = first_release_notes_section(body);

        assert_eq!(notes, "* feat: one\n* fix: two");
    }

    #[test]
    fn first_release_notes_section_ignores_text_before_first_level_three_heading() {
        let body = r#"
v1.1.0

### 更新日誌
* feat: one

### Download
* package
"#;

        let notes = first_release_notes_section(body);

        assert_eq!(notes, "* feat: one");
    }

    #[test]
    fn first_release_notes_section_falls_back_to_trimmed_body_without_heading() {
        let notes = first_release_notes_section("\nplain notes\n");

        assert_eq!(notes, "plain notes");
    }

    #[test]
    fn update_changelog_collects_first_page_range_newest_first_and_skips_empty_notes() {
        let report = collect_update_manifest_and_changelog_with_resolver(
            vec![
                release_with_manifest(
                    "v1.2.8",
                    false,
                    "https://example.invalid/1.2.8.json",
                    "### 更新日誌\nfeat: aabbcc\n\n### Download\nzip",
                ),
                release_without_manifest("v1.2.7", false, "### 更新日誌\nfix: 112233"),
                release_without_manifest("v1.2.6", false, "### 更新日誌\n\n### Download\nzip"),
                release_with_manifest(
                    "v1.1.0",
                    false,
                    "https://example.invalid/1.1.0.json",
                    "### 更新日誌\nold",
                ),
            ],
            UpdateChannel::Stable,
            "1.1.0",
            manifest_resolver(&[(
                "https://example.invalid/1.2.8.json",
                "1.2.8",
                UpdateChannel::Stable,
            )]),
        )
        .expect("update report should be collected");

        assert_eq!(report.manifest.version, "1.2.8");
        assert_eq!(
            report.changelog,
            vec![
                UpdateChangelogEntry {
                    version: "1.2.8".to_string(),
                    release_notes: "feat: aabbcc".to_string(),
                },
                UpdateChangelogEntry {
                    version: "1.2.7".to_string(),
                    release_notes: "fix: 112233".to_string(),
                },
            ]
        );
    }

    #[test]
    fn update_changelog_downloads_only_target_manifest() {
        let manifest_requests = Cell::new(0);
        let report = collect_update_manifest_and_changelog_with_resolver(
            vec![
                release_with_manifest(
                    "v1.2.8",
                    false,
                    "https://example.invalid/1.2.8.json",
                    "### 更新日誌\nnewer",
                ),
                release_with_manifest(
                    "v1.2.7",
                    false,
                    "https://example.invalid/1.2.7.json",
                    "### 更新日誌\nolder",
                ),
            ],
            UpdateChannel::Stable,
            "1.1.0",
            |url| {
                manifest_requests.set(manifest_requests.get() + 1);
                assert_eq!(url, "https://example.invalid/1.2.8.json");
                Ok(test_manifest("1.2.8", UpdateChannel::Stable))
            },
        )
        .expect("target manifest should resolve");

        assert_eq!(manifest_requests.get(), 1);
        assert_eq!(report.manifest.version, "1.2.8");
    }

    #[test]
    fn stable_fast_check_skips_prereleases_and_drafts() {
        let report = collect_update_manifest_and_changelog_with_resolver(
            vec![
                release_with_manifest(
                    "v1.3.0-beta.1",
                    true,
                    "https://example.invalid/1.3.0-beta.1.json",
                    "### 更新日誌\nbeta",
                ),
                draft_release_with_manifest(
                    "v1.2.9",
                    false,
                    "https://example.invalid/1.2.9.json",
                    "### 更新日誌\ndraft",
                ),
                release_with_manifest(
                    "v1.2.8",
                    false,
                    "https://example.invalid/1.2.8.json",
                    "### 更新日誌\nstable",
                ),
            ],
            UpdateChannel::Stable,
            "1.1.0",
            manifest_resolver(&[(
                "https://example.invalid/1.2.8.json",
                "1.2.8",
                UpdateChannel::Stable,
            )]),
        )
        .expect("stable report should be collected");

        assert_eq!(report.manifest.version, "1.2.8");
        assert_eq!(
            report.changelog,
            vec![UpdateChangelogEntry {
                version: "1.2.8".to_string(),
                release_notes: "stable".to_string(),
            }]
        );
    }

    #[test]
    fn beta_fast_check_allows_prerelease_targets() {
        let report = collect_update_manifest_and_changelog_with_resolver(
            vec![release_with_manifest(
                "v1.2.9-beta.1",
                true,
                "https://example.invalid/1.2.9-beta.1.json",
                "### 更新日誌\nbeta",
            )],
            UpdateChannel::Beta,
            "1.2.8",
            manifest_resolver(&[(
                "https://example.invalid/1.2.9-beta.1.json",
                "1.2.9-beta.1",
                UpdateChannel::Beta,
            )]),
        )
        .expect("beta report should be collected");

        assert_eq!(report.manifest.version, "1.2.9-beta.1");
        assert_eq!(
            report.changelog,
            vec![UpdateChangelogEntry {
                version: "1.2.9-beta.1".to_string(),
                release_notes: "beta".to_string(),
            }]
        );
    }

    #[test]
    fn beta_fast_check_can_target_newer_stable_release() {
        let report = collect_update_manifest_and_changelog_with_resolver(
            vec![release_with_manifest(
                "v1.2.9",
                false,
                "https://example.invalid/1.2.9.json",
                "### 更新日誌\nstable",
            )],
            UpdateChannel::Beta,
            "1.2.8",
            manifest_resolver(&[(
                "https://example.invalid/1.2.9.json",
                "1.2.9",
                UpdateChannel::Stable,
            )]),
        )
        .expect("beta report should accept stable target");

        assert_eq!(report.manifest.version, "1.2.9");
    }

    #[test]
    fn update_changelog_allows_current_latest_manifest_without_update() {
        let report = collect_update_manifest_and_changelog_with_resolver(
            vec![release_with_manifest(
                "v1.1.8",
                false,
                "https://example.invalid/1.1.8.json",
                "### 更新日誌\ncurrent",
            )],
            UpdateChannel::Stable,
            "1.1.8",
            manifest_resolver(&[(
                "https://example.invalid/1.1.8.json",
                "1.1.8",
                UpdateChannel::Stable,
            )]),
        )
        .expect("current latest manifest should still produce an update report");

        assert_eq!(report.manifest.version, "1.1.8");
        assert!(report.changelog.is_empty());
    }

    #[test]
    fn update_changelog_requires_installable_manifest_target() {
        let error = collect_update_manifest_and_changelog_with_resolver(
            vec![release_without_manifest(
                "v1.2.8",
                false,
                "### 更新日誌\nnotes",
            )],
            UpdateChannel::Stable,
            "1.1.0",
            manifest_resolver(&[]),
        )
        .expect_err("update target should require manifest");

        assert_eq!(
            api_error_code(error).as_deref(),
            Some("update_manifest_missing")
        );
    }

    fn api_error_code(error: ApiError) -> Option<String> {
        serde_json::to_value(error)
            .ok()?
            .get("code")?
            .as_str()
            .map(str::to_string)
    }

    fn release_with_manifest(
        tag_name: &str,
        prerelease: bool,
        manifest_url: &str,
        body: &str,
    ) -> GithubRelease {
        github_release(tag_name, false, prerelease, Some(manifest_url), body)
    }

    fn draft_release_with_manifest(
        tag_name: &str,
        prerelease: bool,
        manifest_url: &str,
        body: &str,
    ) -> GithubRelease {
        github_release(tag_name, true, prerelease, Some(manifest_url), body)
    }

    fn release_without_manifest(tag_name: &str, prerelease: bool, body: &str) -> GithubRelease {
        github_release(tag_name, false, prerelease, None, body)
    }

    fn github_release(
        tag_name: &str,
        draft: bool,
        prerelease: bool,
        manifest_url: Option<&str>,
        body: &str,
    ) -> GithubRelease {
        GithubRelease {
            tag_name: tag_name.to_string(),
            draft,
            prerelease,
            body: Some(body.to_string()),
            assets: manifest_url
                .map(|url| {
                    vec![GithubAsset {
                        name: UPDATE_MANIFEST_ASSET.to_string(),
                        browser_download_url: url.to_string(),
                    }]
                })
                .unwrap_or_default(),
        }
    }

    fn manifest_resolver<'a>(
        manifests: &'a [(&'static str, &'static str, UpdateChannel)],
    ) -> impl FnMut(&str) -> Result<UpdateManifest, ApiError> + 'a {
        move |url| {
            manifests
                .iter()
                .find(|(manifest_url, _, _)| *manifest_url == url)
                .map(|(_, version, channel)| test_manifest(version, *channel))
                .ok_or_else(|| api_error_message("test_manifest_missing", url))
        }
    }

    fn test_manifest(version: &str, channel: UpdateChannel) -> UpdateManifest {
        UpdateManifest {
            schema: "nte-gacha-exporter-update".to_string(),
            schema_version: 1,
            version: version.to_string(),
            channel,
            release_url: format!("https://example.invalid/releases/{version}"),
            asset_name: format!("nte-gacha-exporter-{version}.zip"),
            download_url: format!("https://example.invalid/downloads/{version}.zip"),
            sha256: "0".repeat(64),
            size: 1,
        }
    }
}

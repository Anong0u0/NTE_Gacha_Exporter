use std::cell::Cell;
use std::io::Cursor;

use nte_core::{UpdateChangelogEntry, UpdateChannel, UpdateManifest};

use super::download::{copy_limited, read_text_limited};
use super::github::{
    collect_update_manifest_and_changelog_with_resolver, first_release_notes_section,
};
use super::types::{GithubAsset, GithubRelease, UPDATE_MANIFEST_ASSET};
use super::*;
use crate::error::api_error_message;

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

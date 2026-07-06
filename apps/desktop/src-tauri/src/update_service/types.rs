use nte_core::{UpdateChangelogEntry, UpdateManifest};
use semver::Version;
use serde::Deserialize;

pub(super) const GITHUB_RELEASES_API: &str =
    "https://api.github.com/repos/Anong0u0/nte_gacha_exporter/releases";
pub(super) const UPDATE_MANIFEST_ASSET: &str = "nte-gacha-exporter-update.json";
pub(super) const USER_AGENT: &str = "nte-gacha-exporter-updater";
pub(super) const MAX_UPDATE_JSON_BYTES: u64 = 1024 * 1024;
pub(super) const READ_BUFFER_BYTES: usize = 64 * 1024;
pub(super) const GITHUB_RELEASES_PAGE_SIZE: u32 = 100;

#[derive(Debug, Deserialize)]
pub(super) struct GithubRelease {
    pub tag_name: String,
    pub draft: bool,
    pub prerelease: bool,
    pub body: Option<String>,
    pub assets: Vec<GithubAsset>,
}

impl GithubRelease {
    pub(super) fn tag_version(&self) -> Option<Version> {
        Version::parse(normalize_version(&self.tag_name)).ok()
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug)]
pub(super) struct FetchedUpdateManifest {
    pub manifest: UpdateManifest,
    pub changelog: Vec<UpdateChangelogEntry>,
}

pub(super) fn normalize_version(value: &str) -> &str {
    value.trim().strip_prefix('v').unwrap_or(value.trim())
}

use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use semver::Version;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use nte_core::{
    GuiError, UpdateChannel, UpdateCheckReport, UpdateInstallPlan, UpdateManifest, UpdatePackage,
    UpdateStageReport, UpdateStatus,
};

const MANIFEST_SCHEMA: &str = "nte-gacha-exporter-update";
const MANIFEST_SCHEMA_VERSION: u32 = 1;
const ROOT_LAUNCHER: &str = "nte-gacha-exporter.exe";
const ROOT_CLI: &str = "nte-gacha-exporter-cli.exe";
const APP_DIR: &str = "app";
const LEGACY_SIDECAR_DIR: &str = "sidecars";
const DATA_DIR: &str = "data";
const UPDATE_DIR: &str = "update";
const APP_EXE: &str = "nte-gacha-exporter-desktop.exe";
const UPDATER_EXE: &str = "nte-gacha-exporter-updater.exe";
const RELEASE_JSON: &str = "release.json";
const RELEASE_SCHEMA: &str = "nte-gacha-exporter-release";
const RELEASE_SCHEMA_VERSION: u32 = 1;

include!("updater/api.rs");
include!("updater/manifest.rs");
include!("updater/archive.rs");
include!("updater/payload.rs");
include!("updater/filesystem.rs");

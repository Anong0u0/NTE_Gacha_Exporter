use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nte_assets::{read_zip_manifest, validate_manifest_shape};
use nte_core::{
    AssetsPackCheckReport, AssetsPackInstallReport, AssetsPackManifest, AssetsPackPackage,
    AssetsPackStatus, GuiError, UpdateChannel, bundled_maps_hash,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::error::{ApiError, api_error, api_error_message};
use crate::state::{AppState, with_store};

const GITHUB_RELEASES_API: &str =
    "https://api.github.com/repos/Anong0u0/nte_gacha_exporter/releases";
const ASSETS_PACK_MANIFEST_ASSET: &str = "nte-assets-pack-manifest.json";
const USER_AGENT: &str = "nte-gacha-exporter-assets-pack";
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(60);
const HTTP_WRITE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RELEASE_JSON_BYTES: u64 = 1024 * 1024;
const READ_BUFFER_BYTES: usize = 64 * 1024;

include!("assets_commands/types.rs");
include!("assets_commands/commands.rs");
include!("assets_commands/status.rs");
include!("assets_commands/fetch.rs");
include!("assets_commands/install.rs");
include!("assets_commands/paths.rs");
include!("assets_commands/io.rs");

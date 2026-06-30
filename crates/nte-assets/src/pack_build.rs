use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Seek, Write};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::Command;

use image::{GenericImageView, imageops};
use nte_core::{AssetsPackAsset, AssetsPackManifest, GuiError, bundled_maps_hash};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zip::{ZipWriter, write::FileOptions};

const PACK_SCHEMA: &str = "nte-gacha-exporter-assets-pack";
const PACK_SCHEMA_VERSION: u32 = 1;
const SOURCE_REPO: &str = "https://github.com/Waifus-Grace/NTE_Assets";
pub const DEFAULT_WEBP_QUALITY: u8 = 82;

#[derive(Debug, Clone)]
pub struct AssetPackBuildOptions {
    pub assets_root: PathBuf,
    pub maps_dir: PathBuf,
    pub out_path: PathBuf,
    pub app_version: String,
    pub webp_quality: u8,
}

#[derive(Debug, Clone)]
pub struct AssetPackBuild {
    pub out_path: PathBuf,
    pub manifest: AssetsPackManifest,
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AssetRefUse {
    asset_ref: String,
    kind: String,
    source_path: String,
    max_edge: u32,
}

#[derive(Debug, Clone)]
struct EncodedAssetUse {
    pack_path: String,
    width: u32,
    height: u32,
    sha256: String,
}

include!("pack_build/build.rs");
include!("pack_build/refs.rs");
include!("pack_build/encode.rs");
include!("pack_build/manifest.rs");
include!("pack_build/tests.rs");

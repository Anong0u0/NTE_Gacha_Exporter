use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Seek, Write};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::Command;

use image::{GenericImageView, imageops};
use nte_core::{
    ASSETS_PACK_SCHEMA, ASSETS_PACK_SCHEMA_VERSION, AssetsPackAsset, AssetsPackManifest, GuiError,
    validate_assets_pack_manifest,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zip::{ZipWriter, write::FileOptions};

pub const DEFAULT_WEBP_QUALITY: u8 = 82;

#[derive(Debug, Clone)]
pub struct AssetPackSource {
    pub repo: String,
    pub commit: String,
}

#[derive(Debug, Clone)]
pub struct AssetPackBuildOptions {
    pub assets_root: PathBuf,
    pub maps_dir: PathBuf,
    pub out_path: PathBuf,
    pub app_version: String,
    pub webp_quality: u8,
    pub source: AssetPackSource,
}

#[derive(Debug, Clone)]
pub struct AssetPackBuild {
    pub out_path: PathBuf,
    pub manifest: AssetsPackManifest,
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetPackRef {
    pub asset_ref: String,
    pub kind: String,
    pub source_path: String,
    pub max_edge: u32,
}

#[derive(Debug, Clone)]
struct EncodedAssetUse {
    pack_path: String,
    width: u32,
    height: u32,
}

include!("pack_build/build.rs");
include!("pack_build/refs.rs");
include!("pack_build/encode.rs");
include!("pack_build/manifest.rs");
include!("pack_build/tests.rs");

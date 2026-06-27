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

pub fn build_assets_pack(options: &AssetPackBuildOptions) -> Result<AssetPackBuild, GuiError> {
    if options.out_path.exists() {
        return Err(invalid_pack(format!(
            "assets pack output already exists: {}",
            options.out_path.display()
        )));
    }
    if !(1..=100).contains(&options.webp_quality) {
        return Err(invalid_pack("webp quality must be between 1 and 100"));
    }
    let source_commit = source_commit_from_git_head(&options.assets_root)?;

    let refs = collect_asset_ref_uses(&options.maps_dir)?;
    let missing = refs
        .iter()
        .filter(|asset| !options.assets_root.join(&asset.source_path).is_file())
        .map(|asset| format!("{} -> {}", asset.asset_ref, asset.source_path))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(invalid_pack(format!(
            "assets pack source files missing: {}",
            missing.join(", ")
        )));
    }

    if let Some(parent) = options.out_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(&options.out_path)?;
    let mut zip = ZipWriter::new(file);
    let options_zip = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut manifest_assets = Vec::new();
    let mut used_pack_paths = BTreeSet::new();
    let mut encoded_paths: BTreeMap<String, String> = BTreeMap::new();
    let mut encoded_refs: BTreeMap<String, EncodedAssetUse> = BTreeMap::new();

    for asset in refs {
        let encoded = if let Some(encoded) = encoded_refs.get(&asset.asset_ref) {
            encoded.clone()
        } else {
            let source = options.assets_root.join(&asset.source_path);
            let encoded =
                encode_asset_webp(&source, asset.max_edge, f32::from(options.webp_quality))?;
            let sha256 = sha256_bytes(&encoded.bytes);
            let pack_path = if let Some(path) = encoded_paths.get(&sha256) {
                path.clone()
            } else {
                let path = unique_pack_path(&sha256, &mut used_pack_paths);
                zip.start_file(&path, options_zip)?;
                zip.write_all(&encoded.bytes)?;
                encoded_paths.insert(sha256.clone(), path.clone());
                path
            };
            let encoded = EncodedAssetUse {
                pack_path,
                width: encoded.width,
                height: encoded.height,
                sha256,
            };
            encoded_refs.insert(asset.asset_ref.clone(), encoded.clone());
            encoded
        };
        manifest_assets.push(AssetsPackAsset {
            asset_ref: asset.asset_ref,
            kind: asset.kind,
            source_path: asset.source_path,
            pack_path: encoded.pack_path,
            width: encoded.width,
            height: encoded.height,
            sha256: encoded.sha256,
        });
    }

    let manifest = AssetsPackManifest {
        schema: PACK_SCHEMA.to_string(),
        schema_version: PACK_SCHEMA_VERSION,
        app_version: options.app_version.clone(),
        map_hash: bundled_maps_hash(),
        source_repo: SOURCE_REPO.to_string(),
        source_commit,
        format: "webp".to_string(),
        quality: options.webp_quality,
        file_count: manifest_assets.len() as u64,
        assets: manifest_assets,
    };
    zip.start_file("manifest.json", options_zip)?;
    zip.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    zip.write_all(b"\n")?;
    zip.finish()?;

    Ok(AssetPackBuild {
        out_path: options.out_path.clone(),
        manifest,
        missing: Vec::new(),
    })
}

fn source_commit_from_git_head(assets_root: &Path) -> Result<String, GuiError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(assets_root)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map_err(|error| {
            invalid_pack(format!(
                "failed to read NTE_Assets git HEAD at {}: {error}",
                assets_root.display()
            ))
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        let suffix = if detail.is_empty() {
            String::new()
        } else {
            format!(": {detail}")
        };
        return Err(invalid_pack(format!(
            "failed to read NTE_Assets git HEAD at {}{suffix}",
            assets_root.display()
        )));
    }

    let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if commit.is_empty() {
        return Err(invalid_pack(format!(
            "NTE_Assets git HEAD is empty at {}",
            assets_root.display()
        )));
    }
    Ok(commit)
}

pub fn normalize_asset_ref(asset_ref: &str) -> Option<String> {
    let trimmed = asset_ref.trim();
    let rel = trimmed
        .strip_prefix("/Game/UI/UI_Icon/")
        .map(|rest| format!("UI_Icon/{rest}"))
        .or_else(|| {
            trimmed
                .strip_prefix("/Game/UI/UI/")
                .map(|rest| format!("UI/{rest}"))
        })?;
    let (prefix, leaf) = rel
        .rsplit_once('/')
        .map_or(("", rel.as_str()), |(prefix, leaf)| (prefix, leaf));
    let stem = leaf.split('.').next().filter(|value| !value.is_empty())?;
    let path = if prefix.is_empty() {
        format!("{stem}.png")
    } else {
        format!("{prefix}/{stem}.png")
    };
    (!path.contains('\\') && !path.contains("..")).then_some(path)
}

fn collect_asset_ref_uses(maps_dir: &Path) -> Result<Vec<AssetRefUse>, GuiError> {
    let mut refs: BTreeMap<(String, String), AssetRefUse> = BTreeMap::new();
    for entry in fs::read_dir(maps_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let text = fs::read_to_string(entry.path())?;
        let value: Value = serde_json::from_str(&text)?;
        collect_asset_refs_from_value(&value, &mut refs);
    }
    let max_edges_by_ref = refs
        .values()
        .map(|asset| (asset.asset_ref.clone(), asset.max_edge))
        .fold(
            BTreeMap::<String, u32>::new(),
            |mut max_edges, (asset_ref, max_edge)| {
                max_edges
                    .entry(asset_ref)
                    .and_modify(|current| *current = (*current).max(max_edge))
                    .or_insert(max_edge);
                max_edges
            },
        );
    Ok(refs
        .into_values()
        .map(|mut asset| {
            if let Some(max_edge) = max_edges_by_ref.get(&asset.asset_ref) {
                asset.max_edge = *max_edge;
            }
            asset
        })
        .collect())
}

fn collect_asset_refs_from_value(
    value: &Value,
    refs: &mut BTreeMap<(String, String), AssetRefUse>,
) {
    match value {
        Value::Object(object) => {
            if let Some(asset_refs) = object.get("asset_refs") {
                collect_asset_refs_from_asset_refs(asset_refs, None, refs);
            }
            for (key, value) in object {
                if key != "asset_refs" {
                    collect_asset_refs_from_value(value, refs);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_asset_refs_from_value(value, refs);
            }
        }
        _ => {}
    }
}

fn collect_asset_refs_from_asset_refs(
    value: &Value,
    current_key: Option<&str>,
    refs: &mut BTreeMap<(String, String), AssetRefUse>,
) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                collect_asset_refs_from_asset_refs(value, Some(key), refs);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_asset_refs_from_asset_refs(value, current_key, refs);
            }
        }
        Value::String(asset_ref) => {
            let Some(source_path) = normalize_asset_ref(asset_ref) else {
                return;
            };
            let kind = current_key.unwrap_or("asset").to_string();
            let max_edge = max_edge_for_kind(&kind);
            refs.entry((asset_ref.clone(), kind.clone()))
                .or_insert_with(|| AssetRefUse {
                    asset_ref: asset_ref.clone(),
                    kind,
                    source_path,
                    max_edge,
                });
        }
        _ => {}
    }
}

fn max_edge_for_kind(kind: &str) -> u32 {
    match kind {
        "icon" => 256,
        "head_icon" => 128,
        "portrait" | "featured_portraits" => 512,
        "image" | "background" | "banner" => 768,
        _ => 512,
    }
}

struct EncodedAsset {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
}

fn encode_asset_webp(path: &Path, max_edge: u32, quality: f32) -> Result<EncodedAsset, GuiError> {
    let image = image::open(path).map_err(|error| invalid_pack(format!("{path:?}: {error}")))?;
    let (source_width, source_height) = image.dimensions();
    let scale = if source_width <= max_edge && source_height <= max_edge {
        1.0
    } else {
        f64::from(max_edge) / f64::from(source_width.max(source_height))
    };
    let width = scaled_dimension(source_width, scale);
    let height = scaled_dimension(source_height, scale);
    let rgba = if width == source_width && height == source_height {
        image.to_rgba8()
    } else {
        imageops::resize(
            &image.to_rgba8(),
            width,
            height,
            imageops::FilterType::Lanczos3,
        )
    };
    let encoder = webp::Encoder::from_rgba(rgba.as_raw(), width, height);
    let bytes = encoder.encode(quality).deref().to_vec();
    Ok(EncodedAsset {
        bytes,
        width,
        height,
    })
}

fn scaled_dimension(value: u32, scale: f64) -> u32 {
    ((f64::from(value) * scale).round() as u32).max(1)
}

fn unique_pack_path(sha256: &str, used: &mut BTreeSet<String>) -> String {
    for len in [16_usize, 24, 32, 40, 64] {
        let path = format!("assets/{}.webp", &sha256[..len]);
        if used.insert(path.clone()) {
            return path;
        }
    }
    unreachable!("sha256 must produce a unique path")
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn invalid_pack(message: impl Into<String>) -> GuiError {
    GuiError::InvalidAssetsPack(message.into())
}

pub fn read_zip_manifest<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
) -> Result<AssetsPackManifest, GuiError> {
    let mut entry = zip
        .by_name("manifest.json")
        .map_err(|_| invalid_pack("assets pack missing manifest.json"))?;
    let mut text = String::new();
    entry.read_to_string(&mut text)?;
    let manifest: AssetsPackManifest = serde_json::from_str(&text)?;
    validate_manifest_shape(&manifest)?;
    Ok(manifest)
}

pub fn validate_manifest_shape(manifest: &AssetsPackManifest) -> Result<(), GuiError> {
    if manifest.schema != PACK_SCHEMA || manifest.schema_version != PACK_SCHEMA_VERSION {
        return Err(invalid_pack(
            "assets pack manifest schema must be nte-gacha-exporter-assets-pack v1",
        ));
    }
    if manifest.format != "webp" {
        return Err(invalid_pack("assets pack format must be webp"));
    }
    if manifest.file_count != manifest.assets.len() as u64 {
        return Err(invalid_pack("assets pack file_count mismatch"));
    }
    for asset in &manifest.assets {
        if asset.asset_ref.trim().is_empty()
            || asset.kind.trim().is_empty()
            || asset.source_path.trim().is_empty()
            || asset.pack_path.trim().is_empty()
        {
            return Err(invalid_pack("assets pack asset fields must be non-empty"));
        }
        if !asset.pack_path.starts_with("assets/")
            || !asset.pack_path.ends_with(".webp")
            || asset.pack_path.contains('\\')
            || asset.pack_path.contains("..")
        {
            return Err(invalid_pack(format!(
                "assets pack contains invalid asset path: {}",
                asset.pack_path
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_asset_ref_maps_unreal_ui_paths_to_png() {
        assert_eq!(
            normalize_asset_ref("/Game/UI/UI_Icon/Fork/1024/fork_Rose.fork_Rose").as_deref(),
            Some("UI_Icon/Fork/1024/fork_Rose.png")
        );
        assert_eq!(
            normalize_asset_ref(
                "/Game/UI/UI/Gacha/YH_lihui_character_anhunqu.YH_lihui_character_anhunqu"
            )
            .as_deref(),
            Some("UI/Gacha/YH_lihui_character_anhunqu.png")
        );
        assert!(normalize_asset_ref("/Game/Other/path.asset").is_none());
    }

    #[test]
    fn manifest_shape_rejects_invalid_pack_paths() {
        let manifest = AssetsPackManifest {
            schema: PACK_SCHEMA.to_string(),
            schema_version: PACK_SCHEMA_VERSION,
            app_version: "0.1.0".to_string(),
            map_hash: "hash".to_string(),
            source_repo: SOURCE_REPO.to_string(),
            source_commit: "commit".to_string(),
            format: "webp".to_string(),
            quality: DEFAULT_WEBP_QUALITY,
            file_count: 1,
            assets: vec![AssetsPackAsset {
                asset_ref: "/Game/UI/UI_Icon/Fork/1024/fork_Rose.fork_Rose".to_string(),
                kind: "icon".to_string(),
                source_path: "UI_Icon/Fork/1024/fork_Rose.png".to_string(),
                pack_path: "../bad.webp".to_string(),
                width: 1,
                height: 1,
                sha256: "hash".to_string(),
            }],
        };

        assert!(validate_manifest_shape(&manifest).is_err());
    }

    #[test]
    fn build_assets_pack_writes_manifest_and_webp_assets() {
        let temp = tempfile::tempdir().unwrap();
        let assets_root = temp.path().join("assets");
        let maps_dir = temp.path().join("maps");
        fs::create_dir_all(assets_root.join("UI_Icon/Fork/1024")).unwrap();
        fs::create_dir_all(assets_root.join("UI/Gacha")).unwrap();
        fs::create_dir_all(assets_root.join("UI/PlayerInfo/BusinessCards/Card_Small")).unwrap();
        fs::create_dir_all(&maps_dir).unwrap();
        let image = image::RgbaImage::from_pixel(512, 512, image::Rgba([255, 0, 0, 255]));
        image
            .save(assets_root.join("UI_Icon/Fork/1024/fork_Rose.png"))
            .unwrap();
        image
            .save(assets_root.join("UI_Icon/Fork/1024/fork_Small.png"))
            .unwrap();
        image.save(assets_root.join("UI/Gacha/shared.png")).unwrap();
        image
            .save(
                assets_root.join(
                    "UI/PlayerInfo/BusinessCards/Card_Small/YH_UI_bg_card_show_strip_08_s.png",
                ),
            )
            .unwrap();
        fs::write(
            maps_dir.join("en.json"),
            r#"{
              "items": {
                "rose": {
                  "asset_refs": {
                    "icon": "/Game/UI/UI_Icon/Fork/1024/fork_Rose.fork_Rose",
                    "head_icon": "/Game/UI/UI_Icon/Fork/1024/fork_Rose.fork_Rose",
                    "portrait": "/Game/UI/UI/Gacha/shared.shared"
                  }
                },
                "small": {
                  "asset_refs": {
                    "head_icon": "/Game/UI/UI_Icon/Fork/1024/fork_Small.fork_Small"
                  }
                },
                "business_card": {
                  "asset_refs": {
                    "banner": "/Game/UI/UI/PlayerInfo/BusinessCards/Card_Small/YH_UI_bg_card_show_strip_08_s.YH_UI_bg_card_show_strip_08_s"
                  }
                }
              },
              "banners": {
                "rose": {
                  "asset_refs": {
                    "image": "/Game/UI/UI/Gacha/shared.shared"
                  }
                }
              }
            }"#,
        )
        .unwrap();
        let source_commit = init_git_head(&assets_root);

        let out_path = temp.path().join("pack.zip");
        let build = build_assets_pack(&AssetPackBuildOptions {
            assets_root,
            maps_dir,
            out_path: out_path.clone(),
            app_version: "0.1.0".to_string(),
            webp_quality: DEFAULT_WEBP_QUALITY,
        })
        .unwrap();

        assert_eq!(build.manifest.file_count, 6);
        assert_eq!(build.manifest.source_commit, source_commit);
        let mut zip = zip::ZipArchive::new(fs::File::open(out_path).unwrap()).unwrap();
        let manifest = read_zip_manifest(&mut zip).unwrap();
        assert_eq!(manifest.assets.len(), 6);
        assert_eq!(manifest.source_commit, source_commit);
        let icon = manifest
            .assets
            .iter()
            .find(|asset| asset.kind == "icon")
            .unwrap();
        let head_icon = manifest
            .assets
            .iter()
            .find(|asset| asset.kind == "head_icon")
            .unwrap();
        let standalone_head_icon = manifest
            .assets
            .iter()
            .find(|asset| asset.asset_ref.contains("fork_Small"))
            .unwrap();
        let image = manifest
            .assets
            .iter()
            .find(|asset| asset.kind == "image")
            .unwrap();
        let portrait = manifest
            .assets
            .iter()
            .find(|asset| asset.kind == "portrait")
            .unwrap();
        let card = manifest
            .assets
            .iter()
            .find(|asset| asset.asset_ref.contains("YH_UI_bg_card_show_strip_08_s"))
            .unwrap();
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!((head_icon.width, head_icon.height), (256, 256));
        assert_eq!(head_icon.pack_path, icon.pack_path);
        assert_eq!(
            (standalone_head_icon.width, standalone_head_icon.height),
            (128, 128)
        );
        assert_eq!((image.width, image.height), (512, 512));
        assert_eq!((portrait.width, portrait.height), (512, 512));
        assert_eq!(portrait.pack_path, image.pack_path);
        assert_eq!(card.kind, "banner");
        assert_eq!(
            card.source_path,
            "UI/PlayerInfo/BusinessCards/Card_Small/YH_UI_bg_card_show_strip_08_s.png"
        );
        assert!(zip.by_name(&icon.pack_path).is_ok());
        assert!(zip.by_name(&image.pack_path).is_ok());
        assert!(zip.by_name(&card.pack_path).is_ok());
    }

    fn init_git_head(root: &Path) -> String {
        run_git(root, &["init"]);
        run_git(root, &["config", "user.email", "nte-test@example.invalid"]);
        run_git(root, &["config", "user.name", "NTE Test"]);
        run_git(root, &["commit", "--allow-empty", "-m", "test assets"]);
        git_stdout(root, &["rev-parse", "HEAD"])
    }

    fn run_git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_stdout(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }
}

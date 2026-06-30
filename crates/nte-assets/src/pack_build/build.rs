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

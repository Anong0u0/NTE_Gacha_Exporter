fn fetch_assets_pack_package(channel: UpdateChannel) -> Result<AssetsPackPackage, ApiError> {
    if let Ok(source) = std::env::var("NTE_GACHA_EXPORTER_ASSETS_PACK_MANIFEST") {
        if !source.trim().is_empty() {
            if source.starts_with("http://") || source.starts_with("https://") {
                return http_get_json(&source);
            }
            let file = fs::File::open(source).map_err(api_error)?;
            let text = read_text_limited(file, MAX_RELEASE_JSON_BYTES, "assets pack manifest")?;
            return serde_json::from_str(&text).map_err(api_error);
        }
    }
    let release = select_release(channel)?;
    let manifest_url = release
        .assets
        .into_iter()
        .find(|asset| asset.name == ASSETS_PACK_MANIFEST_ASSET)
        .map(|asset| asset.browser_download_url)
        .ok_or_else(|| {
            api_error_message(
                "assets_pack_manifest_missing",
                "release assets pack manifest missing",
            )
        })?;
    http_get_json(&manifest_url)
}

fn select_release(channel: UpdateChannel) -> Result<GithubRelease, ApiError> {
    let releases: Vec<GithubRelease> = http_get_json(GITHUB_RELEASES_API)?;
    releases
        .into_iter()
        .find(|release| !release.draft && (channel == UpdateChannel::Beta || !release.prerelease))
        .ok_or_else(|| {
            api_error_message(
                "assets_pack_release_missing",
                "no matching GitHub release found",
            )
        })
}

fn http_get_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, ApiError> {
    let response = http_agent()
        .get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(api_error)?;
    let text = read_text_limited(
        response.into_reader(),
        MAX_RELEASE_JSON_BYTES,
        "assets pack JSON response",
    )?;
    serde_json::from_str(&text).map_err(api_error)
}

fn download_assets_pack(root: &Path, package: &AssetsPackPackage) -> Result<PathBuf, ApiError> {
    let downloads = assets_pack_root(root).join("downloads").join(format!(
        "{}-{}",
        package.app_version,
        short_hash(&package.map_hash)
    ));
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
                "assets_pack_size_mismatch",
                format!(
                    "assets pack content length mismatch: expected {}, got {size}",
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
    if path.exists() {
        fs::remove_file(&path).map_err(api_error)?;
    }
    if let Err(error) = fs::rename(&tmp_path, &path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(api_error(error));
    }
    Ok(path)
}

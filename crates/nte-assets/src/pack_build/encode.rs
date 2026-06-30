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

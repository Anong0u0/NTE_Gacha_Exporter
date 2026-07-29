pub fn maps_hash_from_dir(maps_dir: &Path) -> Result<String, GuiError> {
    let mut maps = Vec::new();
    for entry in fs::read_dir(maps_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let locale = entry
            .path()
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| invalid_pack("assets map filename must be valid UTF-8"))?
            .to_string();
        maps.push((locale, fs::read(entry.path())?));
    }
    if maps.is_empty() {
        return Err(invalid_pack(format!(
            "assets maps directory has no JSON files: {}",
            maps_dir.display()
        )));
    }
    maps.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = Sha256::new();
    for (locale, bytes) in maps {
        hasher.update(locale.as_bytes());
        hasher.update([0]);
        hasher.update(bytes);
        hasher.update([0]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

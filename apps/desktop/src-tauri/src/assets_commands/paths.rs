fn resolve_protocol_path(root: &Path, uri_path: &str) -> Result<PathBuf, String> {
    let path = uri_path.trim_start_matches('/');
    if path.contains("..")
        || path.contains('\\')
        || path.contains(':')
        || !path.starts_with("assets/")
        || !path.ends_with(".webp")
    {
        return Err("invalid asset path".to_string());
    }
    Ok(current_dir(root).join(path))
}

fn asset_url(pack_path: &str) -> String {
    if cfg!(windows) {
        format!("http://nteasset.localhost/{pack_path}")
    } else {
        format!("nteasset://localhost/{pack_path}")
    }
}

fn assets_pack_root(root: &Path) -> PathBuf {
    root.join("data").join("assets-pack")
}

fn current_dir(root: &Path) -> PathBuf {
    assets_pack_root(root).join("current")
}

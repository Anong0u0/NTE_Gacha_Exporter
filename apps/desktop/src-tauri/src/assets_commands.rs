use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use nte_core::{AssetsPackManifest, validate_assets_pack_manifest};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{ApiError, api_error};
use crate::state::{AppState, with_store};

include!("assets_commands/types.rs");
include!("assets_commands/commands.rs");
include!("assets_commands/status.rs");
include!("assets_commands/paths.rs");
include!("assets_commands/io.rs");

#[cfg(test)]
mod assets_manifest_tests {
    use super::*;
    use nte_core::{ASSETS_PACK_SCHEMA, ASSETS_PACK_SCHEMA_VERSION, AssetsPackAsset};

    fn write_manifest(root: &Path, schema_version: u32) {
        let dir = current_dir(root);
        fs::create_dir_all(&dir).unwrap();
        let manifest = AssetsPackManifest {
            schema: ASSETS_PACK_SCHEMA.to_string(),
            schema_version,
            app_version: "0.1.0".to_string(),
            source_repo: "https://example.invalid/assets".to_string(),
            source_commit: "commit".to_string(),
            format: "webp".to_string(),
            quality: 82,
            file_count: 1,
            assets: vec![AssetsPackAsset {
                asset_ref: "/Game/UI/example.example".to_string(),
                kind: "icon".to_string(),
                source_path: "UI/example.png".to_string(),
                pack_path: "assets/example.webp".to_string(),
                width: 1,
                height: 1,
            }],
        };
        fs::write(
            dir.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn bundled_manifest_accepts_v2_and_rejects_v1() {
        let temp = tempfile::tempdir().unwrap();
        write_manifest(temp.path(), ASSETS_PACK_SCHEMA_VERSION);
        assert!(read_bundled_manifest(temp.path()).is_ok());

        write_manifest(temp.path(), 1);
        assert!(read_bundled_manifest(temp.path()).is_err());
    }
}

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
            schema: ASSETS_PACK_SCHEMA.to_string(),
            schema_version: ASSETS_PACK_SCHEMA_VERSION,
            app_version: "0.1.0".to_string(),
            source_repo: "https://example.invalid/assets".to_string(),
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
            }],
        };

        assert!(validate_assets_pack_manifest(&manifest).is_err());
    }

    #[test]
    fn manifest_shape_accepts_only_schema_v2() {
        let mut manifest = AssetsPackManifest {
            schema: ASSETS_PACK_SCHEMA.to_string(),
            schema_version: ASSETS_PACK_SCHEMA_VERSION,
            app_version: "0.1.0".to_string(),
            source_repo: "https://example.invalid/assets".to_string(),
            source_commit: "commit".to_string(),
            format: "webp".to_string(),
            quality: DEFAULT_WEBP_QUALITY,
            file_count: 0,
            assets: Vec::new(),
        };

        validate_assets_pack_manifest(&manifest).unwrap();
        manifest.schema_version = 1;
        assert!(validate_assets_pack_manifest(&manifest).is_err());
        manifest.schema_version = ASSETS_PACK_SCHEMA_VERSION + 1;
        assert!(validate_assets_pack_manifest(&manifest).is_err());
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
            maps_dir.join("core.json"),
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
        let source = AssetPackSource {
            repo: "https://example.invalid/assets".to_string(),
            commit: "test-source".to_string(),
        };
        let out_path = temp.path().join("pack.zip");
        let build = build_assets_pack(&AssetPackBuildOptions {
            assets_root,
            maps_dir,
            out_path: out_path.clone(),
            app_version: "0.1.0".to_string(),
            webp_quality: DEFAULT_WEBP_QUALITY,
            source,
        })
        .unwrap();

        assert_eq!(build.manifest.file_count, 6);
        assert_eq!(build.manifest.schema, ASSETS_PACK_SCHEMA);
        assert_eq!(build.manifest.schema_version, ASSETS_PACK_SCHEMA_VERSION);
        assert_eq!(build.manifest.source_commit, "test-source");
        let mut zip = zip::ZipArchive::new(fs::File::open(out_path).unwrap()).unwrap();
        let manifest = read_zip_manifest(&mut zip).unwrap();
        assert_eq!(manifest.assets.len(), 6);
        assert_eq!(manifest.source_commit, "test-source");
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

}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{AppDatabase, ItemAlias, ItemMeta, PoolRule, RecordFilter};

    fn sample_document() -> String {
        json!({
            "info": {
                "schema": "nte-gacha-export",
                "schema_version": "1.0"
            },
            "nte": {
                "list": [
                    {
                        "record_id": "Character:1",
                        "record_type": "gacha",
                        "time": "2026-01-01 10:00:00",
                        "pool_id": "CardPool_Character",
                        "pool_name": "Limited",
                        "item_id": "item_3",
                        "item_name": "Common A",
                        "count": 1,
                        "roll_points": 1,
                        "roll_label": "1"
                    },
                    {
                        "record_id": "Character:2",
                        "record_type": "gacha",
                        "time": "2026-01-01 10:01:00",
                        "pool_id": "CardPool_Character",
                        "pool_name": "Limited",
                        "item_id": "item_5",
                        "item_name": "Rare A",
                        "count": 1,
                        "roll_points": 2,
                        "roll_label": "2"
                    },
                    {
                        "record_id": "Weapon:1",
                        "record_type": "gacha",
                        "time": "2026-01-02 10:00:00",
                        "pool_id": "CardPool_Weapon",
                        "pool_name": "Weapon",
                        "item_id": "item_weapon",
                        "item_name": "Blade",
                        "count": 1,
                        "roll_points": 1,
                        "roll_label": "1"
                    }
                ]
            }
        })
        .to_string()
    }

    #[test]
    fn migrations_are_idempotent_and_create_default_profile() {
        let db = AppDatabase::open_in_memory().unwrap();
        db.migrate().unwrap();
        db.migrate().unwrap();

        let profiles = db.list_profiles().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "Default");
    }

    #[test]
    fn duplicate_import_is_skipped() {
        let mut db = AppDatabase::open_in_memory().unwrap();
        let profile = db.ensure_default_profile().unwrap();

        let first = db
            .import_public_document(profile.id, &sample_document(), "json", Some("sample.json"))
            .unwrap();
        let second = db
            .import_public_document(profile.id, &sample_document(), "json", Some("sample.json"))
            .unwrap();

        assert_eq!(first.records_seen, 3);
        assert_eq!(first.records_inserted, 3);
        assert_eq!(first.records_skipped, 0);
        assert_eq!(second.records_inserted, 0);
        assert_eq!(second.records_skipped, 3);
    }

    #[test]
    fn dashboard_counts_records_and_known_pity() {
        let mut db = AppDatabase::open_in_memory().unwrap();
        let profile = db.ensure_default_profile().unwrap();
        db.upsert_rules(
            &[PoolRule {
                pool_id: "CardPool_Character".to_string(),
                pool_name: "Limited".to_string(),
                group_label: "Limited".to_string(),
                pickup_item_ids: None,
            }],
            &[ItemMeta {
                item_id: "item_5".to_string(),
                item_name: "Rare A".to_string(),
                rarity: 5,
                category: Some("character".to_string()),
            }],
            &[],
        )
        .unwrap();
        db.import_public_document(profile.id, &sample_document(), "json", None)
            .unwrap();

        let summary = db.dashboard_summary(profile.id).unwrap();

        assert_eq!(summary.total_records, 3);
        assert_eq!(summary.timeline.len(), 2);
        let character = summary
            .pools
            .iter()
            .find(|pool| pool.pool_id == "CardPool_Character")
            .unwrap();
        assert_eq!(character.record_count, 2);
        assert_eq!(character.hit_count, 1);
        assert_eq!(character.current_pity, Some(0));
    }

    #[test]
    fn dashboard_counts_alias_item_as_known_pity_hit() {
        let mut db = AppDatabase::open_in_memory().unwrap();
        let profile = db.ensure_default_profile().unwrap();
        db.upsert_rules(
            &[PoolRule {
                pool_id: "CardPool_Character".to_string(),
                pool_name: "Limited".to_string(),
                group_label: "Limited".to_string(),
                pickup_item_ids: None,
            }],
            &[ItemMeta {
                item_id: "item_5".to_string(),
                item_name: "Rare A".to_string(),
                rarity: 5,
                category: Some("character".to_string()),
            }],
            &[ItemAlias {
                alias_id: "item_5_alias".to_string(),
                item_id: "item_5".to_string(),
            }],
        )
        .unwrap();
        let document = json!({
            "info": {
                "schema": "nte-gacha-export",
                "schema_version": "1.0"
            },
            "nte": {
                "list": [
                    {
                        "record_id": "Character:alias",
                        "record_type": "gacha",
                        "time": "2026-01-01 10:00:00",
                        "pool_id": "CardPool_Character",
                        "pool_name": "Limited",
                        "item_id": "item_5_alias",
                        "item_name": "Rare A",
                        "count": 1,
                        "roll_points": 1,
                        "roll_label": "1"
                    },
                    {
                        "record_id": "Character:common",
                        "record_type": "gacha",
                        "time": "2026-01-01 10:01:00",
                        "pool_id": "CardPool_Character",
                        "pool_name": "Limited",
                        "item_id": "item_3",
                        "item_name": "Common A",
                        "count": 1,
                        "roll_points": 2,
                        "roll_label": "2"
                    }
                ]
            }
        })
        .to_string();
        db.import_public_document(profile.id, &document, "json", None)
            .unwrap();

        let summary = db.dashboard_summary(profile.id).unwrap();

        let character = summary
            .pools
            .iter()
            .find(|pool| pool.pool_id == "CardPool_Character")
            .unwrap();
        assert_eq!(character.record_count, 2);
        assert_eq!(character.hit_count, 1);
        assert_eq!(character.current_pity, Some(1));
    }

    #[test]
    fn pool_rule_deserializes_optional_pickup_item_ids() {
        let rule: PoolRule = serde_json::from_value(json!({
            "pool_id": "ForkLottery_AnHunQu",
            "pool_name": "Miracle Box",
            "group_label": "Arc Research",
            "pickup_item_ids": ["fork_Rose"]
        }))
        .unwrap();

        assert_eq!(rule.pickup_item_ids, Some(vec!["fork_Rose".to_string()]));
    }

    #[test]
    fn export_json_and_csv_include_imported_records() {
        let mut db = AppDatabase::open_in_memory().unwrap();
        let profile = db.ensure_default_profile().unwrap();
        db.import_public_document(profile.id, &sample_document(), "json", None)
            .unwrap();

        let json_text = db.export_json(profile.id).unwrap();
        let csv_text = db.export_csv(profile.id).unwrap();
        let list = db
            .list_records(
                profile.id,
                &RecordFilter {
                    search: Some("Rare".to_string()),
                    ..RecordFilter::default()
                },
            )
            .unwrap();

        assert!(json_text.contains("\"record_id\": \"Character:1\""));
        assert!(csv_text.contains("Rare A"));
        assert_eq!(list.total, 1);
        assert_eq!(list.records[0].item_name.as_deref(), Some("Rare A"));
    }
}

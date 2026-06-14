use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::types::Value as SqlValue;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Transaction};
use serde_json::{json, Map, Value};

use crate::export::{csv_export, json_export};
use crate::model::{
    DashboardSummary, GuiError, ImportReport, ItemAlias, ItemMeta, LatestRecord, PoolRule,
    PoolSummary, Profile, RecordFilter, RecordList, StoredRecord, TimelineBucket, TypeSummary,
};

const DEFAULT_PROFILE_NAME: &str = "Default";

pub struct AppDatabase {
    conn: Connection,
}

impl AppDatabase {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, GuiError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self, GuiError> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn migrate(&self) -> Result<(), GuiError> {
        self.conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
                record_id TEXT NOT NULL,
                record_type TEXT NOT NULL,
                time TEXT,
                pool_id TEXT,
                pool_name TEXT,
                item_id TEXT NOT NULL,
                item_name TEXT,
                count INTEGER,
                roll_points INTEGER,
                roll_label TEXT,
                secondary_item_id TEXT,
                secondary_item_name TEXT,
                secondary_count INTEGER,
                raw_json TEXT NOT NULL,
                source TEXT,
                imported_at TEXT NOT NULL,
                UNIQUE(profile_id, record_id)
            );

            CREATE INDEX IF NOT EXISTS idx_records_profile_time ON records(profile_id, time DESC, id DESC);
            CREATE INDEX IF NOT EXISTS idx_records_profile_pool ON records(profile_id, pool_id, time DESC);
            CREATE INDEX IF NOT EXISTS idx_records_profile_type ON records(profile_id, record_type, time DESC);

            CREATE TABLE IF NOT EXISTS capture_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
                source_kind TEXT NOT NULL,
                source_path TEXT,
                started_at TEXT NOT NULL,
                completed_at TEXT,
                status TEXT NOT NULL,
                records_seen INTEGER NOT NULL DEFAULT 0,
                records_inserted INTEGER NOT NULL DEFAULT 0,
                records_skipped INTEGER NOT NULL DEFAULT 0,
                error_code TEXT,
                error_message TEXT
            );

            CREATE TABLE IF NOT EXISTS pool_rules (
                pool_id TEXT PRIMARY KEY,
                pool_name TEXT NOT NULL,
                group_label TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS item_meta (
                item_id TEXT PRIMARY KEY,
                item_name TEXT NOT NULL,
                rarity INTEGER NOT NULL,
                category TEXT,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS item_aliases (
                alias_id TEXT PRIMARY KEY,
                item_id TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            ",
        )?;
        self.conn.execute(
            "INSERT OR IGNORE INTO migrations(version, name, applied_at) VALUES(1, 'initial_gui_schema', ?1)",
            [now_stamp()],
        )?;
        self.rebuild_table_if_columns_differ(
            "pool_rules",
            &[
                "pool_id",
                "pool_name",
                "group_label",
                "created_at",
                "updated_at",
            ],
            "
            CREATE TABLE pool_rules_new (
                pool_id TEXT PRIMARY KEY,
                pool_name TEXT NOT NULL,
                group_label TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            INSERT INTO pool_rules_new(pool_id, pool_name, group_label, created_at, updated_at)
            SELECT pool_id, pool_name, group_label, created_at, updated_at FROM pool_rules;
            DROP TABLE pool_rules;
            ALTER TABLE pool_rules_new RENAME TO pool_rules;
            ",
        )?;
        self.rebuild_table_if_columns_differ(
            "item_meta",
            &["item_id", "item_name", "rarity", "category", "updated_at"],
            "
            CREATE TABLE item_meta_new (
                item_id TEXT PRIMARY KEY,
                item_name TEXT NOT NULL,
                rarity INTEGER NOT NULL,
                category TEXT,
                updated_at TEXT NOT NULL
            );
            INSERT INTO item_meta_new(item_id, item_name, rarity, category, updated_at)
            SELECT item_id, item_name, rarity, category, updated_at FROM item_meta WHERE rarity IS NOT NULL;
            DROP TABLE item_meta;
            ALTER TABLE item_meta_new RENAME TO item_meta;
            ",
        )?;
        self.rebuild_table_if_columns_differ(
            "item_aliases",
            &["alias_id", "item_id", "updated_at"],
            "
            CREATE TABLE item_aliases_new (
                alias_id TEXT PRIMARY KEY,
                item_id TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            INSERT INTO item_aliases_new(alias_id, item_id, updated_at)
            SELECT alias_id, item_id, updated_at FROM item_aliases;
            DROP TABLE item_aliases;
            ALTER TABLE item_aliases_new RENAME TO item_aliases;
            ",
        )?;
        self.ensure_default_profile()?;
        Ok(())
    }

    fn rebuild_table_if_columns_differ(
        &self,
        table_name: &str,
        expected_columns: &[&str],
        rebuild_sql: &str,
    ) -> Result<(), GuiError> {
        let mut stmt = self
            .conn
            .prepare(&format!("PRAGMA table_info({table_name})"))?;
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        if columns != expected_columns {
            self.conn.execute_batch(rebuild_sql)?;
        }
        Ok(())
    }

    pub fn create_profile(&self, name: &str) -> Result<Profile, GuiError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(GuiError::InvalidDocument(
                "profile name is empty".to_string(),
            ));
        }
        let now = now_stamp();
        self.conn.execute(
            "INSERT OR IGNORE INTO profiles(name, created_at, updated_at) VALUES(?1, ?2, ?2)",
            params![name, now],
        )?;
        self.profile_by_name(name)
    }

    pub fn ensure_default_profile(&self) -> Result<Profile, GuiError> {
        self.create_profile(DEFAULT_PROFILE_NAME)
    }

    pub fn list_profiles(&self) -> Result<Vec<Profile>, GuiError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, created_at, updated_at FROM profiles ORDER BY id ASC")?;
        let profiles = stmt
            .query_map([], |row| {
                Ok(Profile {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(profiles)
    }

    pub fn profile(&self, profile_id: i64) -> Result<Profile, GuiError> {
        self.conn
            .query_row(
                "SELECT id, name, created_at, updated_at FROM profiles WHERE id = ?1",
                [profile_id],
                |row| {
                    Ok(Profile {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        created_at: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| GuiError::InvalidDocument(format!("profile not found: {profile_id}")))
    }

    pub fn import_public_document(
        &mut self,
        profile_id: i64,
        document_text: &str,
        source_kind: &str,
        source_path: Option<&str>,
    ) -> Result<ImportReport, GuiError> {
        self.profile(profile_id)?;
        let document: Value = serde_json::from_str(document_text)?;
        let records = document_records(&document)?;
        let tx = self.conn.transaction()?;
        let now = now_stamp();
        let run_id = create_run(&tx, profile_id, source_kind, source_path, &now)?;
        let mut inserted = 0_u64;
        let mut skipped = 0_u64;

        for record in records {
            let normalized = NormalizedRecord::from_value(record)?;
            let raw_json = serde_json::to_string(record)?;
            let changed = tx.execute(
                "
                INSERT OR IGNORE INTO records(
                    profile_id, record_id, record_type, time, pool_id, pool_name,
                    item_id, item_name, count, roll_points, roll_label,
                    secondary_item_id, secondary_item_name, secondary_count,
                    raw_json, source, imported_at
                ) VALUES(
                    ?1, ?2, ?3, ?4, ?5, ?6,
                    ?7, ?8, ?9, ?10, ?11,
                    ?12, ?13, ?14,
                    ?15, ?16, ?17
                )
                ",
                params![
                    profile_id,
                    normalized.record_id,
                    normalized.record_type,
                    normalized.time,
                    normalized.pool_id,
                    normalized.pool_name,
                    normalized.item_id,
                    normalized.item_name,
                    normalized.count,
                    normalized.roll_points,
                    normalized.roll_label,
                    normalized.secondary_item_id,
                    normalized.secondary_item_name,
                    normalized.secondary_count,
                    raw_json,
                    source_path,
                    now,
                ],
            )?;
            if changed == 1 {
                inserted += 1;
            } else {
                skipped += 1;
            }
        }

        complete_run(&tx, run_id, records.len() as u64, inserted, skipped)?;
        tx.commit()?;
        Ok(ImportReport {
            profile_id,
            run_id,
            source_kind: source_kind.to_string(),
            source_path: source_path.map(ToOwned::to_owned),
            records_seen: records.len() as u64,
            records_inserted: inserted,
            records_skipped: skipped,
        })
    }

    pub fn upsert_rules(
        &mut self,
        pool_rules: &[PoolRule],
        item_meta: &[ItemMeta],
        item_aliases: &[ItemAlias],
    ) -> Result<(), GuiError> {
        let tx = self.conn.transaction()?;
        let now = now_stamp();
        for rule in pool_rules {
            tx.execute(
                "
                INSERT INTO pool_rules(pool_id, pool_name, group_label, created_at, updated_at)
                VALUES(?1, ?2, ?3, ?4, ?4)
                ON CONFLICT(pool_id) DO UPDATE SET
                    pool_name = excluded.pool_name,
                    group_label = excluded.group_label,
                    updated_at = excluded.updated_at
                ",
                params![rule.pool_id, rule.pool_name, rule.group_label, now],
            )?;
        }
        for item in item_meta {
            tx.execute(
                "
                INSERT INTO item_meta(item_id, item_name, rarity, category, updated_at)
                VALUES(?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(item_id) DO UPDATE SET
                    item_name = excluded.item_name,
                    rarity = excluded.rarity,
                    category = excluded.category,
                    updated_at = excluded.updated_at
                ",
                params![
                    item.item_id,
                    item.item_name,
                    item.rarity,
                    item.category,
                    now
                ],
            )?;
        }
        for alias in item_aliases {
            tx.execute(
                "
                INSERT INTO item_aliases(alias_id, item_id, updated_at)
                VALUES(?1, ?2, ?3)
                ON CONFLICT(alias_id) DO UPDATE SET
                    item_id = excluded.item_id,
                    updated_at = excluded.updated_at
                ",
                params![alias.alias_id, alias.item_id, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn dashboard_summary(&self, profile_id: i64) -> Result<DashboardSummary, GuiError> {
        let profile = self.profile(profile_id)?;
        let total_records = self.conn.query_row(
            "SELECT COUNT(*) FROM records WHERE profile_id = ?1",
            [profile_id],
            |row| row.get::<_, u64>(0),
        )?;
        Ok(DashboardSummary {
            profile,
            total_records,
            pools: self.pool_summaries(profile_id)?,
            by_record_type: self.type_summaries(profile_id)?,
            timeline: self.timeline(profile_id)?,
            latest_records: self.latest_records(profile_id, 12)?,
        })
    }

    pub fn list_records(
        &self,
        profile_id: i64,
        filter: &RecordFilter,
    ) -> Result<RecordList, GuiError> {
        self.profile(profile_id)?;
        let limit = filter.limit.unwrap_or(200).min(1000);
        let offset = filter.offset.unwrap_or(0);
        let pool = filter.pool_id.as_deref().filter(|value| !value.is_empty());
        let record_type = filter
            .record_type
            .as_deref()
            .filter(|value| !value.is_empty());
        let search = filter
            .search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let mut clauses = vec!["profile_id = ?".to_string()];
        let mut values = vec![SqlValue::Integer(profile_id)];
        if let Some(pool) = pool {
            clauses.push("pool_id = ?".to_string());
            values.push(SqlValue::Text(pool.to_string()));
        }
        if let Some(record_type) = record_type {
            clauses.push("record_type = ?".to_string());
            values.push(SqlValue::Text(record_type.to_string()));
        }
        if let Some(search) = search {
            clauses.push("(item_name LIKE ? OR item_id LIKE ?)".to_string());
            let needle = format!("%{search}%");
            values.push(SqlValue::Text(needle.clone()));
            values.push(SqlValue::Text(needle));
        }

        let where_sql = clauses.join(" AND ");
        let total = self.query_record_count(&where_sql, &values)?;
        let records = self.query_records(&where_sql, &values, limit, offset)?;
        Ok(RecordList { total, records })
    }

    pub fn export_json(&self, profile_id: i64) -> Result<String, GuiError> {
        json_export(&self.conn, profile_id)
    }

    pub fn export_csv(&self, profile_id: i64) -> Result<String, GuiError> {
        csv_export(&self.conn, profile_id)
    }

    fn profile_by_name(&self, name: &str) -> Result<Profile, GuiError> {
        self.conn
            .query_row(
                "SELECT id, name, created_at, updated_at FROM profiles WHERE name = ?1",
                [name],
                |row| {
                    Ok(Profile {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        created_at: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    fn pool_summaries(&self, profile_id: i64) -> Result<Vec<PoolSummary>, GuiError> {
        let mut stmt = self.conn.prepare(
            "
            SELECT
                COALESCE(r.pool_id, '') AS pool_id,
                COALESCE(MAX(r.pool_name), '') AS pool_name,
                COALESCE(MAX(pr.group_label), MAX(r.pool_name), '') AS group_label,
                COUNT(*) AS record_count,
                COALESCE(SUM(CASE WHEN im.rarity = 5 THEN 1 ELSE 0 END), 0) AS hit_count,
                MAX(r.time) AS last_time
            FROM records r
            LEFT JOIN pool_rules pr ON pr.pool_id = r.pool_id
            LEFT JOIN item_aliases ia ON ia.alias_id = r.item_id
            LEFT JOIN item_meta im ON im.item_id = COALESCE(ia.item_id, r.item_id)
            WHERE r.profile_id = ?1
            GROUP BY COALESCE(r.pool_id, '')
            ORDER BY record_count DESC, pool_name ASC
            ",
        )?;
        let mut summaries = Vec::new();
        let rows = stmt.query_map([profile_id], |row| {
            Ok(PartialPoolSummary {
                pool_id: row.get(0)?,
                pool_name: row.get(1)?,
                group_label: row.get(2)?,
                record_count: row.get(3)?,
                hit_count: row.get(4)?,
                last_time: row.get(5)?,
            })
        })?;
        for row in rows {
            let row = row?;
            let current_pity = if row.hit_count > 0 {
                Some(self.current_pity_after_latest_hit(profile_id, &row.pool_id)?)
            } else {
                None
            };
            let last_item_name = self.last_item_for_pool(profile_id, &row.pool_id)?;
            summaries.push(PoolSummary {
                pool_id: row.pool_id,
                pool_name: row.pool_name,
                group_label: row.group_label,
                record_count: row.record_count,
                hit_count: row.hit_count,
                current_pity,
                last_time: row.last_time,
                last_item_name,
            });
        }
        Ok(summaries)
    }

    fn type_summaries(&self, profile_id: i64) -> Result<Vec<TypeSummary>, GuiError> {
        let mut stmt = self.conn.prepare(
            "
            SELECT record_type, COUNT(*)
            FROM records
            WHERE profile_id = ?1
            GROUP BY record_type
            ORDER BY COUNT(*) DESC, record_type ASC
            ",
        )?;
        let rows = stmt
            .query_map([profile_id], |row| {
                Ok(TypeSummary {
                    record_type: row.get(0)?,
                    record_count: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn timeline(&self, profile_id: i64) -> Result<Vec<TimelineBucket>, GuiError> {
        let mut stmt = self.conn.prepare(
            "
            SELECT SUBSTR(time, 1, 10) AS day, COUNT(*)
            FROM records
            WHERE profile_id = ?1 AND time IS NOT NULL AND time != ''
            GROUP BY day
            ORDER BY day ASC
            ",
        )?;
        let rows = stmt
            .query_map([profile_id], |row| {
                Ok(TimelineBucket {
                    day: row.get(0)?,
                    record_count: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn latest_records(&self, profile_id: i64, limit: u32) -> Result<Vec<LatestRecord>, GuiError> {
        let mut stmt = self.conn.prepare(
            "
            SELECT record_id, record_type, time, pool_id, pool_name, item_id, item_name, count, roll_label
            FROM records
            WHERE profile_id = ?1
            ORDER BY time DESC, id DESC
            LIMIT ?2
            ",
        )?;
        let rows = stmt
            .query_map(params![profile_id, limit], |row| {
                Ok(LatestRecord {
                    record_id: row.get(0)?,
                    record_type: row.get(1)?,
                    time: row.get(2)?,
                    pool_id: row.get(3)?,
                    pool_name: row.get(4)?,
                    item_id: row.get(5)?,
                    item_name: row.get(6)?,
                    count: row.get(7)?,
                    roll_label: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn current_pity_after_latest_hit(
        &self,
        profile_id: i64,
        pool_id: &str,
    ) -> Result<u64, GuiError> {
        let latest_hit_id = self
            .conn
            .query_row(
                "
                SELECT r.id
                FROM records r
                LEFT JOIN item_aliases ia ON ia.alias_id = r.item_id
                JOIN item_meta im ON im.item_id = COALESCE(ia.item_id, r.item_id) AND im.rarity = 5
                WHERE r.profile_id = ?1 AND COALESCE(r.pool_id, '') = ?2
                ORDER BY r.time DESC, r.id DESC
                LIMIT 1
                ",
                params![profile_id, pool_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let Some(latest_hit_id) = latest_hit_id else {
            return Ok(0);
        };
        let count = self.conn.query_row(
            "
            SELECT COUNT(*)
            FROM records
            WHERE profile_id = ?1
              AND COALESCE(pool_id, '') = ?2
              AND id > ?3
            ",
            params![profile_id, pool_id, latest_hit_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    fn last_item_for_pool(
        &self,
        profile_id: i64,
        pool_id: &str,
    ) -> Result<Option<String>, GuiError> {
        self.conn
            .query_row(
                "
                SELECT item_name
                FROM records
                WHERE profile_id = ?1 AND COALESCE(pool_id, '') = ?2
                ORDER BY time DESC, id DESC
                LIMIT 1
                ",
                params![profile_id, pool_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    fn query_record_count(&self, where_sql: &str, values: &[SqlValue]) -> Result<u64, GuiError> {
        let sql = format!("SELECT COUNT(*) FROM records WHERE {where_sql}");
        let mut stmt = self.conn.prepare(&sql)?;
        let count = stmt.query_row(params_from_iter(values.iter()), |row| row.get(0))?;
        Ok(count)
    }

    fn query_records(
        &self,
        where_sql: &str,
        values: &[SqlValue],
        limit: u32,
        offset: u32,
    ) -> Result<Vec<StoredRecord>, GuiError> {
        let sql = format!(
            "
            SELECT record_id, record_type, time, pool_id, pool_name, item_id, item_name,
                   count, roll_points, roll_label, secondary_item_id, secondary_item_name, secondary_count
            FROM records
            WHERE {where_sql}
            ORDER BY time DESC, id DESC
            LIMIT ? OFFSET ?
            ",
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mapper = |row: &rusqlite::Row<'_>| {
            Ok(StoredRecord {
                record_id: row.get(0)?,
                record_type: row.get(1)?,
                time: row.get(2)?,
                pool_id: row.get(3)?,
                pool_name: row.get(4)?,
                item_id: row.get(5)?,
                item_name: row.get(6)?,
                count: row.get(7)?,
                roll_points: row.get(8)?,
                roll_label: row.get(9)?,
                secondary_item_id: row.get(10)?,
                secondary_item_name: row.get(11)?,
                secondary_count: row.get(12)?,
            })
        };
        let mut page_values = values.to_vec();
        page_values.push(SqlValue::Integer(i64::from(limit)));
        page_values.push(SqlValue::Integer(i64::from(offset)));
        let records = stmt
            .query_map(params_from_iter(page_values.iter()), mapper)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(records)
    }
}

#[derive(Debug)]
struct PartialPoolSummary {
    pool_id: String,
    pool_name: String,
    group_label: String,
    record_count: u64,
    hit_count: u64,
    last_time: Option<String>,
}

#[derive(Debug)]
struct NormalizedRecord {
    record_id: String,
    record_type: String,
    time: Option<String>,
    pool_id: Option<String>,
    pool_name: Option<String>,
    item_id: String,
    item_name: Option<String>,
    count: Option<i64>,
    roll_points: Option<i64>,
    roll_label: Option<String>,
    secondary_item_id: Option<String>,
    secondary_item_name: Option<String>,
    secondary_count: Option<i64>,
}

impl NormalizedRecord {
    fn from_value(value: &Value) -> Result<Self, GuiError> {
        let object = value
            .as_object()
            .ok_or_else(|| GuiError::InvalidDocument("record must be an object".to_string()))?;
        let record_id = required_text(object, "record_id")?;
        let item_id = required_text(object, "item_id")?;
        Ok(Self {
            record_id,
            record_type: optional_text(object, "record_type")
                .unwrap_or_else(|| "unknown".to_string()),
            time: optional_text(object, "time"),
            pool_id: optional_text(object, "pool_id"),
            pool_name: optional_text(object, "pool_name"),
            item_id,
            item_name: optional_text(object, "item_name"),
            count: optional_i64(object, "count"),
            roll_points: optional_i64(object, "roll_points"),
            roll_label: optional_text(object, "roll_label"),
            secondary_item_id: optional_text(object, "secondary_item_id"),
            secondary_item_name: optional_text(object, "secondary_item_name"),
            secondary_count: optional_i64(object, "secondary_count"),
        })
    }
}

fn document_records(document: &Value) -> Result<&Vec<Value>, GuiError> {
    document
        .get("nte")
        .and_then(|nte| nte.get("list"))
        .and_then(Value::as_array)
        .ok_or_else(|| GuiError::InvalidDocument("expected nte.list array".to_string()))
}

fn required_text(object: &Map<String, Value>, key: &str) -> Result<String, GuiError> {
    optional_text(object, key)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| GuiError::InvalidDocument(format!("record missing string field: {key}")))
}

fn optional_text(object: &Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn optional_i64(object: &Map<String, Value>, key: &str) -> Option<i64> {
    object.get(key).and_then(Value::as_i64)
}

fn create_run(
    tx: &Transaction<'_>,
    profile_id: i64,
    source_kind: &str,
    source_path: Option<&str>,
    now: &str,
) -> Result<i64, GuiError> {
    tx.execute(
        "
        INSERT INTO capture_runs(profile_id, source_kind, source_path, started_at, status)
        VALUES(?1, ?2, ?3, ?4, 'running')
        ",
        params![profile_id, source_kind, source_path, now],
    )?;
    Ok(tx.last_insert_rowid())
}

fn complete_run(
    tx: &Transaction<'_>,
    run_id: i64,
    records_seen: u64,
    records_inserted: u64,
    records_skipped: u64,
) -> Result<(), GuiError> {
    tx.execute(
        "
        UPDATE capture_runs
        SET completed_at = ?1,
            status = 'completed',
            records_seen = ?2,
            records_inserted = ?3,
            records_skipped = ?4
        WHERE id = ?5
        ",
        params![
            now_stamp(),
            records_seen,
            records_inserted,
            records_skipped,
            run_id
        ],
    )?;
    Ok(())
}

fn now_stamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

pub(crate) fn export_document(records: Vec<Value>) -> Value {
    json!({
        "info": {
            "schema": "nte-gacha-export",
            "schema_version": "1.0",
            "export_app": "nte-gacha-exporter-gui",
            "export_app_version": env!("CARGO_PKG_VERSION"),
            "export_timestamp": now_stamp().parse::<u64>().unwrap_or(0),
            "privacy": "sanitized"
        },
        "nte": {
            "list": records
        }
    })
}

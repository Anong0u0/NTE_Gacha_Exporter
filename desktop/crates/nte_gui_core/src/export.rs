use rusqlite::{params, Connection};
use serde_json::Value;

use crate::db::export_document;
use crate::model::GuiError;

pub fn json_export(conn: &Connection, profile_id: i64) -> Result<String, GuiError> {
    let records = export_records(conn, profile_id)?;
    Ok(serde_json::to_string_pretty(&export_document(records))?)
}

pub fn csv_export(conn: &Connection, profile_id: i64) -> Result<String, GuiError> {
    let mut writer = csv::Writer::from_writer(vec![]);
    writer.write_record([
        "time",
        "pool_group",
        "pool_name",
        "item_name",
        "count",
        "roll_label",
        "secondary_item_name",
        "secondary_count",
    ])?;

    let mut stmt = conn.prepare(
        "
        SELECT
            COALESCE(r.time, ''),
            COALESCE(pr.group_label, r.pool_name, ''),
            COALESCE(r.pool_name, ''),
            COALESCE(r.item_name, ''),
            r.count,
            COALESCE(r.roll_label, ''),
            COALESCE(r.secondary_item_name, ''),
            r.secondary_count
        FROM records r
        LEFT JOIN pool_rules pr ON pr.pool_id = r.pool_id
        WHERE r.profile_id = ?1
        ORDER BY r.time ASC, r.id ASC
        ",
    )?;
    let rows = stmt.query_map([profile_id], |row| {
        let count: Option<i64> = row.get(4)?;
        let secondary_count: Option<i64> = row.get(7)?;
        Ok([
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            count.map(|value| value.to_string()).unwrap_or_default(),
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            secondary_count
                .map(|value| value.to_string())
                .unwrap_or_default(),
        ])
    })?;
    for row in rows {
        writer.write_record(row?)?;
    }
    let bytes = writer
        .into_inner()
        .map_err(|err| GuiError::Io(err.into_error()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn export_records(conn: &Connection, profile_id: i64) -> Result<Vec<Value>, GuiError> {
    let mut stmt = conn.prepare(
        "
        SELECT raw_json
        FROM records
        WHERE profile_id = ?1
        ORDER BY time ASC, id ASC
        ",
    )?;
    let rows = stmt.query_map(params![profile_id], |row| row.get::<_, String>(0))?;
    let mut records = Vec::new();
    for row in rows {
        records.push(serde_json::from_str(&row?)?);
    }
    Ok(records)
}

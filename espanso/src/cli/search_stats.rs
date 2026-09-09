/*
 * This file is part of espanso.
 *
 * Copyright (C) 2019-2021 Federico Terzi
 *
 * espanso is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * espanso is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with espanso.  If not, see <https://www.gnu.org/licenses/>.
 */

use anyhow::Result;
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Default)]
pub struct SearchStatsSnapshot {
    pub monthly_counts: HashMap<String, i64>,
    pub favorites: HashSet<String>,
}

fn open_db(base_dir: &Path) -> Result<Connection> {
    std::fs::create_dir_all(base_dir)?;
    let conn = Connection::open(base_dir.join("stats.db"))?;
    let _ = conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS favorites (
            trigger TEXT PRIMARY KEY,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;
    Ok(conn)
}

pub fn snapshot(base_dir: &Path) -> SearchStatsSnapshot {
    match snapshot_impl(base_dir) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            log::warn!("stats: unable to load search statistics: {err}");
            SearchStatsSnapshot::default()
        }
    }
}

fn snapshot_impl(base_dir: &Path) -> Result<SearchStatsSnapshot> {
    let conn = open_db(base_dir)?;
    let mut result = SearchStatsSnapshot::default();

    // The search badge means the current calendar month, not a rolling 30-day
    // window. If the statistics schema is not initialized yet, simply show 0.
    let count_sql = "SELECT t.name, COUNT(*)
                     FROM expansions e
                     JOIN triggers t ON e.trigger_id = t.id
                     WHERE e.timestamp >= datetime('now', 'start of month')
                     GROUP BY t.name";
    if let Ok(mut stmt) = conn.prepare(count_sql) {
        if let Ok(mut rows) = stmt.query([]) {
            while let Ok(Some(row)) = rows.next() {
                if let (Ok(trigger), Ok(count)) = (row.get::<_, String>(0), row.get::<_, i64>(1)) {
                    result.monthly_counts.insert(trigger, count);
                }
            }
        }
    }

    let mut stmt = conn.prepare("SELECT trigger FROM favorites ORDER BY created_at DESC")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        result.favorites.insert(row.get::<_, String>(0)?);
    }

    Ok(result)
}

pub fn set_favorite(base_dir: &Path, trigger: &str, favorite: bool) -> Result<()> {
    let conn = open_db(base_dir)?;
    if favorite {
        conn.execute(
            "INSERT OR IGNORE INTO favorites(trigger) VALUES (?1)",
            params![trigger],
        )?;
    } else {
        conn.execute("DELETE FROM favorites WHERE trigger = ?1", params![trigger])?;
    }
    Ok(())
}

pub fn record_search_selection(trigger: &str) {
    // Search-generated matches intentionally carry no trigger through the
    // normal matcher pipeline, so they are not counted by StatsMiddleware.
    // Record the explicit search selection here. Only the trigger identifier
    // is persisted; no replacement text, form values, clipboard contents or
    // typed patient data are stored.
    let _ = crate::cli::stats::record_stats(espanso_engine::process::StatsRecord {
        trigger: trigger.to_owned(),
    });
}

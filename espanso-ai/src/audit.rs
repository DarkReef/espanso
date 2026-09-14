//! Local append-only audit trail for MCP workspace mutations.
//! The log never stores file contents, prompts, API keys or agent tokens.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const AUDIT_FILE: &str = "mcp-audit.jsonl";
const ROTATED_AUDIT_FILE: &str = "mcp-audit.1.jsonl";
const MAX_AUDIT_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEntry {
    pub timestamp_ms: u64,
    pub timestamp_utc: String,
    pub agent_id: String,
    pub agent_name: String,
    pub action: String,
    pub path: String,
    pub result: String,
}

fn audit_path(root: &Path) -> PathBuf {
    root.join(AUDIT_FILE)
}

fn rotated_path(root: &Path) -> PathBuf {
    root.join(ROTATED_AUDIT_FILE)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    // Howard Hinnant's civil-from-days algorithm, with 1970-01-01 as day zero.
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month as u32, day as u32)
}

fn utc_timestamp(timestamp_ms: u64) -> String {
    let total_seconds = (timestamp_ms / 1000) as i64;
    let millis = timestamp_ms % 1000;
    let days = total_seconds.div_euclid(86_400);
    let seconds_of_day = total_seconds.rem_euclid(86_400);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z"
    )
}

fn rotate_if_needed(root: &Path) -> Result<(), String> {
    let path = audit_path(root);
    let Ok(metadata) = fs::metadata(&path) else {
        return Ok(());
    };
    if metadata.len() < MAX_AUDIT_BYTES {
        return Ok(());
    }
    let rotated = rotated_path(root);
    if rotated.exists() {
        fs::remove_file(&rotated)
            .map_err(|error| format!("Не удалось обновить архив журнала MCP: {error}"))?;
    }
    fs::rename(&path, &rotated)
        .map_err(|error| format!("Не удалось ротировать журнал MCP: {error}"))?;
    Ok(())
}

fn open_append(path: &Path) -> Result<std::fs::File, String> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|error| format!("Не удалось открыть журнал MCP: {error}"))
}

pub fn record(
    root: &Path,
    agent_id: &str,
    agent_name: &str,
    action: &str,
    path: &str,
    success: bool,
) -> Result<(), String> {
    fs::create_dir_all(root)
        .map_err(|error| format!("Не удалось создать каталог журнала MCP: {error}"))?;
    rotate_if_needed(root)?;
    let timestamp_ms = now_ms();
    let entry = AuditEntry {
        timestamp_ms,
        timestamp_utc: utc_timestamp(timestamp_ms),
        agent_id: agent_id.to_owned(),
        agent_name: agent_name.to_owned(),
        action: action.to_owned(),
        path: path.to_owned(),
        result: if success { "success" } else { "failure" }.to_owned(),
    };
    let mut file = open_append(&audit_path(root))?;
    serde_json::to_writer(&mut file, &entry)
        .map_err(|error| format!("Не удалось сериализовать журнал MCP: {error}"))?;
    file.write_all(b"\n")
        .and_then(|_| file.flush())
        .map_err(|error| format!("Не удалось записать журнал MCP: {error}"))
}

fn read_file(path: &Path, output: &mut Vec<AuditEntry>) -> Result<(), String> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("Не удалось прочитать журнал MCP: {error}")),
    };
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else {
            continue;
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<AuditEntry>(&line) {
            output.push(entry);
        }
    }
    Ok(())
}

pub fn recent(root: &Path, limit: usize) -> Result<Vec<AuditEntry>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    read_file(&rotated_path(root), &mut entries)?;
    read_file(&audit_path(root), &mut entries)?;
    entries.sort_by_key(|entry| entry.timestamp_ms);
    if entries.len() > limit {
        entries.drain(0..entries.len() - limit);
    }
    entries.reverse();
    Ok(entries)
}

pub fn log_path(root: &Path) -> PathBuf {
    audit_path(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_is_rfc3339_utc() {
        assert_eq!(utc_timestamp(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(utc_timestamp(1_700_000_000_123), "2023-11-14T22:13:20.123Z");
    }

    #[test]
    fn records_metadata_without_payload_fields() {
        let dir = tempdir::TempDir::new("mcp-audit").unwrap();
        record(
            dir.path(),
            "agent-id",
            "test agent",
            "write",
            "match/base.yml",
            true,
        )
        .unwrap();
        let raw = fs::read_to_string(audit_path(dir.path())).unwrap();
        assert!(raw.contains("agent-id"));
        assert!(raw.contains("match/base.yml"));
        assert!(!raw.contains("content"));
        assert!(!raw.contains("token"));
        let recent = recent(dir.path(), 10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].result, "success");
    }
}

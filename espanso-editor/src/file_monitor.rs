use std::{
    collections::{hash_map::DefaultHasher, BTreeMap, BTreeSet},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use walkdir::WalkDir;

const STABILITY_DELAY: Duration = Duration::from_millis(750);
const FULL_AUDIT_INTERVAL: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fingerprint {
    Readable { length: u64, content_hash: u64 },
    Unreadable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FileSnapshot {
    files: BTreeMap<PathBuf, Fingerprint>,
}

impl FileSnapshot {
    fn changed_count(&self, other: &Self) -> usize {
        let mut paths = BTreeSet::new();
        paths.extend(self.files.keys().cloned());
        paths.extend(other.files.keys().cloned());
        paths
            .into_iter()
            .filter(|path| self.files.get(path) != other.files.get(path))
            .count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollResult {
    Unchanged,
    Pending { changed_files: usize },
    StableChanged { changed_files: usize },
    Audit,
}

#[derive(Debug)]
pub struct FileMonitor {
    accepted: FileSnapshot,
    candidate: Option<(FileSnapshot, Instant)>,
    next_audit: Instant,
}

impl FileMonitor {
    pub fn new(config_root: &Path, now: Instant) -> Self {
        Self {
            accepted: capture(config_root),
            candidate: None,
            next_audit: now + FULL_AUDIT_INTERVAL,
        }
    }

    pub fn poll(&mut self, config_root: &Path, now: Instant) -> PollResult {
        let current = capture(config_root);
        if current == self.accepted {
            self.candidate = None;
            if now >= self.next_audit {
                self.next_audit = now + FULL_AUDIT_INTERVAL;
                return PollResult::Audit;
            }
            return PollResult::Unchanged;
        }

        let changed_files = self.accepted.changed_count(&current);
        match &mut self.candidate {
            Some((candidate, since)) if candidate == &current => {
                if now.saturating_duration_since(*since) >= STABILITY_DELAY {
                    self.accepted = current;
                    self.candidate = None;
                    self.next_audit = now + FULL_AUDIT_INTERVAL;
                    PollResult::StableChanged { changed_files }
                } else {
                    PollResult::Pending { changed_files }
                }
            }
            Some((candidate, since)) => {
                *candidate = current;
                *since = now;
                PollResult::Pending { changed_files }
            }
            None => {
                self.candidate = Some((current, now));
                PollResult::Pending { changed_files }
            }
        }
    }

    pub fn refresh(&mut self, config_root: &Path, now: Instant) {
        self.accepted = capture(config_root);
        self.candidate = None;
        self.next_audit = now + FULL_AUDIT_INTERVAL;
    }
}

fn capture(config_root: &Path) -> FileSnapshot {
    let mut files = BTreeMap::new();
    for directory_name in ["config", "match", "scripts"] {
        let directory = config_root.join(directory_name);
        if !directory.is_dir() {
            continue;
        }
        for entry in WalkDir::new(&directory).follow_links(false) {
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();
            if !entry.file_type().is_file() || !is_supported(path, directory_name) {
                continue;
            }

            let fingerprint = match fs::read(path) {
                Ok(content) => {
                    let mut hasher = DefaultHasher::new();
                    content.hash(&mut hasher);
                    Fingerprint::Readable {
                        length: content.len() as u64,
                        content_hash: hasher.finish(),
                    }
                }
                Err(_) => Fingerprint::Unreadable,
            };
            files.insert(path.to_path_buf(), fingerprint);
        }
    }
    FileSnapshot { files }
}

fn is_supported(path: &Path, root: &str) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    match root {
        "config" | "match" => {
            extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml")
        }
        "scripts" => extension.eq_ignore_ascii_case("rhai"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_until_hash_snapshot_is_stable() {
        let directory = tempdir::TempDir::new("respanso-file-monitor").unwrap();
        let match_dir = directory.path().join("match");
        fs::create_dir_all(&match_dir).unwrap();
        let start = Instant::now();
        let mut monitor = FileMonitor::new(directory.path(), start);

        let file = match_dir.join("new.yml");
        fs::write(&file, "matches: []\n").unwrap();
        assert_eq!(
            monitor.poll(directory.path(), start + Duration::from_secs(3)),
            PollResult::Pending { changed_files: 1 }
        );
        assert_eq!(
            monitor.poll(
                directory.path(),
                start + Duration::from_secs(3) + STABILITY_DELAY
            ),
            PollResult::StableChanged { changed_files: 1 }
        );
        assert_eq!(
            monitor.poll(directory.path(), start + Duration::from_secs(5)),
            PollResult::Unchanged
        );
    }

    #[test]
    fn restarts_stability_window_when_content_changes_again() {
        let directory = tempdir::TempDir::new("respanso-file-monitor-rewrite").unwrap();
        let scripts = directory.path().join("scripts");
        fs::create_dir_all(&scripts).unwrap();
        let start = Instant::now();
        let mut monitor = FileMonitor::new(directory.path(), start);
        let file = scripts.join("test.rhai");

        fs::write(&file, "40 + 2").unwrap();
        assert!(matches!(
            monitor.poll(directory.path(), start + Duration::from_secs(1)),
            PollResult::Pending { .. }
        ));
        fs::write(&file, "84 / 2").unwrap();
        assert!(matches!(
            monitor.poll(directory.path(), start + Duration::from_secs(2)),
            PollResult::Pending { .. }
        ));
        assert!(matches!(
            monitor.poll(
                directory.path(),
                start + Duration::from_secs(2) + STABILITY_DELAY
            ),
            PollResult::StableChanged { .. }
        ));
    }

    #[test]
    fn audits_even_when_hashes_are_unchanged() {
        let directory = tempdir::TempDir::new("respanso-file-monitor-audit").unwrap();
        let start = Instant::now();
        let mut monitor = FileMonitor::new(directory.path(), start);
        assert_eq!(
            monitor.poll(directory.path(), start + FULL_AUDIT_INTERVAL),
            PollResult::Audit
        );
    }

    #[test]
    fn watches_config_match_and_rhai_only() {
        let directory = tempdir::TempDir::new("respanso-file-monitor-types").unwrap();
        fs::create_dir_all(directory.path().join("config")).unwrap();
        fs::create_dir_all(directory.path().join("match")).unwrap();
        fs::create_dir_all(directory.path().join("scripts")).unwrap();
        fs::write(
            directory.path().join("config/default.yml"),
            "enable: true\n",
        )
        .unwrap();
        fs::write(directory.path().join("match/base.yaml"), "matches: []\n").unwrap();
        fs::write(directory.path().join("scripts/test.rhai"), "42").unwrap();
        fs::write(directory.path().join("scripts/ignored.txt"), "ignored").unwrap();

        let snapshot = capture(directory.path());
        assert_eq!(snapshot.files.len(), 3);
    }
}

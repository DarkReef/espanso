use std::{
    collections::{hash_map::DefaultHasher, BTreeMap},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fingerprint {
    length: u64,
    content_hash: u64,
}

#[derive(Debug, Default)]
pub struct FileMonitor {
    snapshot: BTreeMap<PathBuf, Fingerprint>,
}

impl FileMonitor {
    pub fn new(config_root: &Path) -> Self {
        Self {
            snapshot: capture(config_root),
        }
    }

    pub fn changed(&mut self, config_root: &Path) -> bool {
        let current = capture(config_root);
        if current == self.snapshot {
            false
        } else {
            self.snapshot = current;
            true
        }
    }

    pub fn refresh(&mut self, config_root: &Path) {
        self.snapshot = capture(config_root);
    }
}

fn capture(config_root: &Path) -> BTreeMap<PathBuf, Fingerprint> {
    let mut files = BTreeMap::new();
    for directory in [config_root.join("config"), config_root.join("match")] {
        if !directory.is_dir() {
            continue;
        }
        for entry in WalkDir::new(directory).follow_links(false) {
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();
            if !entry.file_type().is_file() || !is_yaml(path) {
                continue;
            }
            let Ok(content) = fs::read(path) else {
                continue;
            };
            let mut hasher = DefaultHasher::new();
            content.hash(&mut hasher);
            files.insert(
                path.to_path_buf(),
                Fingerprint {
                    length: content.len() as u64,
                    content_hash: hasher.finish(),
                },
            );
        }
    }
    files
}

fn is_yaml(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            value.eq_ignore_ascii_case("yml") || value.eq_ignore_ascii_case("yaml")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_created_changed_and_removed_yaml_files() {
        let directory = tempdir::TempDir::new("respanso-file-monitor").unwrap();
        let match_dir = directory.path().join("match");
        fs::create_dir_all(&match_dir).unwrap();
        let mut monitor = FileMonitor::new(directory.path());
        assert!(!monitor.changed(directory.path()));

        let file = match_dir.join("new.yml");
        fs::write(&file, "matches: []\n").unwrap();
        assert!(monitor.changed(directory.path()));
        assert!(!monitor.changed(directory.path()));

        fs::write(&file, "matches:\n  - trigger: :a\n    replace: A\n").unwrap();
        assert!(monitor.changed(directory.path()));

        fs::remove_file(file).unwrap();
        assert!(monitor.changed(directory.path()));
    }

    #[test]
    fn watches_settings_yaml_and_ignores_unrelated_files() {
        let directory = tempdir::TempDir::new("respanso-settings-monitor").unwrap();
        let config_dir = directory.path().join("config");
        fs::create_dir_all(&config_dir).unwrap();
        let mut monitor = FileMonitor::new(directory.path());

        fs::write(config_dir.join("notes.txt"), "not configuration").unwrap();
        assert!(!monitor.changed(directory.path()));

        fs::write(config_dir.join("default.yaml"), "enable: true\n").unwrap();
        assert!(monitor.changed(directory.path()));
    }
}

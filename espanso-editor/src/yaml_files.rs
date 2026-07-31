use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

const DISABLED_YAML_SUFFIX: &str = ".disabled";
const DISABLED_IMPORTED_YAML_SUFFIX: &str = ".disabled.imported";
const DELETE_QUARANTINE_SUFFIX: &str = ".respanso-delete";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlFileEntry {
    pub path: PathBuf,
    pub enabled: bool,
}

#[must_use]
pub fn entries(active_files: &[PathBuf], match_root: &Path) -> Vec<YamlFileEntry> {
    let mut entries = active_files
        .iter()
        .cloned()
        .map(|path| YamlFileEntry {
            path,
            enabled: true,
        })
        .collect::<Vec<_>>();

    if match_root.is_dir() {
        for path in WalkDir::new(match_root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .map(walkdir::DirEntry::into_path)
            .filter(|path| is_disabled(path))
        {
            let logical = enabled_path(&path).unwrap_or_else(|| path.clone());
            if entries.iter().any(|entry| entry.path == logical) {
                continue;
            }
            entries.push(YamlFileEntry {
                path,
                enabled: false,
            });
        }
    }

    entries.sort_by_key(|entry| {
        visible_path(&entry.path, entry.enabled)
            .to_string_lossy()
            .to_lowercase()
    });
    entries
}

#[must_use]
pub fn disabled_path(path: &Path, imported: bool) -> PathBuf {
    let Some(file_name) = path.file_name() else {
        return path.with_extension("disabled");
    };
    let mut disabled_name = file_name.to_os_string();
    disabled_name.push(if imported {
        DISABLED_IMPORTED_YAML_SUFFIX
    } else {
        DISABLED_YAML_SUFFIX
    });
    path.with_file_name(disabled_name)
}

#[must_use]
pub fn enabled_path(path: &Path) -> Option<PathBuf> {
    let file_name = path.file_name()?.to_string_lossy();
    let lower = file_name.to_ascii_lowercase();
    let suffix = if lower.ends_with(DISABLED_IMPORTED_YAML_SUFFIX) {
        DISABLED_IMPORTED_YAML_SUFFIX
    } else if lower.ends_with(DISABLED_YAML_SUFFIX) {
        DISABLED_YAML_SUFFIX
    } else {
        return None;
    };
    let enabled_name = &file_name[..file_name.len() - suffix.len()];
    let extension = Path::new(enabled_name)
        .extension()
        .and_then(|extension| extension.to_str());
    if !extension.is_some_and(|extension| {
        extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml")
    }) {
        return None;
    }
    Some(path.with_file_name(enabled_name))
}

#[must_use]
pub fn is_disabled(path: &Path) -> bool {
    enabled_path(path).is_some()
}

#[must_use]
pub fn is_imported_disabled(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.to_ascii_lowercase()
                .ends_with(DISABLED_IMPORTED_YAML_SUFFIX)
        })
}

#[must_use]
pub fn visible_path(path: &Path, enabled: bool) -> PathBuf {
    if enabled {
        path.to_path_buf()
    } else {
        enabled_path(path).unwrap_or_else(|| path.to_path_buf())
    }
}

#[must_use]
pub fn display_name(root: &Path, path: &Path, enabled: bool) -> String {
    let visible = visible_path(path, enabled);
    visible
        .strip_prefix(root)
        .unwrap_or(&visible)
        .display()
        .to_string()
}

pub fn normalize_file_name(value: &str) -> Result<String, String> {
    let mut name = value.trim().to_owned();
    if name.is_empty() {
        return Err("Укажите имя YAML-файла".to_owned());
    }
    if name.starts_with('.')
        || name.chars().any(|character| {
            character.is_control()
                || ['<', '>', ':', '"', '/', '\\', '|', '?', '*'].contains(&character)
        })
    {
        return Err("Имя содержит недопустимые для Windows символы".to_owned());
    }
    if Path::new(&name).extension().is_none() {
        name.push_str(".yml");
    }
    let path = Path::new(&name);
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case("yml") && !extension.eq_ignore_ascii_case("yaml") {
        return Err("Допустимы только расширения .yml и .yaml".to_owned());
    }
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    if stem.is_empty() {
        return Err("Имя YAML-файла не может быть пустым".to_owned());
    }
    if stem.ends_with(' ') || stem.ends_with('.') {
        return Err("Имя файла не может оканчиваться пробелом или точкой".to_owned());
    }
    let reserved = stem
        .split('.')
        .next()
        .unwrap_or(stem)
        .to_ascii_uppercase();
    if matches!(
        reserved.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        return Err("Это имя зарезервировано Windows".to_owned());
    }
    Ok(name)
}

pub fn deletion_quarantine_path(path: &Path) -> Result<PathBuf, String> {
    let file_name = path
        .file_name()
        .ok_or_else(|| "Не удалось определить имя удаляемого YAML-файла".to_owned())?;
    let mut quarantine_name = OsString::from(".");
    quarantine_name.push(file_name);
    quarantine_name.push(DELETE_QUARANTINE_SUFFIX);
    Ok(path.with_file_name(quarantine_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_names_round_trip_for_imported_and_standalone_files() {
        let source = Path::new("match/therapy.yml");
        for imported in [false, true] {
            let disabled = disabled_path(source, imported);
            assert!(is_disabled(&disabled));
            assert_eq!(enabled_path(&disabled).as_deref(), Some(source));
        }
    }

    #[test]
    fn unrelated_disabled_file_is_not_treated_as_yaml() {
        assert!(!is_disabled(Path::new("match/readme.txt.disabled")));
    }

    #[test]
    fn normalizes_safe_names_and_rejects_windows_edge_cases() {
        assert_eq!(normalize_file_name("therapy").unwrap(), "therapy.yml");
        assert!(normalize_file_name("../therapy.yml").is_err());
        assert!(normalize_file_name("CON.backup.yml").is_err());
        assert!(normalize_file_name("therapy .yml").is_err());
    }

    #[test]
    fn quarantine_path_cannot_be_loaded_as_yaml() {
        let quarantine = deletion_quarantine_path(Path::new("match/therapy.yml")).unwrap();
        assert_eq!(
            quarantine.file_name().and_then(|name| name.to_str()),
            Some(".therapy.yml.respanso-delete")
        );
        assert!(!is_disabled(&quarantine));
        assert_ne!(
            quarantine.extension().and_then(|value| value.to_str()),
            Some("yml")
        );
    }
}

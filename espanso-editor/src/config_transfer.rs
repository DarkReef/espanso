use rhai::Engine;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use walkdir::WalkDir;

const PACKAGE_FORMAT: &str = "rEspanso configuration package";
const PACKAGE_VERSION: u32 = 1;
const MAX_PACKAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_FILE_BYTES: usize = 4 * 1024 * 1024;
const MAX_FILES: usize = 10_000;
pub const PACKAGE_EXTENSION: &str = "respanso-config";

#[derive(Debug, Clone)]
pub struct PackageSummary {
    pub path: PathBuf,
    pub exported_at_unix: u64,
    pub file_count: usize,
    pub total_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct ImportReport {
    pub package: PathBuf,
    pub backup: PathBuf,
    pub file_count: usize,
    pub cleanup_warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageDocument {
    format: String,
    version: u32,
    exported_at_unix: u64,
    files: Vec<PackageFile>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageFile {
    path: String,
    content: String,
}

pub fn exchange_dir(config_root: &Path) -> PathBuf {
    config_root.join("config-packages")
}

pub fn is_package_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(PACKAGE_EXTENSION))
}

pub fn list_packages(config_root: &Path) -> Result<Vec<PathBuf>, String> {
    let directory = exchange_dir(config_root);
    fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "Не удалось создать папку пакетов {}: {error}",
            directory.display()
        )
    })?;

    let mut packages = fs::read_dir(&directory)
        .map_err(|error| format!("Не удалось прочитать {}: {error}", directory.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_package_path(path))
        .collect::<Vec<_>>();

    packages.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(UNIX_EPOCH)
    });
    packages.reverse();
    Ok(packages)
}

pub fn export_current(config_root: &Path) -> Result<PackageSummary, String> {
    export_to(config_root, &exchange_dir(config_root), "rEspanso-config")
}

pub fn inspect_package(path: &Path) -> Result<PackageSummary, String> {
    let (document, total_bytes) = read_and_validate_package(path)?;
    Ok(PackageSummary {
        path: path.to_path_buf(),
        exported_at_unix: document.exported_at_unix,
        file_count: document.files.len(),
        total_bytes,
    })
}

pub fn import_package(config_root: &Path, package_path: &Path) -> Result<ImportReport, String> {
    let (document, _) = read_and_validate_package(package_path)?;
    let stamp = unix_timestamp()?;
    let backup = export_to(
        config_root,
        &exchange_dir(config_root).join("backups"),
        &format!("pre-import-{stamp}"),
    )?;

    let stage_root = config_root.join(format!(".respanso-import-{stamp}"));
    if stage_root.exists() {
        fs::remove_dir_all(&stage_root).map_err(|error| {
            format!(
                "Не удалось очистить временную папку {}: {error}",
                stage_root.display()
            )
        })?;
    }
    for directory in ["config", "match", "scripts"] {
        fs::create_dir_all(stage_root.join(directory)).map_err(|error| {
            format!(
                "Не удалось создать временную папку импорта {}: {error}",
                stage_root.join(directory).display()
            )
        })?;
    }

    let stage_result = (|| -> Result<(), String> {
        for file in &document.files {
            let relative = validated_relative_path(&file.path)?;
            let target = stage_root.join(&relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("Не удалось создать {}: {error}", parent.display()))?;
            }
            fs::write(&target, &file.content)
                .map_err(|error| format!("Не удалось записать {}: {error}", target.display()))?;
        }
        Ok(())
    })();

    if let Err(error) = stage_result {
        let _ = fs::remove_dir_all(&stage_root);
        return Err(error);
    }

    let mut moved_old = Vec::<(PathBuf, PathBuf)>::new();
    let mut installed = Vec::<PathBuf>::new();
    let install_result = (|| -> Result<(), String> {
        for directory in ["config", "match", "scripts"] {
            let current = config_root.join(directory);
            let staged = stage_root.join(directory);
            let old = config_root.join(format!(".respanso-old-{stamp}-{directory}"));

            if old.exists() {
                fs::remove_dir_all(&old)
                    .map_err(|error| format!("Не удалось очистить {}: {error}", old.display()))?;
            }
            if current.exists() {
                fs::rename(&current, &old).map_err(|error| {
                    format!(
                        "Не удалось подготовить замену {}: {error}",
                        current.display()
                    )
                })?;
                moved_old.push((current.clone(), old));
            }
            fs::rename(&staged, &current).map_err(|error| {
                format!(
                    "Не удалось установить импортированную папку {}: {error}",
                    current.display()
                )
            })?;
            installed.push(current);
        }
        Ok(())
    })();

    if let Err(error) = install_result {
        for current in installed.iter().rev() {
            let _ = fs::remove_dir_all(current);
        }
        for (current, old) in moved_old.iter().rev() {
            let _ = fs::rename(old, current);
        }
        let _ = fs::remove_dir_all(&stage_root);
        return Err(format!(
            "Импорт отменён, исходная конфигурация восстановлена: {error}"
        ));
    }

    let mut cleanup_warnings = Vec::new();
    for (_, old) in moved_old {
        if let Err(error) = fs::remove_dir_all(&old) {
            cleanup_warnings.push(format!("Не удалось удалить {}: {error}", old.display()));
        }
    }
    if let Err(error) = fs::remove_dir_all(&stage_root) {
        if stage_root.exists() {
            cleanup_warnings.push(format!(
                "Не удалось удалить временную папку {}: {error}",
                stage_root.display()
            ));
        }
    }

    Ok(ImportReport {
        package: package_path.to_path_buf(),
        backup: backup.path,
        file_count: document.files.len(),
        cleanup_warnings,
    })
}

pub fn open_exchange_dir(config_root: &Path) -> Result<(), String> {
    let directory = exchange_dir(config_root);
    fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "Не удалось создать папку пакетов {}: {error}",
            directory.display()
        )
    })?;

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer");
        command.arg(&directory);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(&directory);
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(&directory);
        command
    };

    command
        .spawn()
        .map_err(|error| format!("Не удалось открыть {}: {error}", directory.display()))?;
    Ok(())
}

fn export_to(
    config_root: &Path,
    destination: &Path,
    prefix: &str,
) -> Result<PackageSummary, String> {
    fs::create_dir_all(destination).map_err(|error| {
        format!(
            "Не удалось создать папку экспорта {}: {error}",
            destination.display()
        )
    })?;

    let files = collect_files(config_root)?;
    let exported_at_unix = unix_timestamp()?;
    let document = PackageDocument {
        format: PACKAGE_FORMAT.to_owned(),
        version: PACKAGE_VERSION,
        exported_at_unix,
        files,
    };
    let total_bytes = document
        .files
        .iter()
        .map(|file| file.content.len())
        .sum::<usize>();
    let path = unique_package_path(destination, prefix, exported_at_unix);
    let temporary = path.with_extension(format!("{PACKAGE_EXTENSION}.tmp"));
    let serialized = serde_json::to_vec_pretty(&document)
        .map_err(|error| format!("Не удалось сформировать пакет конфигурации: {error}"))?;
    if serialized.len() > MAX_PACKAGE_BYTES {
        return Err(format!(
            "Пакет конфигурации слишком велик: {} МБ (лимит {} МБ)",
            serialized.len() / 1024 / 1024,
            MAX_PACKAGE_BYTES / 1024 / 1024
        ));
    }
    fs::write(&temporary, serialized)
        .map_err(|error| format!("Не удалось записать {}: {error}", temporary.display()))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("Не удалось завершить экспорт {}: {error}", path.display()))?;

    Ok(PackageSummary {
        path,
        exported_at_unix,
        file_count: document.files.len(),
        total_bytes,
    })
}

fn unique_package_path(destination: &Path, prefix: &str, stamp: u64) -> PathBuf {
    let first = destination.join(format!("{prefix}-{stamp}.{PACKAGE_EXTENSION}"));
    if !first.exists() {
        return first;
    }
    for suffix in 1_u32..10_000 {
        let candidate = destination.join(format!("{prefix}-{stamp}-{suffix}.{PACKAGE_EXTENSION}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    destination.join(format!("{prefix}-{stamp}-overflow.{PACKAGE_EXTENSION}"))
}

fn collect_files(config_root: &Path) -> Result<Vec<PackageFile>, String> {
    let mut files = Vec::new();
    for directory in ["config", "match", "scripts"] {
        let source = config_root.join(directory);
        if !source.exists() {
            continue;
        }
        for entry in WalkDir::new(&source).follow_links(false) {
            let entry = entry.map_err(|error| {
                format!(
                    "Не удалось прочитать содержимое {}: {error}",
                    source.display()
                )
            })?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let relative = path.strip_prefix(config_root).map_err(|error| {
                format!(
                    "Не удалось определить относительный путь {}: {error}",
                    path.display()
                )
            })?;
            if !is_allowed_relative(relative) {
                continue;
            }
            let content = fs::read_to_string(path).map_err(|error| {
                format!("Не удалось прочитать {} как UTF-8: {error}", path.display())
            })?;
            if content.len() > MAX_FILE_BYTES {
                return Err(format!(
                    "Файл {} слишком велик: лимит {} МБ",
                    path.display(),
                    MAX_FILE_BYTES / 1024 / 1024
                ));
            }
            files.push(PackageFile {
                path: relative_to_package_path(relative),
                content,
            });
            if files.len() > MAX_FILES {
                return Err(format!("Слишком много файлов: лимит {MAX_FILES}"));
            }
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    if !files.iter().any(|file| {
        file.path.eq_ignore_ascii_case("match/base.yml")
            || file.path.eq_ignore_ascii_case("match/base.yaml")
    }) {
        return Err("Не найден match/base.yml или match/base.yaml".to_owned());
    }
    Ok(files)
}

fn read_and_validate_package(path: &Path) -> Result<(PackageDocument, usize), String> {
    if !path.is_file() {
        return Err(format!("Файл пакета не найден: {}", path.display()));
    }
    if !is_package_path(path) {
        return Err(format!(
            "Ожидается файл .{PACKAGE_EXTENSION}: {}",
            path.display()
        ));
    }
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Не удалось прочитать свойства {}: {error}", path.display()))?;
    if metadata.len() > MAX_PACKAGE_BYTES as u64 {
        return Err(format!(
            "Пакет слишком велик: лимит {} МБ",
            MAX_PACKAGE_BYTES / 1024 / 1024
        ));
    }
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("Не удалось прочитать {}: {error}", path.display()))?;
    let document: PackageDocument = serde_json::from_str(&raw)
        .map_err(|error| format!("Некорректный пакет rEspanso: {error}"))?;
    if document.format != PACKAGE_FORMAT {
        return Err("Неизвестный формат пакета конфигурации".to_owned());
    }
    if document.version != PACKAGE_VERSION {
        return Err(format!(
            "Версия пакета {} не поддерживается (ожидается {})",
            document.version, PACKAGE_VERSION
        ));
    }
    if document.files.is_empty() {
        return Err("Пакет не содержит файлов".to_owned());
    }
    if document.files.len() > MAX_FILES {
        return Err(format!("Слишком много файлов: лимит {MAX_FILES}"));
    }

    let mut seen = HashSet::new();
    let mut total_bytes = 0_usize;
    let mut has_base = false;
    let engine = Engine::new();
    for file in &document.files {
        let relative = validated_relative_path(&file.path)?;
        if !seen.insert(relative.clone()) {
            return Err(format!("Пакет содержит повторяющийся путь: {}", file.path));
        }
        if file.content.len() > MAX_FILE_BYTES {
            return Err(format!(
                "Файл {} превышает лимит {} МБ",
                file.path,
                MAX_FILE_BYTES / 1024 / 1024
            ));
        }
        total_bytes = total_bytes.saturating_add(file.content.len());
        let extension = relative
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default();
        if extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml") {
            serde_norway::from_str::<serde_json::Value>(&file.content)
                .map_err(|error| format!("Ошибка YAML в {}: {error}", file.path))?;
        } else if extension.eq_ignore_ascii_case("rhai") {
            engine
                .compile(&file.content)
                .map_err(|error| format!("Ошибка Rhai в {}: {error}", file.path))?;
        }
        has_base |= file.path.eq_ignore_ascii_case("match/base.yml")
            || file.path.eq_ignore_ascii_case("match/base.yaml");
    }
    if !has_base {
        return Err("Пакет не содержит match/base.yml или match/base.yaml".to_owned());
    }
    Ok((document, total_bytes))
}

fn validated_relative_path(raw: &str) -> Result<PathBuf, String> {
    if raw.is_empty() || raw.contains('\\') || raw.contains(':') {
        return Err(format!("Недопустимый путь в пакете: {raw:?}"));
    }
    let path = Path::new(raw);
    if !is_allowed_relative(path) {
        return Err(format!("Недопустимый путь в пакете: {raw}"));
    }
    Ok(path.to_path_buf())
}

fn is_allowed_relative(path: &Path) -> bool {
    let mut components = path.components();
    let Some(Component::Normal(root)) = components.next() else {
        return false;
    };
    if !matches!(root.to_str(), Some("config" | "match" | "scripts")) {
        return false;
    }
    if components.any(|component| !matches!(component, Component::Normal(_))) {
        return false;
    }
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    match root.to_str() {
        Some("config" | "match") => {
            extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml")
        }
        Some("scripts") => extension.eq_ignore_ascii_case("rhai"),
        _ => false,
    }
}

fn relative_to_package_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn unix_timestamp() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| format!("Системное время некорректно: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    fn seed_root(root: &Path) {
        fs::create_dir_all(root.join("config")).unwrap();
        fs::create_dir_all(root.join("match/medical")).unwrap();
        fs::create_dir_all(root.join("scripts/medical")).unwrap();
        fs::write(
            root.join("config/default.yml"),
            "show_notifications: true\n",
        )
        .unwrap();
        fs::write(
            root.join("match/base.yml"),
            "imports:\n  - medical/rules.yml\nmatches: []\n",
        )
        .unwrap();
        fs::write(root.join("match/medical/rules.yml"), "matches: []\n").unwrap();
        fs::write(root.join("scripts/medical/days.rhai"), "40 + 2").unwrap();
        fs::write(root.join("scripts/ignored.txt"), "not exported").unwrap();
    }

    #[test]
    fn exports_and_inspects_configuration() {
        let temp = TempDir::new("respanso-export").unwrap();
        seed_root(temp.path());
        let report = export_current(temp.path()).unwrap();
        assert_eq!(report.file_count, 4);
        assert!(report.path.is_file());
        let summary = inspect_package(&report.path).unwrap();
        assert_eq!(summary.file_count, 4);
        let raw = fs::read_to_string(report.path).unwrap();
        assert!(!raw.contains("ignored.txt"));
    }

    #[test]
    fn imports_package_and_creates_backup() {
        let source = TempDir::new("respanso-source").unwrap();
        seed_root(source.path());
        fs::write(source.path().join("scripts/medical/days.rhai"), "84 / 2").unwrap();
        let package = export_current(source.path()).unwrap();

        let target = TempDir::new("respanso-target").unwrap();
        seed_root(target.path());
        fs::write(target.path().join("scripts/medical/days.rhai"), "1").unwrap();
        let report = import_package(target.path(), &package.path).unwrap();
        assert!(report.backup.is_file());
        assert_eq!(
            fs::read_to_string(target.path().join("scripts/medical/days.rhai")).unwrap(),
            "84 / 2"
        );
    }

    #[test]
    fn rejects_path_traversal() {
        let temp = TempDir::new("respanso-traversal").unwrap();
        let package_path = temp.path().join(format!("bad.{PACKAGE_EXTENSION}"));
        let document = PackageDocument {
            format: PACKAGE_FORMAT.to_owned(),
            version: PACKAGE_VERSION,
            exported_at_unix: 1,
            files: vec![PackageFile {
                path: "../escape.yml".to_owned(),
                content: "matches: []\n".to_owned(),
            }],
        };
        fs::write(package_path.clone(), serde_json::to_vec(&document).unwrap()).unwrap();
        assert!(inspect_package(&package_path).is_err());
    }

    #[test]
    fn invalid_rhai_is_rejected_before_replacement() {
        let target = TempDir::new("respanso-invalid-rhai").unwrap();
        seed_root(target.path());
        let original = fs::read_to_string(target.path().join("scripts/medical/days.rhai")).unwrap();
        let package_path = target.path().join(format!("bad.{PACKAGE_EXTENSION}"));
        let document = PackageDocument {
            format: PACKAGE_FORMAT.to_owned(),
            version: PACKAGE_VERSION,
            exported_at_unix: 1,
            files: vec![
                PackageFile {
                    path: "match/base.yml".to_owned(),
                    content: "matches: []\n".to_owned(),
                },
                PackageFile {
                    path: "scripts/bad.rhai".to_owned(),
                    content: "let value = ;".to_owned(),
                },
            ],
        };
        fs::write(package_path.clone(), serde_json::to_vec(&document).unwrap()).unwrap();
        assert!(import_package(target.path(), &package_path).is_err());
        assert_eq!(
            fs::read_to_string(target.path().join("scripts/medical/days.rhai")).unwrap(),
            original
        );
    }
}

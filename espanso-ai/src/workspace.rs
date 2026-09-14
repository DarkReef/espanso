//! Sandboxed workspace access for MCP agents.
//! Only match/**/*.yml|yaml and scripts/**/*.rhai are visible or writable.
use rhai::Engine;
use serde_json::{json, Value as JsonValue};
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_WORKSPACE_FILE: u64 = 512 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceKind {
    Match,
    Script,
}

impl WorkspaceKind {
    fn label(self) -> &'static str {
        match self {
            Self::Match => "match",
            Self::Script => "script",
        }
    }
}

fn normalize_relative(relative: &str) -> Result<(PathBuf, WorkspaceKind), String> {
    if relative.is_empty() || relative.contains('\0') || relative.contains('\\') {
        return Err("Используйте относительный UTF-8 путь с разделителем /".into());
    }
    let path = Path::new(relative);
    if path.is_absolute() {
        return Err("Абсолютные пути запрещены".into());
    }
    let mut components = path.components();
    let Some(Component::Normal(first)) = components.next() else {
        return Err("Некорректный путь workspace".into());
    };
    for component in components {
        if !matches!(component, Component::Normal(_)) {
            return Err("Переходы . и .. запрещены".into());
        }
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let kind = if first == "match" {
        if extension != "yml" && extension != "yaml" {
            return Err("В match/ разрешены только .yml и .yaml".into());
        }
        WorkspaceKind::Match
    } else if first == "scripts" {
        if extension != "rhai" {
            return Err("В scripts/ разрешены только .rhai".into());
        }
        WorkspaceKind::Script
    } else {
        return Err("MCP имеет доступ только к match/ и scripts/".into());
    };
    Ok((path.to_path_buf(), kind))
}

fn resolve(root: &Path, relative: &str) -> Result<(PathBuf, WorkspaceKind), String> {
    let (relative, kind) = normalize_relative(relative)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err("Некорректный путь workspace".into());
        };
        current.push(part);
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() {
                return Err("Символические ссылки в MCP workspace запрещены".into());
            }
        }
    }
    Ok((current, kind))
}

fn fingerprint(bytes: &[u8]) -> String {
    // Deterministic FNV-1a fingerprint. It is only an optimistic concurrency token,
    // not a credential or a cryptographic integrity primitive.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = fs::metadata(path).map_err(|_| "Файл workspace не найден".to_owned())?;
    if !metadata.is_file() {
        return Err("Путь workspace не является файлом".into());
    }
    if metadata.len() > MAX_WORKSPACE_FILE {
        return Err("Файл больше 512 КБ и не может редактироваться через MCP".into());
    }
    fs::read(path).map_err(|error| format!("Не удалось прочитать workspace-файл: {error}"))
}

fn read_utf8(path: &Path) -> Result<(String, String), String> {
    let bytes = read_bytes(path)?;
    let hash = fingerprint(&bytes);
    let content = String::from_utf8(bytes).map_err(|_| "Workspace-файл не является UTF-8".to_owned())?;
    Ok((content, hash))
}

fn validate_match(content: &str) -> Result<(), String> {
    let value: serde_norway::Value = serde_norway::from_str(content)
        .map_err(|error| format!("Некорректный YAML: {error}"))?;
    let mapping = value
        .as_mapping()
        .ok_or_else(|| "Match YAML должен содержать корневой объект".to_owned())?;
    let matches_key = serde_norway::Value::String("matches".to_owned());
    if let Some(matches) = mapping.get(&matches_key) {
        if matches.as_sequence().is_none() {
            return Err("Поле matches должно быть YAML-списком".into());
        }
    }
    Ok(())
}

fn validate_script(content: &str) -> Result<(), String> {
    Engine::new()
        .compile(content)
        .map(|_| ())
        .map_err(|error| format!("Некорректный Rhai: {error}"))
}

pub fn validate(relative: &str, content: &str) -> Result<JsonValue, String> {
    if content.len() as u64 > MAX_WORKSPACE_FILE {
        return Err("Содержимое больше 512 КБ".into());
    }
    let (_, kind) = normalize_relative(relative)?;
    match kind {
        WorkspaceKind::Match => validate_match(content)?,
        WorkspaceKind::Script => validate_script(content)?,
    }
    Ok(json!({
        "valid": true,
        "path": relative,
        "kind": kind.label(),
        "bytes": content.len(),
        "hash": fingerprint(content.as_bytes())
    }))
}

fn collect_files(
    root: &Path,
    directory: &Path,
    scope: &str,
    output: &mut Vec<JsonValue>,
) -> Result<(), String> {
    if !directory.is_dir() {
        return Ok(());
    }
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("Не удалось прочитать workspace: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("Ошибка workspace: {error}"))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Ошибка workspace: {error}"))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_files(root, &path, scope, output)?;
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let relative_text = relative.to_string_lossy().replace('\\', "/");
        let Ok((_, kind)) = normalize_relative(&relative_text) else {
            continue;
        };
        if scope != "all" && scope != kind.label() && !(scope == "scripts" && kind == WorkspaceKind::Script) {
            continue;
        }
        if metadata.len() > MAX_WORKSPACE_FILE {
            output.push(json!({
                "path": relative_text,
                "kind": kind.label(),
                "bytes": metadata.len(),
                "tooLarge": true
            }));
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| format!("Ошибка workspace: {error}"))?;
        output.push(json!({
            "path": relative_text,
            "kind": kind.label(),
            "bytes": bytes.len(),
            "hash": fingerprint(&bytes)
        }));
    }
    Ok(())
}

pub fn list(root: &Path, scope: &str) -> Result<JsonValue, String> {
    if !["all", "match", "scripts"].contains(&scope) {
        return Err("scope должен быть all, match или scripts".into());
    }
    let mut files = Vec::new();
    if scope == "all" || scope == "match" {
        collect_files(root, &root.join("match"), scope, &mut files)?;
    }
    if scope == "all" || scope == "scripts" {
        collect_files(root, &root.join("scripts"), scope, &mut files)?;
    }
    files.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    Ok(json!({"files": files}))
}

pub fn read(root: &Path, relative: &str) -> Result<JsonValue, String> {
    let (path, kind) = resolve(root, relative)?;
    let (content, hash) = read_utf8(&path)?;
    Ok(json!({
        "path": relative,
        "kind": kind.label(),
        "hash": hash,
        "content": content
    }))
}

fn write_validated(path: &Path, content: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| "Нет каталога workspace".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| format!("Не удалось создать каталог: {error}"))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("Не удалось создать временный файл: {error}"))?;
    temp.write_all(content)
        .and_then(|_| temp.as_file().sync_all())
        .map_err(|error| format!("Ошибка записи workspace: {error}"))?;

    if !path.exists() {
        temp.persist(path)
            .map_err(|error| format!("Не удалось сохранить workspace-файл: {}", error.error))?;
        return Ok(());
    }

    let old_permissions = fs::metadata(path).ok().map(|metadata| metadata.permissions());
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("workspace");
    let backup = parent.join(format!(".{file_name}.respanso-mcp-backup"));
    if backup.exists() {
        return Err(format!(
            "Обнаружена незавершённая резервная копия {}. Проверьте файл вручную.",
            backup.display()
        ));
    }
    fs::rename(path, &backup)
        .map_err(|error| format!("Не удалось подготовить безопасную замену: {error}"))?;
    match temp.persist(path) {
        Ok(_) => {
            if let Some(permissions) = old_permissions {
                let _ = fs::set_permissions(path, permissions);
            }
            let _ = fs::remove_file(&backup);
            Ok(())
        }
        Err(error) => {
            let _ = fs::rename(&backup, path);
            Err(format!("Не удалось заменить workspace-файл: {}", error.error))
        }
    }
}

pub fn write(
    root: &Path,
    relative: &str,
    content: &str,
    expected_hash: Option<&str>,
    confirmed: bool,
    allowed: bool,
) -> Result<JsonValue, String> {
    if !allowed {
        return Err("Запись workspace через MCP отключена в локальных настройках".into());
    }
    if !confirmed {
        return Err("Изменение должно быть явно подтверждено пользователем".into());
    }
    validate(relative, content)?;
    let (path, kind) = resolve(root, relative)?;

    if path.exists() {
        let (current, current_hash) = read_utf8(&path)?;
        if current == content {
            return Ok(json!({
                "status": "unchanged",
                "path": relative,
                "kind": kind.label(),
                "hash": current_hash
            }));
        }
        let expected = expected_hash
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "Для изменения существующего файла нужен expected_hash из workspace_read".to_owned())?;
        if expected != current_hash {
            return Err("Файл изменился после чтения. Перечитайте его и повторите изменение.".into());
        }
    } else if expected_hash.is_some_and(|value| !value.is_empty()) {
        return Err("Файл ещё не существует; expected_hash для создания не нужен".into());
    }

    write_validated(&path, content.as_bytes())?;
    let (_, new_hash) = read_utf8(&path)?;
    Ok(json!({
        "status": "written",
        "path": relative,
        "kind": kind.label(),
        "hash": new_hash
    }))
}

pub fn delete(
    root: &Path,
    relative: &str,
    expected_hash: &str,
    confirmed: bool,
    allowed: bool,
) -> Result<JsonValue, String> {
    if !allowed {
        return Err("Удаление workspace через MCP отключено в локальных настройках".into());
    }
    if !confirmed {
        return Err("Удаление должно быть явно подтверждено пользователем".into());
    }
    let (path, kind) = resolve(root, relative)?;
    let (_, current_hash) = read_utf8(&path)?;
    if expected_hash != current_hash {
        return Err("Файл изменился после чтения. Перечитайте его перед удалением.".into());
    }

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Некорректное системное время".to_owned())?
        .as_nanos();
    let trash_root = root.join(".respanso-mcp-trash").join(stamp.to_string());
    let relative_path = normalize_relative(relative)?.0;
    let trash_path = trash_root.join(&relative_path);
    let trash_parent = trash_path
        .parent()
        .ok_or_else(|| "Некорректный путь корзины".to_owned())?;
    fs::create_dir_all(trash_parent)
        .map_err(|error| format!("Не удалось создать MCP-корзину: {error}"))?;
    fs::rename(&path, &trash_path)
        .map_err(|error| format!("Не удалось переместить файл в MCP-корзину: {error}"))?;
    Ok(json!({
        "status": "moved_to_trash",
        "path": relative,
        "kind": kind.label(),
        "trash": trash_path.strip_prefix(root).unwrap_or(&trash_path).to_string_lossy().replace('\\', "/")
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> tempdir::TempDir {
        let dir = tempdir::TempDir::new("mcp-workspace").unwrap();
        fs::create_dir_all(dir.path().join("match")).unwrap();
        fs::create_dir_all(dir.path().join("scripts")).unwrap();
        dir
    }

    #[test]
    fn rejects_path_escape_and_wrong_extensions() {
        for path in ["../secret.yml", "config/default.yml", "match/a.txt", "scripts/a.py"] {
            assert!(normalize_relative(path).is_err(), "{path}");
        }
    }

    #[test]
    fn validates_yaml_and_rhai() {
        assert!(validate("match/test.yml", "matches:\n  - trigger: ':a'\n    replace: 'b'\n").is_ok());
        assert!(validate("match/test.yml", "matches: [").is_err());
        assert!(validate("scripts/test.rhai", "let x = 1 + 2;").is_ok());
        assert!(validate("scripts/test.rhai", "let x = 1 + ;").is_err());
    }

    #[test]
    fn write_uses_optimistic_concurrency_and_utf8() {
        let dir = root();
        let first = "matches:\n  - trigger: ':тест'\n    replace: 'Привет'\n";
        let created = write(dir.path(), "match/test.yml", first, None, true, true).unwrap();
        let hash = created["hash"].as_str().unwrap().to_owned();
        let second = "matches:\n  - trigger: ':тест'\n    replace: 'Здравствуйте'\n";
        assert!(write(dir.path(), "match/test.yml", second, Some("stale"), true, true).is_err());
        let changed = write(dir.path(), "match/test.yml", second, Some(&hash), true, true).unwrap();
        assert_eq!(changed["status"], "written");
        let read_back = read(dir.path(), "match/test.yml").unwrap();
        assert!(read_back["content"].as_str().unwrap().contains("Здравствуйте"));
    }

    #[test]
    fn write_is_disabled_by_default_gate() {
        let dir = root();
        assert!(write(
            dir.path(),
            "scripts/test.rhai",
            "let x = 1;",
            None,
            true,
            false
        )
        .is_err());
    }

    #[test]
    fn delete_is_recoverable() {
        let dir = root();
        let created = write(
            dir.path(),
            "scripts/test.rhai",
            "let x = 1;",
            None,
            true,
            true,
        )
        .unwrap();
        let hash = created["hash"].as_str().unwrap();
        let result = delete(dir.path(), "scripts/test.rhai", hash, true, true).unwrap();
        assert_eq!(result["status"], "moved_to_trash");
        assert!(!dir.path().join("scripts/test.rhai").exists());
    }
}

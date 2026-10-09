use serde::Deserialize;
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportEntry {
    pub path: PathBuf,
    pub enabled: bool,
}

#[derive(Debug, Default, Deserialize)]
struct MatchFileImports {
    #[serde(default)]
    imports: Vec<String>,
}

pub fn find_base_file(files: &[PathBuf], match_root: &Path) -> Option<PathBuf> {
    ["base.yml", "base.yaml"]
        .into_iter()
        .map(|name| match_root.join(name))
        .find(|candidate| files.iter().any(|file| same_path(file, candidate)))
        .or_else(|| {
            files
                .iter()
                .find(|file| {
                    file.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| {
                            name.eq_ignore_ascii_case("base.yml")
                                || name.eq_ignore_ascii_case("base.yaml")
                        })
                })
                .cloned()
        })
}

pub fn import_entries(
    files: &[PathBuf],
    base_file: &Path,
    base_content: &str,
) -> Result<Vec<ImportEntry>, String> {
    let shape = parse_imports(base_content)?;
    let enabled = shape
        .imports
        .iter()
        .map(|import| path_key(&resolve_import(base_file, import)))
        .collect::<HashSet<_>>();

    Ok(files
        .iter()
        .filter(|file| !same_path(file, base_file))
        .map(|file| ImportEntry {
            path: file.clone(),
            enabled: enabled.contains(&path_key(file)),
        })
        .collect())
}

pub fn update_import(
    base_content: &str,
    base_file: &Path,
    target_file: &Path,
    enabled: bool,
) -> Result<String, String> {
    if same_path(base_file, target_file) {
        return Err(
            "base.yml является основным файлом и не может импортировать сам себя".to_owned(),
        );
    }

    let shape = parse_imports(base_content)?;
    let target_key = path_key(target_file);
    let mut seen = HashSet::new();
    let mut imports = Vec::new();

    for import in shape.imports {
        let resolved_key = path_key(&resolve_import(base_file, &import));
        if resolved_key == target_key {
            continue;
        }
        if seen.insert(resolved_key) {
            imports.push(import);
        }
    }

    if enabled {
        let relative = relative_import_path(base_file, target_file)?;
        let resolved_key = path_key(&resolve_import(base_file, &relative));
        if seen.insert(resolved_key) {
            imports.push(relative);
        }
    }

    let updated = replace_imports_section(base_content, &imports);
    parse_imports(&updated)?;
    Ok(updated)
}

fn parse_imports(content: &str) -> Result<MatchFileImports, String> {
    serde_norway::from_str::<MatchFileImports>(content)
        .map_err(|error| format!("Не удалось прочитать imports в base.yml: {error}"))
}

fn relative_import_path(base_file: &Path, target_file: &Path) -> Result<String, String> {
    let parent = base_file
        .parent()
        .ok_or_else(|| "Не удалось определить папку base.yml".to_owned())?;
    let relative = target_file.strip_prefix(parent).map_err(|_| {
        format!(
            "Файл {} находится вне папки base.yml",
            target_file.display()
        )
    })?;

    let value = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");

    if value.is_empty() {
        Err("Не удалось сформировать относительный путь импорта".to_owned())
    } else {
        Ok(value)
    }
}

fn replace_imports_section(content: &str, imports: &[String]) -> String {
    let newline = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let section = top_level_section(content, "imports");
    let rendered = render_imports(imports, newline);

    match section {
        Some((start, end)) => {
            let mut result = String::with_capacity(content.len() + rendered.len());
            result.push_str(&content[..start]);
            result.push_str(&rendered);
            result.push_str(&content[end..]);
            result
        }
        None if imports.is_empty() => content.to_owned(),
        None => {
            let insertion =
                top_level_section(content, "matches").map_or(content.len(), |(start, _)| start);
            let mut result = String::with_capacity(content.len() + rendered.len() + newline.len());
            result.push_str(&content[..insertion]);
            let prefix = &content[..insertion];
            if insertion > 0 && !prefix.ends_with('\n') && !prefix.ends_with('\r') {
                result.push_str(newline);
            }
            result.push_str(&rendered);
            result.push_str(&content[insertion..]);
            result
        }
    }
}

fn render_imports(imports: &[String], newline: &str) -> String {
    if imports.is_empty() {
        return String::new();
    }

    let mut result = format!("imports:{newline}");
    for import in imports {
        let encoded = serde_json::to_string(import).unwrap_or_else(|_| "\"\"".to_owned());
        result.push_str("  - ");
        result.push_str(&encoded);
        result.push_str(newline);
    }
    result.push_str(newline);
    result
}

fn top_level_section(content: &str, key: &str) -> Option<(usize, usize)> {
    let ranges = line_ranges(content);
    for (index, (start, end)) in ranges.iter().copied().enumerate() {
        let line = &content[start..end];
        let trimmed = trim_line(line);
        if trimmed.is_empty() || trimmed.starts_with('#') || indentation(line) != 0 {
            continue;
        }
        if !is_key_line(trimmed, key) {
            continue;
        }

        let mut section_end = content.len();
        for (next_start, next_end) in ranges.iter().copied().skip(index + 1) {
            let next_line = &content[next_start..next_end];
            let next_trimmed = trim_line(next_line);
            if next_trimmed.is_empty() || next_trimmed.starts_with('#') {
                continue;
            }
            if indentation(next_line) == 0 {
                section_end = next_start;
                break;
            }
        }
        return Some((start, section_end));
    }
    None
}

fn line_ranges(content: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for segment in content.split_inclusive('\n') {
        let end = start + segment.len();
        ranges.push((start, end));
        start = end;
    }
    if start < content.len() {
        ranges.push((start, content.len()));
    }
    ranges
}

fn indentation(line: &str) -> usize {
    line.chars()
        .take_while(|character| *character == ' ')
        .count()
}

fn trim_line(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n']).trim_start()
}

fn is_key_line(trimmed: &str, key: &str) -> bool {
    trimmed
        .strip_prefix(key)
        .is_some_and(|rest| rest.trim_start().starts_with(':'))
}

fn resolve_import(source: &Path, import: &str) -> PathBuf {
    let import_path = PathBuf::from(import);
    if import_path.is_absolute() {
        normalize_path(&import_path)
    } else {
        normalize_path(
            &source
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(import_path),
        )
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

fn path_key(path: &Path) -> String {
    let key = normalize_path(path).to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        key.to_lowercase()
    } else {
        key
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    path_key(left) == path_key(right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_enabled_files_from_base_imports() {
        let root = PathBuf::from("config/match");
        let base = root.join("base.yml");
        let files = vec![
            base.clone(),
            root.join("medical.yml"),
            root.join("sub/tools.yaml"),
        ];
        let entries =
            import_entries(&files, &base, "imports:\n  - medical.yml\nmatches: []\n").unwrap();

        assert_eq!(entries.len(), 2);
        assert!(entries[0].enabled);
        assert!(!entries[1].enabled);
    }

    #[test]
    fn adds_import_before_matches_without_rewriting_rules() {
        let base = PathBuf::from("config/match/base.yml");
        let target = PathBuf::from("config/match/medical.yml");
        let original = "# header\nmatches:\n  - trigger: :a\n    replace: A\n";
        let updated = update_import(original, &base, &target, true).unwrap();

        assert!(updated.contains("imports:\n  - \"medical.yml\"\n\n"));
        assert!(updated.contains("  - trigger: :a\n    replace: A\n"));
    }

    #[test]
    fn removes_only_selected_import_and_keeps_unknown_entries() {
        let base = PathBuf::from("config/match/base.yml");
        let target = PathBuf::from("config/match/medical.yml");
        let original =
            "imports:\n  - medical.yml\n  - missing.yml\n\nmatches:\n  - trigger: :a\n    replace: A\n";
        let updated = update_import(original, &base, &target, false).unwrap();

        assert!(!updated.contains("medical.yml"));
        assert!(updated.contains("missing.yml"));
        assert!(updated.contains("matches:"));
    }

    #[test]
    fn preserves_crlf_when_rendering_imports() {
        let base = PathBuf::from("config/match/base.yml");
        let target = PathBuf::from("config/match/sub/tools.yaml");
        let updated = update_import("matches: []\r\n", &base, &target, true).unwrap();

        assert!(updated.contains("imports:\r\n  - \"sub/tools.yaml\"\r\n\r\n"));
    }
}

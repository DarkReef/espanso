from pathlib import Path
import base64
import re
import shutil
import subprocess
import textwrap


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


def replace_between(text: str, start_marker: str, end_marker: str, new: str, label: str) -> str:
    try:
        start = text.index(start_marker)
        end = text.index(end_marker, start)
    except ValueError as error:
        raise SystemExit(f"{label}: boundary not found: {error}") from error
    return text[:start] + new + text[end:]


def insert_before(text: str, marker: str, addition: str, label: str) -> str:
    count = text.count(marker)
    if count != 1:
        raise SystemExit(f"{label}: marker count is {count}")
    return text.replace(marker, addition + marker, 1)


def rust(text: str, indent: int = 0) -> str:
    normalized = textwrap.dedent(text).strip("\n") + "\n"
    return textwrap.indent(normalized, " " * indent)


app_path = Path("espanso-editor/src/app.rs")
app = app_path.read_text(encoding="utf-8")

app = replace_once(
    app,
    "use std::{\n    path::{Path, PathBuf},\n    time::{Duration, Instant},\n};",
    "use std::{\n    fs,\n    io::Write as _,\n    path::{Path, PathBuf},\n    time::{Duration, Instant},\n};",
    "app imports",
)

app = replace_once(
    app,
    "    dynamic_variables: DynamicVariableDialog,\n",
    "    dynamic_variables: DynamicVariableDialog,\n"
    "    show_create_yaml_file: bool,\n"
    "    new_yaml_file_name: String,\n"
    "    confirm_delete_yaml_file: bool,\n"
    "    pending_delete_yaml_file: Option<PathBuf>,\n",
    "yaml file state fields",
)
app = replace_once(
    app,
    "            dynamic_variables: DynamicVariableDialog::default(),\n",
    "            dynamic_variables: DynamicVariableDialog::default(),\n"
    "            show_create_yaml_file: false,\n"
    "            new_yaml_file_name: \"rules.yml\".to_owned(),\n"
    "            confirm_delete_yaml_file: false,\n"
    "            pending_delete_yaml_file: None,\n",
    "yaml file state initialization",
)

yaml_methods = rust(r'''
fn create_yaml_file(&mut self) -> bool {
    if self.has_transfer_unsaved_changes() || self.settings.dirty() {
        "Сначала сохраните текущие изменения, затем создайте YAML-файл"
            .clone_into(&mut self.status);
        return false;
    }

    let name = match normalize_yaml_file_name(&self.new_yaml_file_name) {
        Ok(name) => name,
        Err(error) => {
            self.status = error;
            return false;
        }
    };
    let path = self.config_root.join("match").join(&name);
    let result = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .and_then(|mut file| {
            file.write_all(b"matches:\n  - trigger: \":new\"\n    replace: \"\"\n")
        });
    if let Err(error) = result {
        self.status = if path.exists() {
            format!("YAML-файл {name} уже существует")
        } else {
            format!("Не удалось создать YAML-файл {name}: {error}")
        };
        return false;
    }

    self.reload();
    if self.load_error.is_some() {
        return false;
    }
    self.file_filter = Some(path.clone());
    self.select_rule(RuleId { file: path, ordinal: 0 });
    self.new_yaml_file_name = "rules.yml".to_owned();
    self.status = format!("Создан match\\{name}. Добавьте правила и сохраните их Ctrl+S");
    true
}

fn delete_yaml_file(&mut self, path: PathBuf) -> bool {
    if self.has_transfer_unsaved_changes() || self.settings.dirty() {
        "Сначала сохраните текущие изменения, затем удалите YAML-файл"
            .clone_into(&mut self.status);
        return false;
    }

    let Some(workspace) = &self.workspace else {
        return false;
    };
    let files = workspace.files();
    if !files.iter().any(|file| file == &path) {
        "Выбранный YAML-файл больше не входит в рабочую область"
            .clone_into(&mut self.status);
        return false;
    }
    if yaml_imports::find_base_file(&files, workspace.match_root()).as_ref() == Some(&path) {
        "Нельзя удалить основной base.yml/base.yaml".clone_into(&mut self.status);
        return false;
    }

    let display_name = relative_display(&self.config_root, &path);
    if let Err(error) = fs::remove_file(&path) {
        self.status = format!("Не удалось удалить {display_name}: {error}");
        return false;
    }
    self.file_filter = None;
    self.selected = None;
    self.raw_rule.clear();
    self.reload();
    self.status = format!("Удалён {display_name}");
    true
}

fn sync_builtin_variables(&mut self, id: &RuleId) -> Result<usize, String> {
    let definitions = dynamic_variables::builtin_definitions_in(&self.draft.replace);
    if definitions.is_empty() {
        return Ok(0);
    }

    let mut raw = self.raw_rule.clone();
    let mut added = 0_usize;
    for definition in definitions {
        let (updated, was_added) = dynamic_variables::upsert_rule_variable(&raw, &definition)?;
        raw = updated;
        added += usize::from(was_added);
    }

    if raw != self.raw_rule {
        let workspace = self
            .workspace
            .as_mut()
            .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())?;
        workspace
            .update_rule_raw(id, &raw)
            .map_err(|error| ru_message(&error.to_string()))?;
        self.raw_rule = raw;
    }
    Ok(added)
}

''', 4)
app = insert_before(app, "    fn create_rule(&mut self) {\n", yaml_methods, "yaml file methods")

app = replace_between(
    app,
    "    fn apply_current(&mut self) {",
    "    fn apply_structured(&mut self) {",
    "",
    "remove apply_current",
)

apply_structured = rust(r'''
fn apply_structured(&mut self) {
    let Some(id) = self.selected.clone() else {
        return;
    };
    let result = self
        .workspace
        .as_mut()
        .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())
        .and_then(|workspace| {
            workspace
                .update_rule(&id, &self.draft)
                .map_err(|error| ru_message(&error.to_string()))?;
            workspace
                .rule(&id)
                .map(|rule| rule.raw)
                .map_err(|error| ru_message(&error.to_string()))
        });

    match result {
        Ok(raw) => {
            self.raw_rule = raw;
            match self.sync_builtin_variables(&id) {
                Ok(0) => "Изменения ожидают сохранения (Ctrl+S)"
                    .clone_into(&mut self.status),
                Ok(count) => {
                    self.status = format!(
                        "Изменения ожидают сохранения. Автоматически объявлено переменных: {count}"
                    );
                }
                Err(error) => {
                    self.status = format!("Не удалось объявить переменную: {error}");
                }
            }
        }
        Err(error) => self.status = error,
    }
}

''', 4)
app = replace_between(
    app,
    "    fn apply_structured(&mut self) {",
    "    fn apply_raw(&mut self) {",
    apply_structured,
    "live structured editor backend",
)

add_dynamic = rust(r'''
fn add_dynamic_variable(&mut self, action: DynamicVariableAction) {
    let Some(id) = self.selected.clone() else {
        "Сначала выберите правило".clone_into(&mut self.status);
        return;
    };
    let previous_draft = self.draft.clone();
    self.draft.replace.push_str(&action.placeholder);

    let result = self
        .workspace
        .as_mut()
        .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())
        .and_then(|workspace| {
            workspace
                .update_rule(&id, &self.draft)
                .map_err(|error| ru_message(&error.to_string()))?;
            let raw = workspace
                .rule(&id)
                .map(|rule| rule.raw)
                .map_err(|error| ru_message(&error.to_string()))?;
            let (updated, added) =
                dynamic_variables::upsert_rule_variable(&raw, &action.definition)?;
            workspace
                .update_rule_raw(&id, &updated)
                .map_err(|error| ru_message(&error.to_string()))?;
            Ok((updated, added))
        });

    match result {
        Ok((updated, added)) => {
            self.raw_rule = updated;
            self.status = if added {
                format!("{}. Нажмите Ctrl+S для записи YAML", action.message)
            } else {
                format!(
                    "Переменная {} уже объявлена; шаблон добавлен в текст. Нажмите Ctrl+S",
                    action.placeholder
                )
            };
        }
        Err(error) => {
            self.draft = previous_draft;
            if let Some(workspace) = &mut self.workspace {
                let _ = workspace.update_rule(&id, &self.draft);
      
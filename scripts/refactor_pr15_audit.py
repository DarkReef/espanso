from pathlib import Path
import re


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"anchor not found: {label}")
    return text.replace(old, new, 1)


def replace_regex(text: str, pattern: str, replacement: str, label: str) -> str:
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.S)
    if count != 1:
        raise SystemExit(f"regex anchor not found or ambiguous: {label} ({count})")
    return updated


lib_path = Path("espanso-editor/src/lib.rs")
lib = lib_path.read_text(encoding="utf-8")
lib = replace_once(
    lib,
    "pub mod global_variables;\n",
    "pub mod global_variables;\npub mod trigger_conversion;\npub mod yaml_files;\n",
    "lib module declarations",
)
lib_path.write_text(lib, encoding="utf-8")


dynamic_path = Path("espanso-editor/src/dynamic_variables.rs")
dynamic = dynamic_path.read_text(encoding="utf-8")
new_builtin_fn = '''#[must_use]
pub fn newly_completed_builtin_definitions(
    previous: &str,
    current: &str,
) -> Vec<VariableDefinition> {
    let previous_names = builtin_definitions_in(previous)
        .into_iter()
        .map(|definition| definition.name)
        .collect::<std::collections::HashSet<_>>();
    builtin_definitions_in(current)
        .into_iter()
        .filter(|definition| !previous_names.contains(&definition.name))
        .collect()
}

'''
dynamic = replace_once(
    dynamic,
    "pub fn canonical_definition(definition: &VariableDefinition) -> Result<VariableDefinition, String> {",
    new_builtin_fn
    + "pub fn canonical_definition(definition: &VariableDefinition) -> Result<VariableDefinition, String> {",
    "new builtin detector insertion",
)
new_builtin_test = '''    #[test]
    fn detects_only_newly_completed_builtin_placeholders() {
        let definitions = newly_completed_builtin_definitions(
            "Дата {{dat",
            "Дата {{date}}, время {{time}}",
        );
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.name.as_str())
                .collect::<Vec<_>>(),
            vec!["date", "time"]
        );
        assert!(newly_completed_builtin_definitions(
            "Дата {{date}}",
            "Дата: {{date}}"
        )
        .is_empty());
    }

'''
dynamic = replace_once(
    dynamic,
    "    #[test]\n    fn time_alias_uses_date_extension() {",
    new_builtin_test + "    #[test]\n    fn time_alias_uses_date_extension() {",
    "new builtin detector test",
)
dynamic_path.write_text(dynamic, encoding="utf-8")


app_path = Path("espanso-editor/src/app.rs")
app = app_path.read_text(encoding="utf-8")
app = replace_once(
    app,
    "    dynamic_variables,\n",
    "    dynamic_variables,\n    trigger_conversion,\n    yaml_files,\n",
    "app module imports",
)
app = app.replace("use walkdir::WalkDir;\n", "", 1)
app = app.replace('const DISABLED_YAML_SUFFIX: &str = ".disabled";\n', "", 1)
app = app.replace('const DISABLED_IMPORTED_YAML_SUFFIX: &str = ".disabled.imported";\n', "", 1)
app = replace_regex(
    app,
    r"\n#\[derive\(Debug, Clone\)\]\nstruct YamlFileEntry \{.*?\n\}\n",
    "\n",
    "YamlFileEntry extraction",
)


toggle_fn = '''    fn set_yaml_file_enabled(&mut self, file: PathBuf, enabled: bool) {
        if self.has_transfer_unsaved_changes() || self.settings.dirty() {
            "Сначала сохраните изменения, затем включайте или выключайте YAML-файл"
                .clone_into(&mut self.status);
            return;
        }

        let display_name = yaml_files::display_name(
            &self.config_root,
            &file,
            !yaml_files::is_disabled(&file),
        );
        let workspace_snapshot = self
            .workspace
            .as_ref()
            .map(MatchWorkspace::working_snapshot)
            .unwrap_or_default();
        let result = (|| -> Result<PathBuf, String> {
            if !file.is_file() {
                return Err(format!("Файл не найден: {}", file.display()));
            }

            let workspace = self
                .workspace
                .as_mut()
                .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())?;
            let files = workspace.files();
            let base_file = yaml_imports::find_base_file(&files, workspace.match_root());
            let logical_enabled_path = if enabled {
                yaml_files::enabled_path(&file)
                    .ok_or_else(|| format!("Файл {display_name} уже включён"))?
            } else {
                file.clone()
            };
            if base_file.as_ref() == Some(&logical_enabled_path) {
                return Err("Нельзя выключить основной base.yml/base.yaml".to_owned());
            }

            let mut base_update = None;
            let target = if enabled {
                let target = logical_enabled_path;
                if yaml_files::is_imported_disabled(&file) {
                    if let Some(base_file) = &base_file {
                        let base_content = workspace
                            .raw_file(base_file)
                            .map_err(|error| ru_message(&error.to_string()))?
                            .to_owned();
                        let updated =
                            yaml_imports::update_import(&base_content, base_file, &target, true)?;
                        base_update = Some((base_file.clone(), updated));
                    }
                }
                target
            } else {
                let mut was_imported = false;
                if let Some(base_file) = &base_file {
                    let base_content = workspace
                        .raw_file(base_file)
                        .map_err(|error| ru_message(&error.to_string()))?
                        .to_owned();
                    was_imported = yaml_imports::import_entries(&files, base_file, &base_content)?
                        .into_iter()
                        .any(|entry| entry.path == file && entry.enabled);
                    if was_imported {
                        let updated =
                            yaml_imports::update_import(&base_content, base_file, &file, false)?;
                        base_update = Some((base_file.clone(), updated));
                    }
                }
                yaml_files::disabled_path(&file, was_imported)
            };

            if target.exists() {
                return Err(format!("Целевой файл уже существует: {}", target.display()));
            }

            fs::rename(&file, &target).map_err(|error| {
                format!(
                    "Не удалось переименовать {} в {}: {error}",
                    file.display(),
                    target.display()
                )
            })?;

            if let Some((base_file, updated)) = base_update {
                let update_result = workspace
                    .set_raw_file(&base_file, updated)
                    .map_err(|error| ru_message(&error.to_string()))
                    .and_then(|()| {
                        workspace
                            .save_all()
                            .map(|_| ())
                            .map_err(|error| ru_message(&error.to_string()))
                    });
                if let Err(error) = update_result {
                    workspace.restore_working_snapshot(&workspace_snapshot);
                    let rollback = fs::rename(&target, &file);
                    return Err(match rollback {
                        Ok(()) => format!(
                            "Не удалось обновить imports; переименование и рабочая копия восстановлены: {error}"
                        ),
                        Err(rollback_error) => format!(
                            "Не удалось обновить imports ({error}); файл сохранён по адресу {} и не удалось вернуть исходное имя ({rollback_error})",
                            target.display()
                        ),
                    });
                }
            }
            Ok(target)
        })();

        match result {
            Ok(target) => {
                self.selected = None;
                self.raw_rule.clear();
                self.reload();
                self.file_filter = Some(target);
                self.status = if enabled {
                    format!(
                        "Файл {display_name} включён; прежняя запись imports восстановлена. Перезагрузите конфигурацию или перезапустите rEspanso"
                    )
                } else {
                    format!(
                        "Файл {display_name} выключен расширением .disabled и больше не загружается. Состояние imports сохранено для обратного включения"
                    )
                };
            }
            Err(error) => {
                self.status = format!("Не удалось изменить состояние {display_name}: {error}");
            }
        }
    }

'''
app = replace_regex(
    app,
    r"    fn set_yaml_file_enabled\(.*?\n    fn create_yaml_file",
    toggle_fn + "    fn create_yaml_file",
    "transactional YAML toggle",
)


delete_fn = '''    fn delete_yaml_file(&mut self, path: PathBuf) -> bool {
        if self.has_transfer_unsaved_changes() || self.settings.dirty() {
            "Сначала сохраните текущие изменения, затем удалите YAML-файл"
                .clone_into(&mut self.status);
            return false;
        }

        let disabled = yaml_files::is_disabled(&path);
        let logical_path = if disabled {
            let Some(path) = yaml_files::enabled_path(&path) else {
                "Не удалось определить исходное имя выключенного YAML-файла"
                    .clone_into(&mut self.status);
                return false;
            };
            path
        } else {
            path.clone()
        };
        let display_name = yaml_files::display_name(&self.config_root, &path, !disabled);
        let workspace_snapshot = self
            .workspace
            .as_ref()
            .map(MatchWorkspace::working_snapshot)
            .unwrap_or_default();

        let result = (|| -> Result<PathBuf, String> {
            let workspace = self
                .workspace
                .as_mut()
                .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())?;
            let files = workspace.files();
            if disabled {
                if !path.is_file() {
                    return Err("Выбранный выключенный YAML-файл больше не существует".to_owned());
                }
            } else if !files.iter().any(|file| file == &path) {
                return Err("Выбранный YAML-файл больше не входит в рабочую область".to_owned());
            }

            let base_file = yaml_imports::find_base_file(&files, workspace.match_root());
            if base_file.as_ref() == Some(&logical_path) {
                return Err("Нельзя удалить основной base.yml/base.yaml".to_owned());
            }

            let mut base_update = None;
            if let Some(base_file) = base_file {
                let base_content = workspace
                    .raw_file(&base_file)
                    .map_err(|error| {
                        format!(
                            "Не удалось прочитать imports перед удалением {display_name}: {}",
                            ru_message(&error.to_string())
                        )
                    })?
                    .to_owned();
                let updated = yaml_imports::update_import(
                    &base_content,
                    &base_file,
                    &logical_path,
                    false,
                )?;
                if updated != base_content {
                    base_update = Some((base_file, updated));
                }
            }

            let quarantine = yaml_files::deletion_quarantine_path(&path)?;
            if quarantine.exists() {
                return Err(format!(
                    "Удаление остановлено: уже существует файл безопасного карантина {}",
                    quarantine.display()
                ));
            }
            fs::rename(&path, &quarantine).map_err(|error| {
                format!(
                    "Не удалось переместить {display_name} в безопасный карантин {}: {error}",
                    quarantine.display()
                )
            })?;

            if let Some((base_file, updated)) = base_update {
                let save_result = workspace
                    .set_raw_file(&base_file, updated)
                    .map_err(|error| ru_message(&error.to_string()))
                    .and_then(|()| {
                        workspace
                            .save_all()
                            .map(|_| ())
                            .map_err(|error| ru_message(&error.to_string()))
                    });
                if let Err(error) = save_result {
                    workspace.restore_working_snapshot(&workspace_snapshot);
                    let rollback = fs::rename(&quarantine, &path);
                    return Err(match rollback {
                        Ok(()) => format!(
                            "Удаление отменено: imports не сохранены ({error}); исходный файл и рабочая копия восстановлены"
                        ),
                        Err(rollback_error) => format!(
                            "Удаление imports отменено ({error}), но файл остался в карантине {} и не вернулся на место ({rollback_error})",
                            quarantine.display()
                        ),
                    });
                }
            }
            Ok(quarantine)
        })();

        match result {
            Ok(quarantine) => {
                let cleanup_error = fs::remove_file(&quarantine).err();
                self.file_filter = None;
                self.selected = None;
                self.raw_rule.clear();
                self.reload();
                self.status = cleanup_error.map_or_else(
                    || format!("Удалён {display_name}; связанные imports также очищены"),
                    |error| {
                        format!(
                            "{display_name} отключён и исключён из imports, но карантинный файл {} не удалось окончательно удалить: {error}",
                            quarantine.display()
                        )
                    },
                );
                true
            }
            Err(error) => {
                self.status = format!("Удаление {display_name} остановлено: {error}");
                false
            }
        }
    }

'''
app = replace_regex(
    app,
    r"    fn delete_yaml_file\(.*?\n    fn global_variable_records",
    delete_fn + "    fn global_variable_records",
    "transactional YAML deletion",
)


sync_fn = '''    fn sync_builtin_variables(
        &mut self,
        definitions: Vec<dynamic_variables::VariableDefinition>,
    ) -> Result<usize, String> {
        if definitions.is_empty() {
            return Ok(0);
        }

        let mut existing = self
            .global_variable_records()?
            .into_iter()
            .map(|record| record.definition.name)
            .collect::<HashSet<_>>();
        let missing = definitions
            .into_iter()
            .filter(|definition| existing.insert(definition.name.clone()))
            .collect::<Vec<_>>();
        if missing.is_empty() {
            return Ok(0);
        }

        let base_file = self.global_variable_base_file()?;
        let workspace = self
            .workspace
            .as_mut()
            .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())?;
        let mut content = workspace
            .raw_file(&base_file)
            .map_err(|error| ru_message(&error.to_string()))?
            .to_owned();
        let mut added = 0_usize;
        for definition in missing {
            let (updated, was_added) =
                global_variables::ensure_global_variable(&content, &definition)?;
            content = updated;
            added += usize::from(was_added);
        }
        workspace
            .set_raw_file(&base_file, content)
            .map_err(|error| ru_message(&error.to_string()))?;
        Ok(added)
    }

'''
app = replace_regex(
    app,
    r"    fn sync_builtin_variables\(.*?\n    fn insert_global_placeholder",
    sync_fn + "    fn insert_global_placeholder",
    "builtin synchronization service",
)


apply_fn = '''    fn apply_structured(
        &mut self,
        builtin_definitions: Vec<dynamic_variables::VariableDefinition>,
    ) {
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
                match self.sync_builtin_variables(builtin_definitions) {
                    Ok(0) => {
                        "Изменения ожидают сохранения (Ctrl+S)".clone_into(&mut self.status);
                    }
                    Ok(count) => {
                        self.status = format!(
                            "Изменения ожидают сохранения. Автоматически объявлено общих переменных: {count}"
                        );
                    }
                    Err(error) => {
                        self.status = format!(
                            "Правило изменено, но общую переменную объявить не удалось: {error}"
                        );
                    }
                }
            }
            Err(error) => self.status = error,
        }
    }

'''
app = replace_regex(
    app,
    r"    fn apply_structured\(.*?\n    fn apply_raw",
    apply_fn + "    fn apply_raw",
    "structured apply refactor",
)


new_transition = '''        if previous_draft.kind == MatchKind::Trigger && self.draft.kind == MatchKind::Regex {
            let conversion = trigger_conversion::from_triggers(&previous_draft.triggers);
            self.draft.regex = conversion.pattern;
            if !conversion.examples.is_empty() {
                self.regex_examples_text = conversion.examples;
            }
            self.refresh_regex_examples();
        }
'''
app = replace_regex(
    app,
    r"        if previous_draft\.kind == MatchKind::Trigger && self\.draft\.kind == MatchKind::Regex \{.*?\n        \}\n(?=        ui\.horizontal\(\|ui\| \{)",
    new_transition,
    "trigger to regexp transition",
)
app = replace_once(
    app,
    '''        if self.draft != previous_draft {
            self.apply_structured();
        }
''',
    '''        if self.draft != previous_draft {
            let new_builtins = dynamic_variables::newly_completed_builtin_definitions(
                &previous_draft.replace,
                &self.draft.replace,
            );
            self.apply_structured(new_builtins);
        }
''',
    "structured editor apply call",
)


replacements = {
    "yaml_file_entries(": "yaml_files::entries(",
    "yaml_display_name(": "yaml_files::display_name(",
    "is_disabled_yaml_path(": "yaml_files::is_disabled(",
    "enabled_yaml_path(": "yaml_files::enabled_path(",
    "is_imported_disabled_yaml_path(": "yaml_files::is_imported_disabled(",
    "disabled_yaml_path(": "yaml_files::disabled_path(",
    "normalize_yaml_file_name(": "yaml_files::normalize_file_name(",
}
for old, new in replacements.items():
    app = app.replace(old, new)

app = replace_regex(
    app,
    r"\nfn yaml_files::entries\(.*?\nfn relative_display",
    "\nfn relative_display",
    "YAML helper extraction",
)
app = replace_regex(
    app,
    r"\n#\[cfg\(test\)\]\nmod tests \{.*?\n\}\s*\Z",
    "\n",
    "app helper tests extraction",
)

stale = [
    "YamlFileEntry",
    "WalkDir",
    "literal_trigger_regex",
    "fn yaml_file_entries",
    "DISABLED_YAML_SUFFIX",
    "DISABLED_IMPORTED_YAML_SUFFIX",
    "self.apply_structured();",
    "sync_builtin_variables(&id)",
]
remaining = [item for item in stale if item in app]
if remaining:
    raise SystemExit(f"stale app symbols after refactor: {remaining}")
app_path.write_text(app, encoding="utf-8")

from __future__ import annotations

import re
from pathlib import Path

APP = Path("espanso-editor/src/app.rs")
DYNAMIC = Path("espanso-editor/src/dynamic_variables.rs")
GLOBAL = Path("espanso-editor/src/global_variables.rs")


def replace_once(source: str, old: str, new: str, label: str) -> str:
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one occurrence, found {count}")
    return source.replace(old, new, 1)


def replace_regex(source: str, pattern: str, replacement: str, label: str) -> str:
    updated, count = re.subn(pattern, lambda _match: replacement, source, count=1, flags=re.S)
    if count != 1:
        raise RuntimeError(f"{label}: expected one regex match, found {count}")
    return updated


app = APP.read_text(encoding="utf-8")
app = replace_once(
    app,
    "    dynamic_variables::{self, DynamicVariableAction, DynamicVariableDialog},\n"
    "    file_monitor::{FileMonitor, PollResult},\n",
    "    dynamic_variables::{self, VariableDefinition},\n"
    "    file_monitor::{FileMonitor, PollResult},\n"
    "    global_variables::{\n"
    "        self, GlobalVariableEditor, GlobalVariableEditorAction, GlobalVariableRecord,\n"
    "    },\n",
    "imports",
)
app = replace_once(
    app,
    "use std::{\n    fs,\n",
    "use std::{\n    collections::HashSet,\n    fs,\n",
    "hashset import",
)
app = replace_once(
    app,
    "    show_dynamic_variables: bool,\n    dynamic_variables: DynamicVariableDialog,\n",
    "    show_global_variables: bool,\n    global_variables: GlobalVariableEditor,\n",
    "state fields",
)
app = replace_once(
    app,
    "            show_dynamic_variables: false,\n            dynamic_variables: DynamicVariableDialog::default(),\n",
    "            show_global_variables: false,\n            global_variables: GlobalVariableEditor::default(),\n",
    "state init",
)

new_variable_methods = r'''    fn global_variable_records(&self) -> Result<Vec<GlobalVariableRecord>, String> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())?;
        let mut records = Vec::new();
        for file in workspace.files() {
            let content = workspace
                .raw_file(&file)
                .map_err(|error| ru_message(&error.to_string()))?;
            records.extend(global_variables::list_global_variables(&file, content)?);
        }
        records.sort_by(|left, right| {
            left.definition
                .name
                .cmp(&right.definition.name)
                .then_with(|| left.file.cmp(&right.file))
        });
        Ok(records)
    }

    fn global_variable_base_file(&self) -> Result<PathBuf, String> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())?;
        let files = workspace.files();
        yaml_imports::find_base_file(&files, workspace.match_root())
            .ok_or_else(|| "Не найден основной match/base.yml или match/base.yaml".to_owned())
    }

    fn sync_builtin_variables(&mut self, _id: &RuleId) -> Result<usize, String> {
        let definitions = dynamic_variables::builtin_definitions_in(&self.draft.replace);
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

    fn insert_global_placeholder(&mut self, name: &str) -> Result<String, String> {
        let id = self
            .selected
            .clone()
            .ok_or_else(|| "Сначала выберите правило для вставки переменной".to_owned())?;
        let placeholder = format!("{{{{{name}}}}}");
        self.draft.replace.push_str(&placeholder);
        let workspace = self
            .workspace
            .as_mut()
            .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())?;
        workspace
            .update_rule(&id, &self.draft)
            .map_err(|error| ru_message(&error.to_string()))?;
        self.raw_rule = workspace
            .rule(&id)
            .map(|rule| rule.raw)
            .map_err(|error| ru_message(&error.to_string()))?;
        Ok(placeholder)
    }

    fn apply_global_variable_action(&mut self, action: GlobalVariableEditorAction) {
        match action {
            GlobalVariableEditorAction::Save {
                file,
                original_name,
                definition,
                insert,
            } => {
                let result = (|| -> Result<(bool, Option<String>), String> {
                    if file.is_none() {
                        if let Some(existing) = self
                            .global_variable_records()?
                            .into_iter()
                            .find(|record| record.definition.name == definition.name)
                        {
                            return Err(format!(
                                "Переменная {} уже объявлена в {}. Выберите её в списке для изменения",
                                definition.placeholder(),
                                relative_display(&self.config_root, &existing.file)
                            ));
                        }
                    }
                    let target = match file {
                        Some(file) => file,
                        None => self.global_variable_base_file()?,
                    };
                    let added = {
                        let workspace = self
                            .workspace
                            .as_mut()
                            .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())?;
                        let content = workspace
                            .raw_file(&target)
                            .map_err(|error| ru_message(&error.to_string()))?
                            .to_owned();
                        let (updated, added) = global_variables::upsert_global_variable(
                            &content,
                            original_name.as_deref(),
                            &definition,
                        )?;
                        workspace
                            .set_raw_file(&target, updated)
                            .map_err(|error| ru_message(&error.to_string()))?;
                        added
                    };
                    let inserted = if insert {
                        Some(self.insert_global_placeholder(&definition.name)?)
                    } else {
                        None
                    };
                    Ok((added, inserted))
                })();

                self.status = match result {
                    Ok((added, inserted)) => {
                        let operation = if added { "добавлена" } else { "обновлена" };
                        if let Some(placeholder) = inserted {
                            format!(
                                "Глобальная переменная {placeholder} {operation} и вставлена в правило. Сохраните изменения Ctrl+S"
                            )
                        } else {
                            format!(
                                "Глобальная переменная {} {operation}. Сохраните изменения Ctrl+S",
                                definition.placeholder()
                            )
                        }
                    }
                    Err(error) => format!("Не удалось сохранить глобальную переменную: {error}"),
                };
            }
            GlobalVariableEditorAction::Delete { file, name } => {
                let result = (|| -> Result<bool, String> {
                    let workspace = self
                        .workspace
                        .as_mut()
                        .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())?;
                    let content = workspace
                        .raw_file(&file)
                        .map_err(|error| ru_message(&error.to_string()))?
                        .to_owned();
                    let (updated, removed) =
                        global_variables::remove_global_variable(&content, &name)?;
                    if removed {
                        workspace
                            .set_raw_file(&file, updated)
                            .map_err(|error| ru_message(&error.to_string()))?;
                    }
                    Ok(removed)
                })();
                self.status = match result {
                    Ok(true) => format!(
                        "Глобальная переменная {{{{{name}}}}} удалена. Использования в правилах оставлены для ручной проверки"
                    ),
                    Ok(false) => format!("Глобальная переменная {{{{{name}}}}} уже отсутствует"),
                    Err(error) => format!("Не удалось удалить глобальную переменную: {error}"),
                };
            }
            GlobalVariableEditorAction::Insert { name } => {
                self.status = match self
                    .global_variable_records()
                    .and_then(|records| {
                        if records
                            .iter()
                            .any(|record| record.definition.name == name)
                        {
                            self.insert_global_placeholder(&name)
                        } else {
                            Err(format!(
                                "Глобальная переменная {{{{{name}}}}} ещё не сохранена"
                            ))
                        }
                    }) {
                    Ok(placeholder) => format!(
                        "{placeholder} добавлена в текст правила. Сохраните изменения Ctrl+S"
                    ),
                    Err(error) => format!("Не удалось вставить переменную: {error}"),
                };
            }
        }
    }

    fn create_rule'''
app = replace_regex(
    app,
    r"    fn sync_builtin_variables\(&mut self, id: &RuleId\) -> Result<usize, String> \{.*?\n    fn create_rule",
    new_variable_methods,
    "global variable methods",
)

app = replace_once(
    app,
    "                            self.create_rule();\n                        }\n                        if ui\n                            .button(\"Сохранить всё\")",
    "                            self.create_rule();\n                        }\n                        if ui\n                            .button(\"Переменные\")\n                            .on_hover_text(\"Просмотр, создание и изменение общих global_vars\")\n                            .clicked()\n                        {\n                            self.show_global_variables = true;\n                        }\n                        if ui\n                            .button(\"Сохранить всё\")",
    "toolbar variables button",
)
app = replace_once(
    app,
    "                    \"Переменные: date, time, string, clipboard, echo, choice, form, random, rhai, script, shell\",\n                )\n                .clicked()\n            {\n                self.show_dynamic_variables = true;\n            }",
    "                    \"Открыть общие переменные: date, time, string, clipboard, echo, choice, form, random, rhai, script, shell\",\n                )\n                .clicked()\n            {\n                self.show_global_variables = true;\n            }",
    "replacement help",
)
app = replace_once(
    app,
    "                        self.status = format!(\n                            \"Изменения ожидают сохранения. Автоматически объявлено переменных: {count}\"\n                        );",
    "                        self.status = format!(\n                            \"Изменения ожидают сохранения. Автоматически объявлено общих переменных: {count}\"\n                        );",
    "sync status",
)
app = replace_regex(
    app,
    r"    fn add_dynamic_variable\(&mut self, action: DynamicVariableAction\) \{.*?\n    fn report_rhai_action",
    "    fn report_rhai_action",
    "remove local variable action",
)
app = replace_once(
    app,
    "        if self.show_dynamic_variables {\n            let mut open = self.show_dynamic_variables;\n            if let Some(action) = self.dynamic_variables.show(context, &mut open) {\n                self.add_dynamic_variable(action);\n            }\n            self.show_dynamic_variables = open;\n        }",
    "        if self.show_global_variables {\n            let records = match self.global_variable_records() {\n                Ok(records) => records,\n                Err(error) => {\n                    self.status = format!(\"Не удалось прочитать global_vars: {error}\");\n                    Vec::new()\n                }\n            };\n            let mut open = self.show_global_variables;\n            let action = self.global_variables.show(\n                context,\n                &mut open,\n                &records,\n                self.selected.is_some(),\n            );\n            self.show_global_variables = open;\n            if let Some(action) = action {\n                self.apply_global_variable_action(action);\n            }\n        }",
    "global variable dialog",
)

for forbidden in ["DynamicVariableAction", "DynamicVariableDialog", "show_dynamic_variables", "add_dynamic_variable"]:
    if forbidden in app:
        raise RuntimeError(f"old local variable symbol remains: {forbidden}")

APP.write_text(app, encoding="utf-8")


dynamic = DYNAMIC.read_text(encoding="utf-8")
dynamic = replace_once(
    dynamic,
    "Переменная добавляется в vars выбранного правила, а её шаблон — в конец текста подстановки.",
    "Переменная добавляется в общие global_vars, а её шаблон — в конец текста выбранного правила.",
    "dialog scope text",
)
dynamic = replace_once(
    dynamic,
    "fn canonical_definition(definition: &VariableDefinition)",
    "pub fn canonical_definition(definition: &VariableDefinition)",
    "canonical visibility",
)
dynamic = replace_once(
    dynamic,
    "fn validate_definition(definition: &VariableDefinition)",
    "pub fn validate_definition(definition: &VariableDefinition)",
    "validation visibility",
)
DYNAMIC.write_text(dynamic, encoding="utf-8")


global_text = GLOBAL.read_text(encoding="utf-8")
global_text = replace_once(
    global_text,
    "can_insert && !self.name.trim().is_empty(),",
    "can_insert && self.selected.is_some() && !self.name.trim().is_empty(),",
    "only insert saved variable",
)
GLOBAL.write_text(global_text, encoding="utf-8")

print("Updated app.rs, dynamic_variables.rs and global_variables.rs")

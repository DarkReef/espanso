use crate::dynamic_variables::{canonical_definition, validate_definition, VariableDefinition};
use eframe::egui;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalVariableRecord {
    pub file: PathBuf,
    pub definition: VariableDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobalVariableEditorAction {
    Save {
        file: Option<PathBuf>,
        original_name: Option<String>,
        definition: VariableDefinition,
        insert: bool,
    },
    Delete {
        file: PathBuf,
        name: String,
    },
    Insert {
        name: String,
    },
}

#[derive(Debug, Default)]
pub struct GlobalVariableEditor {
    selected: Option<(PathBuf, String)>,
    name: String,
    variable_type: String,
    params_yaml: String,
    error: Option<String>,
}

impl GlobalVariableEditor {
    pub fn show(
        &mut self,
        context: &egui::Context,
        open: &mut bool,
        records: &[GlobalVariableRecord],
        can_insert: bool,
    ) -> Option<GlobalVariableEditorAction> {
        let mut action = None;
        let mut close_requested = false;
        let mut preset = None;
        let mut select_record = None;

        egui::Window::new("Глобальные переменные")
            .open(open)
            .default_width(920.0)
            .default_height(620.0)
            .resizable(true)
            .show(context, |ui| {
                ui.label(
                    "Переменные из global_vars доступны всем включённым правилам. Новые переменные сохраняются в base.yml/base.yaml.",
                );
                ui.label(
                    egui::RichText::new(
                        "Локальные vars существующих правил не удаляются автоматически, но Studio больше не создаёт новые локальные переменные.",
                    )
                    .weak(),
                );
                ui.separator();

                ui.horizontal_wrapped(|ui| {
                    ui.label("Готовые переменные:");
                    if ui.button("{{date}}").clicked() {
                        preset = Some(preset_definition(
                            "date",
                            "date",
                            "format: \"%d.%m.%Y\"",
                        ));
                    }
                    if ui.button("{{time}}").clicked() {
                        preset = Some(preset_definition(
                            "time",
                            "date",
                            "format: \"%H:%M\"",
                        ));
                    }
                    if ui.button("{{weekday}}").clicked() {
                        preset = Some(preset_definition(
                            "weekday",
                            "date",
                            "format: \"%A\"\nlocale: \"ru-RU\"",
                        ));
                    }
                    if ui.button("{{clipboard}}").clicked() {
                        preset = Some(preset_definition("clipboard", "clipboard", ""));
                    }
                    if ui.button("Новая").clicked() {
                        self.selected = None;
                        "my_variable".clone_into(&mut self.name);
                        "string".clone_into(&mut self.variable_type);
                        self.params_yaml.clear();
                        self.error = None;
                    }
                });
                ui.separator();

                ui.columns(2, |columns| {
                    columns[0].heading(format!("Общие переменные ({})", records.len()));
                    egui::ScrollArea::vertical()
                        .id_salt("global_variable_list")
                        .max_height(470.0)
                        .show(&mut columns[0], |ui| {
                            for record in records {
                                let selected = self.selected.as_ref().is_some_and(|(file, name)| {
                                    file == &record.file && name == &record.definition.name
                                });
                                let label = format!(
                                    "{{{{{}}}}}  ·  {}\n{}",
                                    record.definition.name,
                                    display_type(&record.definition),
                                    record.file.display()
                                );
                                if ui.selectable_label(selected, label).clicked() {
                                    select_record = Some(record.clone());
                                }
                                ui.separator();
                            }
                            if records.is_empty() {
                                ui.label("Глобальные переменные пока не объявлены");
                            }
                        });

                    columns[1].heading(if self.selected.is_some() {
                        "Изменение переменной"
                    } else {
                        "Новая переменная"
                    });
                    if let Some((file, _)) = &self.selected {
                        columns[1].label(
                            egui::RichText::new(format!("Файл: {}", file.display())).weak(),
                        );
                    } else {
                        columns[1].label(
                            egui::RichText::new("Будет добавлена в основной base.yml/base.yaml")
                                .weak(),
                        );
                    }

                    egui::Grid::new("global_variable_editor_grid")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(&mut columns[1], |ui| {
                            ui.label("Имя");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.name)
                                    .desired_width(f32::INFINITY)
                                    .hint_text("Латиницей: doctor_name"),
                            );
                            ui.end_row();

                            ui.label("Тип");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.variable_type)
                                    .desired_width(f32::INFINITY)
                                    .hint_text("date, string, clipboard, echo, choice…"),
                            );
                            ui.end_row();
                        });
                    columns[1].label(
                        egui::RichText::new(
                            "Тип string сохраняется в Espanso как echo. Тип time сохраняется как date с форматом времени.",
                        )
                        .weak(),
                    );

                    let string_type = self.variable_type.trim().eq_ignore_ascii_case("string");
                    columns[1].label(if string_type {
                        "Значение строки"
                    } else {
                        "Параметры YAML без строки params:"
                    });
                    columns[1].add(
                        egui::TextEdit::multiline(&mut self.params_yaml)
                            .code_editor()
                            .desired_rows(10)
                            .desired_width(f32::INFINITY)
                            .hint_text(if string_type {
                                "Например: Куцин Иван Юрьевич"
                            } else {
                                "Например: format: \"%d.%m.%Y\""
                            }),
                    );
                    columns[1].monospace(format!("Шаблон: {{{{{}}}}}", self.name.trim()));

                    if let Some(error) = &self.error {
                        columns[1].colored_label(columns[1].visuals().error_fg_color, error);
                    }

                    columns[1].horizontal_wrapped(|ui| {
                        if ui.button("Сохранить глобально").clicked() {
                            action = self.save_action(false);
                        }
                        if ui
                            .add_enabled(can_insert, egui::Button::new("Сохранить и вставить"))
                            .on_hover_text("Сохранить global_vars и добавить {{имя}} в выбранное правило")
                            .clicked()
                        {
                            action = self.save_action(true);
                            if action.is_some() {
                                close_requested = true;
                            }
                        }
                        if ui
                            .add_enabled(
                                can_insert && self.selected.is_some() && !self.name.trim().is_empty(),
                                egui::Button::new("Только вставить"),
                            )
                            .clicked()
                        {
                            action = Some(GlobalVariableEditorAction::Insert {
                                name: self.name.trim().to_owned(),
                            });
                            close_requested = true;
                        }
                    });
                    columns[1].horizontal(|ui| {
                        if let Some((file, original_name)) = &self.selected {
                            if ui
                                .button("Удалить глобальную переменную")
                                .on_hover_text("Удаляет объявление, но не меняет тексты правил")
                                .clicked()
                            {
                                action = Some(GlobalVariableEditorAction::Delete {
                                    file: file.clone(),
                                    name: original_name.clone(),
                                });
                                close_requested = true;
                            }
                        }
                        if ui.button("Закрыть").clicked() {
                            close_requested = true;
                        }
                    });
                });
            });

        if let Some(record) = select_record {
            self.load_record(&record);
        }
        if let Some(definition) = preset {
            if let Some(record) = records
                .iter()
                .find(|record| record.definition.name == definition.name)
            {
                self.load_record(record);
            } else {
                self.selected = None;
                self.name = definition.name;
                self.variable_type = definition.variable_type;
                self.params_yaml = definition.params_yaml;
                self.error = None;
            }
        }
        if close_requested {
            *open = false;
        }
        action
    }

    fn load_record(&mut self, record: &GlobalVariableRecord) {
        self.selected = Some((record.file.clone(), record.definition.name.clone()));
        self.name.clone_from(&record.definition.name);
        self.variable_type
            .clone_from(&record.definition.variable_type);
        self.params_yaml.clone_from(&record.definition.params_yaml);
        self.error = None;
    }

    fn save_action(&mut self, insert: bool) -> Option<GlobalVariableEditorAction> {
        let definition = VariableDefinition {
            name: self.name.trim().to_owned(),
            variable_type: self.variable_type.trim().to_owned(),
            params_yaml: self.params_yaml.trim().to_owned(),
        };
        match canonical_definition(&definition).and_then(|definition| {
            validate_definition(&definition)?;
            Ok(definition)
        }) {
            Ok(definition) => {
                self.error = None;
                Some(GlobalVariableEditorAction::Save {
                    file: self.selected.as_ref().map(|(file, _)| file.clone()),
                    original_name: self.selected.as_ref().map(|(_, name)| name.clone()),
                    definition,
                    insert,
                })
            }
            Err(error) => {
                self.error = Some(error);
                None
            }
        }
    }
}

pub fn list_global_variables(
    file: &Path,
    content: &str,
) -> Result<Vec<GlobalVariableRecord>, String> {
    let holder = parse_holder(content)?;
    holder
        .global_vars
        .into_iter()
        .map(|variable| {
            Ok(GlobalVariableRecord {
                file: file.to_path_buf(),
                definition: stored_to_definition(variable)?,
            })
        })
        .collect()
}

pub fn ensure_global_variable(
    content: &str,
    definition: &VariableDefinition,
) -> Result<(String, bool), String> {
    let definition = canonical_definition(definition)?;
    validate_definition(&definition)?;
    let holder = parse_holder(content)?;
    if holder
        .global_vars
        .iter()
        .any(|variable| variable.name == definition.name)
    {
        return Ok((content.to_owned(), false));
    }
    upsert_global_variable(content, None, &definition)
}

pub fn upsert_global_variable(
    content: &str,
    original_name: Option<&str>,
    definition: &VariableDefinition,
) -> Result<(String, bool), String> {
    let definition = canonical_definition(definition)?;
    validate_definition(&definition)?;
    let mut holder = parse_holder(content)?;
    ensure_rewrite_safe(content)?;
    let existing_index = original_name
        .and_then(|name| {
            holder
                .global_vars
                .iter()
                .position(|variable| variable.name == name)
        })
        .or_else(|| {
            holder
                .global_vars
                .iter()
                .position(|variable| variable.name == definition.name)
        });

    if holder
        .global_vars
        .iter()
        .enumerate()
        .any(|(index, variable)| variable.name == definition.name && Some(index) != existing_index)
    {
        return Err(format!(
            "Глобальная переменная {} уже объявлена в этом файле",
            definition.placeholder()
        ));
    }

    let params = parse_params(&definition.params_yaml)?;
    let added = existing_index.is_none();
    let mut stored = StoredVariable {
        name: definition.name,
        variable_type: definition.variable_type,
        params,
        extra: BTreeMap::new(),
    };
    if let Some(index) = existing_index {
        stored.extra = holder.global_vars[index].extra.clone();
        holder.global_vars[index] = stored;
    } else {
        holder.global_vars.push(stored);
    }

    let updated = replace_global_vars_section(content, &holder.global_vars)?;
    validate_document(&updated)?;
    Ok((updated, added))
}

pub fn remove_global_variable(content: &str, name: &str) -> Result<(String, bool), String> {
    let mut holder = parse_holder(content)?;
    ensure_rewrite_safe(content)?;
    let before = holder.global_vars.len();
    holder.global_vars.retain(|variable| variable.name != name);
    if holder.global_vars.len() == before {
        return Ok((content.to_owned(), false));
    }
    let updated = replace_global_vars_section(content, &holder.global_vars)?;
    validate_document(&updated)?;
    Ok((updated, true))
}

#[derive(Debug, Default, Deserialize)]
struct GlobalVarsHolder {
    #[serde(default, deserialize_with = "deserialize_global_vars")]
    global_vars: Vec<StoredVariable>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredVariable {
    name: String,
    #[serde(rename = "type")]
    variable_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StoredVariables {
    Sequence(Vec<StoredVariable>),
    Single(StoredVariable),
}

fn deserialize_global_vars<'de, D>(deserializer: D) -> Result<Vec<StoredVariable>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(
        match Option::<StoredVariables>::deserialize(deserializer)? {
            Some(StoredVariables::Sequence(variables)) => variables,
            Some(StoredVariables::Single(variable)) => vec![variable],
            None => Vec::new(),
        },
    )
}

fn parse_holder(content: &str) -> Result<GlobalVarsHolder, String> {
    serde_norway::from_str(content)
        .map_err(|error| format!("Не удалось прочитать global_vars: {error}"))
}

fn parse_params(params_yaml: &str) -> Result<Option<Value>, String> {
    let trimmed = params_yaml.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = serde_norway::from_str::<Value>(trimmed)
        .map_err(|error| format!("Ошибка параметров переменной: {error}"))?;
    if !value.is_object() {
        return Err(
            "Параметры переменной должны быть YAML-объектом вида ключ: значение".to_owned(),
        );
    }
    Ok(Some(value))
}

fn stored_to_definition(variable: StoredVariable) -> Result<VariableDefinition, String> {
    if variable.variable_type.eq_ignore_ascii_case("echo") {
        if let Some(Value::Object(params)) = &variable.params {
            if params.len() == 1 {
                if let Some(Value::String(value)) = params.get("echo") {
                    return Ok(VariableDefinition {
                        name: variable.name,
                        variable_type: "string".to_owned(),
                        params_yaml: value.clone(),
                    });
                }
            }
        }
    }

    let params_yaml = variable.params.map_or_else(
        || Ok(String::new()),
        |params| {
            serde_norway::to_string(&params)
                .map(|value| value.trim().to_owned())
                .map_err(|error| format!("Не удалось показать параметры переменной: {error}"))
        },
    )?;
    Ok(VariableDefinition {
        name: variable.name,
        variable_type: variable.variable_type,
        params_yaml,
    })
}

fn ensure_rewrite_safe(content: &str) -> Result<(), String> {
    let Some((start, end)) = top_level_section(content, "global_vars") else {
        return Ok(());
    };
    let section = &content[start..end];
    let unsafe_line = section.lines().skip(1).find(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with('#')
            || trimmed.starts_with('*')
            || line.contains(": &")
            || line.contains(": *")
    });
    if let Some(line) = unsafe_line {
        return Err(format!(
            "Секция global_vars содержит комментарий или YAML-якорь ('{}'). Чтобы не потерять оформление, измените её в исходном YAML",
            line.trim()
        ));
    }
    Ok(())
}

fn replace_global_vars_section(
    content: &str,
    variables: &[StoredVariable],
) -> Result<String, String> {
    let newline = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let rendered = render_global_vars(variables, newline)?;
    let section = top_level_section(content, "global_vars");

    Ok(match section {
        Some((start, end)) => {
            let mut result = String::with_capacity(content.len() + rendered.len());
            result.push_str(&content[..start]);
            result.push_str(&rendered);
            result.push_str(&content[end..]);
            result
        }
        None if variables.is_empty() => content.to_owned(),
        None => {
            let insertion = top_level_section(content, "matches")
                .or_else(|| top_level_section(content, "imports"))
                .map_or(content.len(), |(start, _)| start);
            let mut result = String::with_capacity(content.len() + rendered.len() + newline.len());
            result.push_str(&content[..insertion]);
            if insertion > 0
                && !content[..insertion].ends_with('\n')
                && !content[..insertion].ends_with('\r')
            {
                result.push_str(newline);
            }
            result.push_str(&rendered);
            result.push_str(&content[insertion..]);
            result
        }
    })
}

fn render_global_vars(variables: &[StoredVariable], newline: &str) -> Result<String, String> {
    if variables.is_empty() {
        return Ok(String::new());
    }
    let mut result = format!("global_vars:{newline}");
    for variable in variables {
        let rendered = serde_norway::to_string(variable)
            .map_err(|error| format!("Не удалось сохранить глобальную переменную: {error}"))?;
        for (index, line) in rendered.lines().enumerate() {
            result.push_str(if index == 0 { "  - " } else { "    " });
            result.push_str(line);
            result.push_str(newline);
        }
    }
    result.push_str(newline);
    Ok(result)
}

fn validate_document(content: &str) -> Result<(), String> {
    serde_norway::from_str::<Value>(content)
        .map(|_| ())
        .map_err(|error| format!("После изменения global_vars YAML стал некорректным: {error}"))
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
            if next_trimmed.is_empty() {
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

fn preset_definition(name: &str, variable_type: &str, params_yaml: &str) -> VariableDefinition {
    VariableDefinition {
        name: name.to_owned(),
        variable_type: variable_type.to_owned(),
        params_yaml: params_yaml.to_owned(),
    }
}

fn display_type(definition: &VariableDefinition) -> &str {
    if definition.variable_type == "string" {
        "строка"
    } else {
        &definition.variable_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_global_variable_before_matches() {
        let source = "matches:\n  - trigger: :a\n    replace: A\n";
        let definition = VariableDefinition {
            name: "date".to_owned(),
            variable_type: "date".to_owned(),
            params_yaml: "format: \"%d.%m.%Y\"".to_owned(),
        };
        let (updated, added) = upsert_global_variable(source, None, &definition).unwrap();
        assert!(added);
        assert!(updated.starts_with("global_vars:\n"));
        assert!(updated.contains("name: date"));
        assert!(updated.contains("matches:"));
    }

    #[test]
    fn edits_string_without_losing_extra_fields() {
        let source = "global_vars:\n  - name: doc\n    type: echo\n    inject_vars: false\n    params:\n      echo: Old\n\nmatches: []\n";
        let definition = VariableDefinition {
            name: "doc".to_owned(),
            variable_type: "string".to_owned(),
            params_yaml: "New".to_owned(),
        };
        let (updated, added) = upsert_global_variable(source, Some("doc"), &definition).unwrap();
        assert!(!added);
        assert!(updated.contains("inject_vars: false"));
        assert!(updated.contains("echo: New"));
    }

    #[test]
    fn refuses_structural_rewrite_when_comments_would_be_lost() {
        let source = "global_vars:\n  # keep me\n  - name: doc\n    type: echo\n    params:\n      echo: Old\n\nmatches: []\n";
        let definition = VariableDefinition {
            name: "doc".to_owned(),
            variable_type: "string".to_owned(),
            params_yaml: "New".to_owned(),
        };
        assert!(upsert_global_variable(source, Some("doc"), &definition)
            .expect_err("commented section must require raw edit")
            .contains("исходном YAML"));
    }

    #[test]
    fn preserves_top_level_comment_after_global_vars() {
        let source = "global_vars:\n  - name: doc\n    type: echo\n    params:\n      echo: Old\n\n# section comment\nmatches: []\n";
        let definition = VariableDefinition {
            name: "doc".to_owned(),
            variable_type: "string".to_owned(),
            params_yaml: "New".to_owned(),
        };
        let (updated, _) =
            upsert_global_variable(source, Some("doc"), &definition).expect("update");
        assert!(updated.contains("# section comment"));
    }

    #[test]
    fn lists_echo_scalar_as_string() {
        let source = "global_vars:\n  - name: doc\n    type: echo\n    params:\n      echo: Иван\n\nmatches: []\n";
        let records = list_global_variables(Path::new("match/base.yml"), source).unwrap();
        assert_eq!(records[0].definition.variable_type, "string");
        assert_eq!(records[0].definition.params_yaml, "Иван");
    }

    #[test]
    fn renders_new_string_as_sequence_and_adds_it_to_selection_pool() {
        let source = "matches: []\n";
        let definition = VariableDefinition {
            name: "doc".to_owned(),
            variable_type: "string".to_owned(),
            params_yaml: "Куцин Иван Юрьевич".to_owned(),
        };

        let (updated, added) =
            upsert_global_variable(source, None, &definition).expect("add string variable");
        assert!(added);
        assert!(updated.contains("global_vars:\n  - name: doc\n"));

        let records = list_global_variables(Path::new("match/base.yml"), &updated)
            .expect("new variable should enter the selection pool");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].definition.name, "doc");
        assert_eq!(records[0].definition.variable_type, "string");
        assert_eq!(records[0].definition.params_yaml, "Куцин Иван Юрьевич");
    }

    #[test]
    fn accepts_legacy_single_map_and_rewrites_it_as_sequence() {
        let source = "global_vars:\n  name: doc\n  type: echo\n  params:\n    echo: Куцин Иван Юрьевич\n\nmatches: []\n";
        let records = list_global_variables(Path::new("match/base.yml"), source)
            .expect("legacy map should remain editable and visible");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].definition.name, "doc");

        let (updated, added) = upsert_global_variable(source, Some("doc"), &records[0].definition)
            .expect("normalize legacy map");
        assert!(!added);
        assert!(updated.contains("global_vars:\n  - name: doc\n"));
        assert!(!updated.contains("global_vars:\n  name: doc\n"));
    }
}

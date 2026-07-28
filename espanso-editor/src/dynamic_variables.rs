use eframe::egui;
use serde::Deserialize;
use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableDefinition {
    pub name: String,
    pub variable_type: String,
    pub params_yaml: String,
}

impl VariableDefinition {
    #[must_use]
    pub fn placeholder(&self) -> String {
        format!("{{{{{}}}}}", self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicVariableAction {
    pub definition: VariableDefinition,
    pub placeholder: String,
    pub message: String,
}

pub struct DynamicVariableDialog {
    name: String,
    variable_type: String,
    params_yaml: String,
    error: Option<String>,
}

impl Default for DynamicVariableDialog {
    fn default() -> Self {
        Self {
            name: "doc".to_owned(),
            variable_type: "string".to_owned(),
            params_yaml: "Куцин Иван Юрьевич".to_owned(),
            error: None,
        }
    }
}

impl DynamicVariableDialog {
    pub fn show(
        &mut self,
        context: &egui::Context,
        open: &mut bool,
    ) -> Option<DynamicVariableAction> {
        let mut action = None;
        let mut close_requested = false;
        egui::Window::new("Динамические переменные")
            .open(open)
            .default_width(620.0)
            .resizable(true)
            .show(context, |ui| {
                ui.label(
                    "Переменная добавляется в vars выбранного правила, а её шаблон — в конец текста подстановки.",
                );
                ui.add_space(6.0);
                ui.heading("Готовые переменные");
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button("{{date}}")
                        .on_hover_text("Текущая дата: 28.07.2026")
                        .clicked()
                    {
                        action = Some(preset_action(
                            "date",
                            "date",
                            "format: \"%d.%m.%Y\"",
                            "Добавлена переменная текущей даты {{date}}",
                        ));
                        close_requested = true;
                    }
                    if ui
                        .button("{{time}}")
                        .on_hover_text("Текущее время: 16:45")
                        .clicked()
                    {
                        action = Some(preset_action(
                            "time",
                            "date",
                            "format: \"%H:%M\"",
                            "Добавлена переменная текущего времени {{time}}",
                        ));
                        close_requested = true;
                    }
                    if ui
                        .button("{{weekday}}")
                        .on_hover_text("Название дня недели")
                        .clicked()
                    {
                        action = Some(preset_action(
                            "weekday",
                            "date",
                            "format: \"%A\"\nlocale: \"ru-RU\"",
                            "Добавлена переменная дня недели {{weekday}}",
                        ));
                        close_requested = true;
                    }
                    if ui
                        .button("{{clipboard}}")
                        .on_hover_text("Текущее содержимое буфера обмена")
                        .clicked()
                    {
                        action = Some(preset_action(
                            "clipboard",
                            "clipboard",
                            "",
                            "Добавлена переменная буфера обмена {{clipboard}}",
                        ));
                        close_requested = true;
                    }
                });
                ui.label(
                    egui::RichText::new(
                        "Для RegExp-группы (?P<patient>...) используйте {{patient}} — отдельный блок vars не нужен.",
                    )
                    .weak(),
                );

                ui.separator();
                ui.heading("Своя переменная");
                egui::Grid::new("custom_dynamic_variable_grid")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Имя");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.name)
                                .desired_width(360.0)
                                .hint_text("Латиницей: patient_date"),
                        );
                        ui.end_row();

                        ui.label("Тип");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.variable_type)
                                .desired_width(360.0)
                                .hint_text("date, time, string, clipboard, echo, choice, form, random, rhai, script, shell"),
                        );
                        ui.end_row();
                    });
                ui.label(
                    egui::RichText::new(
                        "Типы: date, time, string, clipboard, echo, choice, form, random, rhai, script, shell",
                    )
                    .weak(),
                );
                let string_type = self.variable_type.trim().eq_ignore_ascii_case("string");
                ui.label(if string_type {
                    "Значение строки"
                } else {
                    "Параметры YAML без строки params: (можно оставить пустыми)"
                });
                ui.add(
                    egui::TextEdit::multiline(&mut self.params_yaml)
                        .code_editor()
                        .desired_rows(5)
                        .desired_width(f32::INFINITY)
                        .hint_text(if string_type {
                            "Например: Куцин Иван Юрьевич"
                        } else {
                            "Например: format: \"%d.%m.%Y\""
                        }),
                );
                ui.monospace(format!("Шаблон: {{{{{}}}}}", self.name.trim()));

                if let Some(error) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }

                ui.horizontal(|ui| {
                    if ui.button("Добавить переменную").clicked() {
                        let definition = VariableDefinition {
                            name: self.name.trim().to_owned(),
                            variable_type: self.variable_type.trim().to_owned(),
                            params_yaml: self.params_yaml.trim().to_owned(),
                        };
                        match validate_definition(&definition) {
                            Ok(()) => {
                                let placeholder = definition.placeholder();
                                action = Some(DynamicVariableAction {
                                    message: format!(
                                        "Добавлена пользовательская переменная {placeholder}"
                                    ),
                                    placeholder,
                                    definition,
                                });
                                self.error = None;
                                close_requested = true;
                            }
                            Err(error) => self.error = Some(error),
                        }
                    }
                    if ui.button("Закрыть").clicked() {
                        close_requested = true;
                    }
                });
            });
        if close_requested {
            *open = false;
        }
        action
    }
}

fn preset_action(
    name: &str,
    variable_type: &str,
    params_yaml: &str,
    message: &str,
) -> DynamicVariableAction {
    let definition = VariableDefinition {
        name: name.to_owned(),
        variable_type: variable_type.to_owned(),
        params_yaml: params_yaml.to_owned(),
    };
    DynamicVariableAction {
        placeholder: definition.placeholder(),
        definition,
        message: message.to_owned(),
    }
}

pub fn builtin_definitions_in(replacement: &str) -> Vec<VariableDefinition> {
    let mut definitions = Vec::new();
    if replacement.contains("{{date}}") {
        definitions.push(VariableDefinition {
            name: "date".to_owned(),
            variable_type: "date".to_owned(),
            params_yaml: "format: \"%d.%m.%Y\"".to_owned(),
        });
    }
    if replacement.contains("{{time}}") {
        definitions.push(VariableDefinition {
            name: "time".to_owned(),
            variable_type: "time".to_owned(),
            params_yaml: String::new(),
        });
    }
    if replacement.contains("{{weekday}}") {
        definitions.push(VariableDefinition {
            name: "weekday".to_owned(),
            variable_type: "date".to_owned(),
            params_yaml: "format: \"%A\"\nlocale: \"ru-RU\"".to_owned(),
        });
    }
    if replacement.contains("{{clipboard}}") {
        definitions.push(VariableDefinition {
            name: "clipboard".to_owned(),
            variable_type: "clipboard".to_owned(),
            params_yaml: String::new(),
        });
    }
    definitions
}
pub fn upsert_rule_variable(
    raw_rule: &str,
    definition: &VariableDefinition,
) -> Result<(String, bool), String> {
    let definition = canonical_definition(definition)?;
    validate_definition(&definition)?;
    let params = normalized_params(&definition.params_yaml)?;
    let rule_indent = raw_rule
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(leading_spaces)
        .min()
        .unwrap_or(0);
    let child_indent = rule_indent + 2;
    let mut offset = 0_usize;
    let lines = raw_rule
        .split_inclusive('\n')
        .map(|line| {
            let start = offset;
            offset += line.len();
            (start, offset, line)
        })
        .collect::<Vec<_>>();

    let vars_line = lines
        .iter()
        .enumerate()
        .find_map(|(index, (start, end, line))| {
            let trimmed = line.trim_end_matches(['\r', '\n']).trim_start();
            if leading_spaces(line) == child_indent && trimmed.strip_prefix("vars:").is_some() {
                Some((index, *start, *end, trimmed.to_owned()))
            } else {
                None
            }
        });
    let item = render_variable(&definition, params.as_deref(), child_indent + 2);

    let updated = if let Some((line_index, vars_start, vars_line_end, vars_text)) = vars_line {
        let suffix = vars_text.strip_prefix("vars:").unwrap_or_default().trim();
        if !suffix.is_empty() && suffix != "[]" {
            return Err(
                "В правиле используется компактная запись vars. Откройте «Исходный YAML» и разверните vars в список"
                    .to_owned(),
            );
        }

        let mut block_end = raw_rule.len();
        for (start, _, line) in lines.iter().skip(line_index + 1) {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if leading_spaces(line) <= child_indent {
                block_end = *start;
                break;
            }
        }

        if suffix.is_empty() {
            let block = &raw_rule[vars_start..block_end];
            let normalized = deindent(block, child_indent);
            let holder: VarsHolder = serde_norway::from_str(&normalized)
                .map_err(|error| format!("Не удалось разобрать существующий vars: {error}"))?;
            if holder
                .vars
                .iter()
                .any(|variable| variable.name == definition.name)
            {
                return Ok((raw_rule.to_owned(), false));
            }
            let mut result = raw_rule.to_owned();
            result.insert_str(block_end, &item);
            result
        } else {
            let mut result = String::with_capacity(raw_rule.len() + item.len());
            result.push_str(&raw_rule[..vars_start]);
            let _ = writeln!(result, "{}vars:", " ".repeat(child_indent));
            result.push_str(&item);
            result.push_str(&raw_rule[vars_line_end..]);
            result
        }
    } else {
        let mut result = raw_rule.to_owned();
        if !result.ends_with('\n') {
            result.push('\n');
        }
        let _ = writeln!(result, "{}vars:", " ".repeat(child_indent));
        result.push_str(&item);
        result
    };

    validate_rule_yaml(&updated)?;
    Ok((updated, true))
}

fn canonical_definition(definition: &VariableDefinition) -> Result<VariableDefinition, String> {
    let variable_type = definition.variable_type.trim().to_ascii_lowercase();
    let mut canonical = definition.clone();
    canonical.name = canonical.name.trim().to_owned();
    canonical.variable_type.clone_from(&variable_type);
    canonical.params_yaml = canonical.params_yaml.trim().to_owned();

    match variable_type.as_str() {
        "time" => {
            "date".clone_into(&mut canonical.variable_type);
            if canonical.params_yaml.is_empty() {
                "format: \"%H:%M\"".clone_into(&mut canonical.params_yaml);
            }
        }
        "string" => {
            if canonical.params_yaml.is_empty() {
                return Err("Для типа string укажите значение строки".to_owned());
            }
            "echo".clone_into(&mut canonical.variable_type);
            canonical.params_yaml = format!(
                "echo: {}",
                serde_json::to_string(&canonical.params_yaml)
                    .map_err(|error| format!("Не удалось подготовить строку: {error}"))?
            );
        }
        _ => {}
    }
    Ok(canonical)
}
fn validate_definition(definition: &VariableDefinition) -> Result<(), String> {
    let mut characters = definition.name.chars();
    let Some(first) = characters.next() else {
        return Err("Укажите имя переменной".to_owned());
    };
    if !(first.is_ascii_alphabetic() || first == '_')
        || !characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(
            "Имя должно начинаться с латинской буквы или _, далее допустимы буквы, цифры и _"
                .to_owned(),
        );
    }
    if definition.variable_type.is_empty()
        || !definition.variable_type.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
    {
        return Err("Укажите корректный тип переменной".to_owned());
    }
    let _ = normalized_params(&definition.params_yaml)?;
    Ok(())
}

fn normalized_params(params_yaml: &str) -> Result<Option<String>, String> {
    if params_yaml.trim().is_empty() {
        return Ok(None);
    }
    let value: serde_json::Value = serde_norway::from_str(params_yaml)
        .map_err(|error| format!("Ошибка YAML в параметрах: {error}"))?;
    if !value.is_object() {
        return Err("Параметры должны быть YAML-объектом вида key: value".to_owned());
    }
    let rendered = serde_norway::to_string(&value)
        .map_err(|error| format!("Не удалось подготовить параметры: {error}"))?;
    Ok(Some(rendered.trim().to_owned()))
}

fn render_variable(
    definition: &VariableDefinition,
    params: Option<&str>,
    item_indent: usize,
) -> String {
    let field_indent = item_indent + 2;
    let params_indent = field_indent + 2;
    let name = serde_json::to_string(&definition.name).unwrap_or_else(|_| "\"\"".to_owned());
    let variable_type =
        serde_json::to_string(&definition.variable_type).unwrap_or_else(|_| "\"\"".to_owned());
    let mut result = format!(
        "{}- name: {name}\n{}type: {variable_type}\n",
        " ".repeat(item_indent),
        " ".repeat(field_indent)
    );
    if let Some(params) = params {
        let _ = writeln!(result, "{}params:", " ".repeat(field_indent));
        for line in params.lines() {
            let _ = writeln!(result, "{}{}", " ".repeat(params_indent), line);
        }
    }
    result
}

fn validate_rule_yaml(raw_rule: &str) -> Result<(), String> {
    let wrapper = format!("matches:\n{}", reindent(raw_rule, 2));
    serde_norway::from_str::<serde_json::Value>(&wrapper)
        .map(|_| ())
        .map_err(|error| format!("Получившийся YAML правила некорректен: {error}"))
}

fn leading_spaces(line: &str) -> usize {
    line.chars()
        .take_while(|character| *character == ' ')
        .count()
}

fn deindent(text: &str, indent: usize) -> String {
    let mut result = String::new();
    for line in text.lines() {
        result.push_str(line.get(indent..).unwrap_or(line));
        result.push('\n');
    }
    result
}

fn reindent(text: &str, target_indent: usize) -> String {
    let minimum = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(leading_spaces)
        .min()
        .unwrap_or(0);
    let prefix = " ".repeat(target_indent);
    let mut result = String::new();
    for line in text.lines() {
        result.push_str(&prefix);
        result.push_str(line.get(minimum..).unwrap_or(line));
        result.push('\n');
    }
    result
}

#[derive(Debug, Deserialize)]
struct VarsHolder {
    #[serde(default)]
    vars: Vec<VariableName>,
}

#[derive(Debug, Deserialize)]
struct VariableName {
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_variable_to_rule_without_vars() {
        let definition = VariableDefinition {
            name: "date".to_owned(),
            variable_type: "date".to_owned(),
            params_yaml: "format: \"%d.%m.%Y\"".to_owned(),
        };
        let raw = "  - trigger: \":x\"\n    replace: \"X\"\n";
        let (updated, added) = upsert_rule_variable(raw, &definition).unwrap();
        assert!(added);
        assert!(updated.contains("    vars:\n      - name: \"date\""));
        assert!(updated.contains("format:"));
        assert!(updated.contains("%d.%m.%Y"));
    }

    #[test]
    fn does_not_duplicate_existing_variable() {
        let definition = VariableDefinition {
            name: "date".to_owned(),
            variable_type: "date".to_owned(),
            params_yaml: String::new(),
        };
        let raw = "  - trigger: \":x\"\n    replace: \"{{date}}\"\n    vars:\n      - name: date\n        type: date\n";
        let (updated, added) = upsert_rule_variable(raw, &definition).unwrap();
        assert!(!added);
        assert_eq!(updated, raw);
    }

    #[test]
    fn expands_empty_inline_vars() {
        let definition = VariableDefinition {
            name: "clipboard".to_owned(),
            variable_type: "clipboard".to_owned(),
            params_yaml: String::new(),
        };
        let raw = "  - trigger: \":x\"\n    replace: \"X\"\n    vars: []\n";
        let (updated, added) = upsert_rule_variable(raw, &definition).unwrap();
        assert!(added);
        assert!(updated.contains("    vars:\n      - name: \"clipboard\""));
    }

    #[test]
    fn rejects_invalid_variable_name() {
        let definition = VariableDefinition {
            name: "1date".to_owned(),
            variable_type: "date".to_owned(),
            params_yaml: String::new(),
        };
        assert!(
            upsert_rule_variable("  - trigger: \":x\"\n    replace: \"X\"\n", &definition).is_err()
        );
    }
    #[test]
    fn time_alias_uses_date_extension() {
        let definition = VariableDefinition {
            name: "time".to_owned(),
            variable_type: "time".to_owned(),
            params_yaml: String::new(),
        };
        let (updated, added) = upsert_rule_variable(
            "  - trigger: \":x\"\n    replace: \"{{time}}\"\n",
            &definition,
        )
        .unwrap();
        assert!(added);
        assert!(updated.contains("type: \"date\""));
        assert!(updated.contains("%H:%M"));
    }

    #[test]
    fn string_alias_uses_echo_extension() {
        let definition = VariableDefinition {
            name: "doc".to_owned(),
            variable_type: "string".to_owned(),
            params_yaml: "Куцин Иван Юрьевич".to_owned(),
        };
        let (updated, added) = upsert_rule_variable(
            "  - trigger: \":doc\"\n    replace: \"{{doc}}\"\n",
            &definition,
        )
        .unwrap();
        assert!(added);
        assert!(updated.contains("type: \"echo\""));
        assert!(updated.contains("Куцин Иван Юрьевич"));
    }

    #[test]
    fn detects_manually_typed_builtin_placeholders() {
        let definitions = builtin_definitions_in("{{date}} {{time}} {{clipboard}}");
        let names = definitions
            .iter()
            .map(|definition| definition.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["date", "time", "clipboard"]);
    }
}

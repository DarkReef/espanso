anel = rust(r'''
egui::Panel::bottom("status").show(ui, |ui| {
    ui.horizontal(|ui| {
        ui.label(self.status.as_str());
        ui.separator();
        ui.label(
            egui::RichText::new(format!("Конфигурация: {}", self.config_root.display()))
                .weak(),
        );
        if self.external_change_pending {
            ui.separator();
            if ui.button("Обновить внешние изменения").clicked() {
                self.request_external_reload();
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.hyperlink_to(
                "imaganate.dark@gmail.com",
                "mailto:imaganate.dark@gmail.com",
            )
            .on_hover_text("Автор форка: Куцин Иван Юрьевич");
        });
    });
});
''', 8)
app = replace_between(
    app,
    '        egui::Panel::bottom("status").show(ui, |ui| {',
    "        match self.active_tab {",
    status_panel + "\n",
    "status author link",
)

normalize_helper = rust(r'''
fn normalize_yaml_file_name(value: &str) -> Result<String, String> {
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
    let reserved = stem.to_ascii_uppercase();
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

''')
app = insert_before(
    app,
    "fn relative_display(root: &Path, path: &Path) -> String {\n",
    normalize_helper,
    "yaml filename helper",
)
app_path.write_text(app, encoding="utf-8")


dynamic_path = Path("espanso-editor/src/dynamic_variables.rs")
dynamic = dynamic_path.read_text(encoding="utf-8")
dynamic = replace_once(dynamic, 'name: "my_date".to_owned(),', 'name: "doc".to_owned(),', "default variable name")
if 'variable_type: "date".to_owned(),' not in dynamic:
    raise SystemExit("default variable type not found")
dynamic = dynamic.replace(
    'variable_type: "date".to_owned(),',
    'variable_type: "string".to_owned(),',
    1,
)
if 'params_yaml: "format: \\"%d.%m.%Y\\"".to_owned(),' not in dynamic:
    raise SystemExit("default variable value not found")
dynamic = dynamic.replace(
    'params_yaml: "format: \\"%d.%m.%Y\\"".to_owned(),',
    'params_yaml: "Куцин Иван Юрьевич".to_owned(),',
    1,
)
dynamic = replace_once(
    dynamic,
    '.hint_text("date, clipboard, shell, script, random…")',
    '.hint_text("date, time, string, clipboard, echo, choice, form, random, rhai, script, shell")',
    "variable type hint",
)

params_start = '                ui.label("Параметры YAML без строки params: (можно оставить пустыми)");'
params_end = "                ui.monospace(format!(\"Шаблон: {{{{{}}}}}\", self.name.trim()));"
params_ui = rust(r'''
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
''', 16)
dynamic = replace_between(dynamic, params_start, params_end, params_ui, "variable params UI")

builtin_scan = rust(r'''
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

''')
dynamic = insert_before(dynamic, "pub fn upsert_rule_variable(\n", builtin_scan, "builtin variable scanner")
dynamic = replace_once(
    dynamic,
    "    validate_definition(definition)?;\n    let params = normalized_params(&definition.params_yaml)?;",
    "    let definition = canonical_definition(definition)?;\n"
    "    validate_definition(&definition)?;\n"
    "    let params = normalized_params(&definition.params_yaml)?;",
    "canonical variable start",
)
dynamic = replace_once(
    dynamic,
    "let item = render_variable(definition, params.as_deref(), child_indent + 2);",
    "let item = render_variable(&definition, params.as_deref(), child_indent + 2);",
    "canonical render",
)

canonical = rust(r'''
fn canonical_definition(definition: &VariableDefinition) -> Result<VariableDefinition, String> {
    let variable_type = definition.variable_type.trim().to_ascii_lowercase();
    let mut canonical = definition.clone();
    canonical.name = canonical.name.trim().to_owned();
    canonical.variable_type = variable_type.clone();
    canonical.params_yaml = canonical.params_yaml.trim().to_owned();

    match variable_type.as_str() {
        "time" => {
            canonical.variable_type = "date".to_owned();
            if canonical.params_yaml.is_empty() {
                canonical.params_yaml = "format: \"%H:%M\"".to_owned();
            }
        }
        "string" => {
            if canonical.params_yaml.is_empty() {
                return Err("Для типа string укажите значение строки".to_owned());
            }
            canonical.variable_type = "echo".to_owned();
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

''')
dynamic = insert_before(
    dynamic,
    "fn validate_definition(definition: &VariableDefinition) -> Result<(), String> {\n",
    canonical,
    "variable aliases",
)

alias_tests = rust(r'''
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
       
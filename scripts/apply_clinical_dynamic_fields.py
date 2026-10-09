from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"anchor not found: {label}")
    return text.replace(old, new, 1)

root = Path('.')

cargo = root / 'espanso-editor/Cargo.toml'
t = cargo.read_text(encoding='utf-8')
t = replace_once(
    t,
    'eframe = { version = "0.35.0", default-features = false, features = ["default_fonts", "glow", "x11"] }\nregex.workspace = true\n',
    'eframe = { version = "0.35.0", default-features = false, features = ["default_fonts", "glow", "x11"] }\nchrono.workspace = true\nregex.workspace = true\n',
    'chrono dependency',
)
cargo.write_text(t, encoding='utf-8')

core = root / 'espanso-editor/src/clinical_extender_core.rs'
t = core.read_text(encoding='utf-8')

t = replace_once(
    t,
    '    collections::HashSet,\n',
    '    collections::{BTreeMap, HashSet},\n',
    'BTreeMap import',
)
t = replace_once(t, 'const DATABASE_VERSION: u32 = 1;', 'const DATABASE_VERSION: u32 = 2;', 'database version')

field_types = r'''

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClinicalFieldKind {
    Text,
    Date,
    Integer,
    Decimal,
    Choice,
    Boolean,
}

impl Default for ClinicalFieldKind {
    fn default() -> Self {
        Self::Text
    }
}

impl ClinicalFieldKind {
    fn label(self) -> &'static str {
        match self {
            Self::Text => "Текст",
            Self::Date => "Дата",
            Self::Integer => "Целое",
            Self::Decimal => "Число",
            Self::Choice => "Выбор",
            Self::Boolean => "Да/нет",
        }
    }

    fn all() -> [Self; 6] {
        [
            Self::Text,
            Self::Date,
            Self::Integer,
            Self::Decimal,
            Self::Choice,
            Self::Boolean,
        ]
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ClinicalFieldDefinition {
    pub name: String,
    pub kind: ClinicalFieldKind,
    pub choices: Vec<String>,
}
'''
t = replace_once(
    t,
    'enum ClinicalTab {\n    Visit,\n    Nosologies,\n}\n',
    'enum ClinicalTab {\n    Visit,\n    Nosologies,\n}\n' + field_types,
    'field types',
)

t = replace_once(
    t,
    'pub struct ClinicalDatabase {\n    pub version: u32,\n    pub catalog: Vec<Investigation>,\n    pub templates: Vec<NosologyTemplate>,\n}',
    'pub struct ClinicalDatabase {\n    pub version: u32,\n    pub catalog: Vec<Investigation>,\n    pub templates: Vec<NosologyTemplate>,\n    pub fields: Vec<ClinicalFieldDefinition>,\n}',
    'database fields',
)
t = replace_once(
    t,
    '            catalog,\n            templates: vec![base, hypertension, heart_failure, osteochondrosis],\n        }',
    '            catalog,\n            templates: vec![base, hypertension, heart_failure, osteochondrosis],\n            fields: Vec::new(),\n        }',
    'database default fields',
)

t = replace_once(
    t,
    '    new_investigation: String,\n    status: String,\n}',
    '    new_investigation: String,\n    field_values: BTreeMap<String, String>,\n    active_fields: Vec<String>,\n    status: String,\n}',
    'runtime fields',
)

t = replace_once(
    t,
    '        let saved_db = serialize_database(&db).unwrap_or_default();\n        let mut result = Self {',
    '        let saved_db = serialize_database(&db).unwrap_or_default();\n        let today = today_local_string();\n        let mut field_values = BTreeMap::new();\n        field_values.insert("today".to_owned(), today.clone());\n        field_values.insert("visit_date".to_owned(), today);\n        let mut result = Self {',
    'runtime values init',
)
t = replace_once(
    t,
    '            new_investigation: String::new(),\n            status,\n',
    '            new_investigation: String::new(),\n            field_values,\n            active_fields: Vec::new(),\n            status,\n',
    'runtime values struct init',
)

# Non-localized visit UI is still used by tests/legacy entry points.
t = replace_once(
    t,
    '            ui.label(egui::RichText::new(&self.status).weak());\n            ui.separator();\n\n            visit_section_ui(ui, "Жалобы", &mut self.visit.complaints, 3);',
    '            ui.label(egui::RichText::new(&self.status).weak());\n            ui.separator();\n            self.dynamic_fields_ui(ui);\n\n            visit_section_ui(ui, "Жалобы", &mut self.visit.complaints, 3);',
    'legacy dynamic fields panel',
)

t = replace_once(
    t,
    '        ui.separator();\n\n        template_text_field(\n            ui,\n            "Жалобы",',
    '        ui.separator();\n        ui.small("Динамические поля: {{field_name}}. Даты: {{today}}, {{visit_date}}, {{therapy_start + 2w}}, {{visit_date - 3d}}, также поддерживаются mo и y.");\n\n        template_text_field(\n            ui,\n            "Жалобы",',
    'template dynamic field hint',
)

old_recompose = '''    fn recompose(&mut self, force: bool) {
        let tokens = parse_diagnosis_input(&self.diagnosis_input);
        let (document, matched, unknown) = compose(&self.db, &tokens);
        self.visit.refresh(document, force);
        self.matched = matched;
        self.unknown = unknown;
    }
'''
new_recompose = r'''    fn recompose(&mut self, force: bool) {
        let tokens = parse_diagnosis_input(&self.diagnosis_input);
        let (raw_document, matched, unknown) = compose(&self.db, &tokens);
        self.sync_dynamic_fields(&raw_document);
        let document = render_dynamic_document(raw_document, &self.field_values, &self.db.fields);
        self.visit.refresh(document, force);
        self.matched = matched;
        self.unknown = unknown;
    }

    fn sync_dynamic_fields(&mut self, document: &VisitDocument) {
        self.field_values
            .insert("today".to_owned(), today_local_string());
        self.field_values
            .entry("visit_date".to_owned())
            .or_insert_with(today_local_string);

        let names = collect_dynamic_field_names(document);
        for name in &names {
            self.field_values.entry(name.clone()).or_default();
            if matches!(name.as_str(), "today" | "visit_date") {
                continue;
            }
            if self.db.fields.iter().all(|field| field.name != *name) {
                self.db.fields.push(ClinicalFieldDefinition {
                    name: name.clone(),
                    kind: infer_field_kind(name),
                    choices: Vec::new(),
                });
            }
        }
        self.active_fields = names;
    }

    fn dynamic_fields_ui(&mut self, ui: &mut egui::Ui) {
        if self.active_fields.is_empty() {
            return;
        }

        let mut changed = false;
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Поля осмотра").strong());
                ui.label(egui::RichText::new("значения пациента не сохраняются на диск").weak());
            });
            ui.small("Поддержка дат: +/− Nd, Nw, Nmo, Ny. Пример: {{therapy_start + 2w}}.");

            for name in self.active_fields.clone() {
                if name == "today" {
                    let value = self.field_values.get(&name).cloned().unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.monospace("{{today}}");
                        ui.label(value);
                        ui.label(egui::RichText::new("системная дата").weak());
                    });
                    continue;
                }

                let mut definition = if name == "visit_date" {
                    ClinicalFieldDefinition {
                        name: name.clone(),
                        kind: ClinicalFieldKind::Date,
                        choices: Vec::new(),
                    }
                } else {
                    self.db
                        .fields
                        .iter()
                        .find(|field| field.name == name)
                        .cloned()
                        .unwrap_or_else(|| ClinicalFieldDefinition {
                            name: name.clone(),
                            kind: infer_field_kind(&name),
                            choices: Vec::new(),
                        })
                };
                let original_definition = definition.clone();
                let mut value = self.field_values.get(&name).cloned().unwrap_or_default();
                let original_value = value.clone();

                ui.horizontal_wrapped(|ui| {
                    ui.monospace(format!("{{{{{name}}}}}"));
                    if name == "visit_date" {
                        ui.label("Дата");
                    } else {
                        egui::ComboBox::from_id_salt(format!("clinical_field_kind_{name}"))
                            .selected_text(definition.kind.label())
                            .show_ui(ui, |ui| {
                                for kind in ClinicalFieldKind::all() {
                                    ui.selectable_value(&mut definition.kind, kind, kind.label());
                                }
                            });
                    }

                    match definition.kind {
                        ClinicalFieldKind::Boolean => {
                            let mut checked = matches!(
                                value.trim().to_ascii_lowercase().as_str(),
                                "true" | "1" | "yes" | "да"
                            );
                            if ui.checkbox(&mut checked, "Да").changed() {
                                value = if checked { "true" } else { "false" }.to_owned();
                            }
                        }
                        ClinicalFieldKind::Choice if !definition.choices.is_empty() => {
                            let selected = if value.is_empty() {
                                "— выберите —".to_owned()
                            } else {
                                value.clone()
                            };
                            egui::ComboBox::from_id_salt(format!("clinical_field_choice_{name}"))
                                .selected_text(selected)
                                .show_ui(ui, |ui| {
                                    for choice in definition.choices.clone() {
                                        ui.selectable_value(&mut value, choice.clone(), choice);
                                    }
                                });
                        }
                        _ => {
                            let hint = match definition.kind {
                                ClinicalFieldKind::Date => "ДД.ММ.ГГГГ",
                                ClinicalFieldKind::Integer => "0",
                                ClinicalFieldKind::Decimal => "0,0",
                                ClinicalFieldKind::Choice => "значение",
                                ClinicalFieldKind::Text | ClinicalFieldKind::Boolean => "значение",
                            };
                            ui.add(
                                egui::TextEdit::singleline(&mut value)
                                    .hint_text(hint)
                                    .desired_width(180.0),
                            );
                        }
                    }
                });

                if definition.kind == ClinicalFieldKind::Choice && name != "visit_date" {
                    let mut choices_text = definition.choices.join(", ");
                    ui.horizontal(|ui| {
                        ui.add_space(28.0);
                        ui.label(egui::RichText::new("Варианты:").weak());
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut choices_text)
                                    .hint_text("вариант 1, вариант 2")
                                    .desired_width(320.0),
                            )
                            .changed()
                        {
                            definition.choices = choices_text
                                .split([',', ';', '\n'])
                                .map(str::trim)
                                .filter(|choice| !choice.is_empty())
                                .map(ToOwned::to_owned)
                                .collect();
                        }
                    });
                }

                if definition.kind == ClinicalFieldKind::Date
                    && !value.trim().is_empty()
                    && parse_clinical_date(&value).is_none()
                {
                    ui.colored_label(
                        egui::Color32::from_rgb(210, 135, 25),
                        format!("{name}: дата не распознана; используйте ДД.ММ.ГГГГ или ГГГГ-ММ-ДД"),
                    );
                }

                if value != original_value {
                    self.field_values.insert(name.clone(), value);
                    changed = true;
                }
                if name != "visit_date" && definition != original_definition {
                    if let Some(slot) = self.db.fields.iter_mut().find(|field| field.name == name) {
                        *slot = definition;
                    } else {
                        self.db.fields.push(definition);
                    }
                    changed = true;
                }
            }
        });
        ui.add_space(6.0);

        if changed {
            self.recompose(false);
        }
    }
'''
t = replace_once(t, old_recompose, new_recompose, 'recompose and field UI')

helpers = r'''

fn infer_field_kind(name: &str) -> ClinicalFieldKind {
    let normalized = name.trim().to_ascii_lowercase();
    if matches!(normalized.as_str(), "today" | "visit_date")
        || normalized.ends_with("_date")
        || normalized.starts_with("date_")
    {
        ClinicalFieldKind::Date
    } else if normalized.ends_with("_count") || normalized.ends_with("_age") {
        ClinicalFieldKind::Integer
    } else {
        ClinicalFieldKind::Text
    }
}

fn today_local_string() -> String {
    chrono::Local::now().date_naive().format("%d.%m.%Y").to_string()
}

fn visit_document_parts(document: &VisitDocument) -> [&str; 8] {
    [
        &document.complaints,
        &document.disease_history,
        &document.life_history,
        &document.past_diseases,
        &document.objective_status,
        &document.examination_plan,
        &document.treatment,
        &document.recommendations,
    ]
}

fn collect_dynamic_field_names(document: &VisitDocument) -> Vec<String> {
    let placeholder = regex::Regex::new(r"\{\{\s*([^{}]+?)\s*\}\}").expect("valid placeholder regex");
    let identifier = regex::Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*$").expect("valid identifier regex");
    let date_expression = regex::Regex::new(
        r"^([A-Za-z_][A-Za-z0-9_]*)\s*[+-]\s*\d+\s*(?:mo|d|w|y)$",
    )
    .expect("valid date expression regex");
    let mut result = Vec::new();
    let mut seen = HashSet::new();

    for text in visit_document_parts(document) {
        for captures in placeholder.captures_iter(text) {
            let expression = captures.get(1).map_or("", |item| item.as_str()).trim();
            let name = if identifier.is_match(expression) {
                Some(expression)
            } else {
                date_expression
                    .captures(expression)
                    .and_then(|items| items.get(1).map(|item| item.as_str()))
            };
            if let Some(name) = name {
                let name = name.to_owned();
                if seen.insert(name.clone()) {
                    result.push(name);
                }
            }
        }
    }
    result
}

fn render_dynamic_document(
    document: VisitDocument,
    values: &BTreeMap<String, String>,
    definitions: &[ClinicalFieldDefinition],
) -> VisitDocument {
    VisitDocument {
        complaints: render_dynamic_text(&document.complaints, values, definitions),
        disease_history: render_dynamic_text(&document.disease_history, values, definitions),
        life_history: render_dynamic_text(&document.life_history, values, definitions),
        past_diseases: render_dynamic_text(&document.past_diseases, values, definitions),
        objective_status: render_dynamic_text(&document.objective_status, values, definitions),
        examination_plan: render_dynamic_text(&document.examination_plan, values, definitions),
        treatment: render_dynamic_text(&document.treatment, values, definitions),
        recommendations: render_dynamic_text(&document.recommendations, values, definitions),
    }
}

fn render_dynamic_text(
    text: &str,
    values: &BTreeMap<String, String>,
    definitions: &[ClinicalFieldDefinition],
) -> String {
    let placeholder = regex::Regex::new(r"\{\{\s*([^{}]+?)\s*\}\}").expect("valid placeholder regex");
    placeholder
        .replace_all(text, |captures: &regex::Captures<'_>| {
            let whole = captures.get(0).map_or("", |item| item.as_str());
            let expression = captures.get(1).map_or("", |item| item.as_str()).trim();
            resolve_dynamic_expression(expression, values, definitions)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| whole.to_owned())
        })
        .into_owned()
}

fn resolve_dynamic_expression(
    expression: &str,
    values: &BTreeMap<String, String>,
    definitions: &[ClinicalFieldDefinition],
) -> Option<String> {
    let identifier = regex::Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*$").ok()?;
    if identifier.is_match(expression) {
        let value = values.get(expression)?.trim();
        if value.is_empty() {
            return None;
        }
        let kind = definitions
            .iter()
            .find(|field| field.name == expression)
            .map_or_else(|| infer_field_kind(expression), |field| field.kind);
        return Some(match kind {
            ClinicalFieldKind::Date => parse_clinical_date(value)
                .map(|date| date.format("%d.%m.%Y").to_string())
                .unwrap_or_else(|| value.to_owned()),
            ClinicalFieldKind::Boolean => {
                if matches!(
                    value.to_ascii_lowercase().as_str(),
                    "true" | "1" | "yes" | "да"
                ) {
                    "Да".to_owned()
                } else {
                    "Нет".to_owned()
                }
            }
            _ => value.to_owned(),
        });
    }

    let date_expression = regex::Regex::new(
        r"^([A-Za-z_][A-Za-z0-9_]*)\s*([+-])\s*(\d+)\s*(mo|d|w|y)$",
    )
    .ok()?;
    let captures = date_expression.captures(expression)?;
    let base_name = captures.get(1)?.as_str();
    let sign = captures.get(2)?.as_str();
    let amount = captures.get(3)?.as_str().parse::<u32>().ok()?;
    let unit = captures.get(4)?.as_str();
    let base = parse_clinical_date(values.get(base_name)?.trim())?;
    let forward = sign == "+";
    let date = match unit {
        "d" => {
            let days = i64::from(amount);
            if forward {
                base.checked_add_signed(chrono::Duration::days(days))?
            } else {
                base.checked_sub_signed(chrono::Duration::days(days))?
            }
        }
        "w" => {
            let days = i64::from(amount).checked_mul(7)?;
            if forward {
                base.checked_add_signed(chrono::Duration::days(days))?
            } else {
                base.checked_sub_signed(chrono::Duration::days(days))?
            }
        }
        "mo" => {
            let months = chrono::Months::new(amount);
            if forward {
                base.checked_add_months(months)?
            } else {
                base.checked_sub_months(months)?
            }
        }
        "y" => {
            let months = chrono::Months::new(amount.checked_mul(12)?);
            if forward {
                base.checked_add_months(months)?
            } else {
                base.checked_sub_months(months)?
            }
        }
        _ => return None,
    };
    Some(date.format("%d.%m.%Y").to_string())
}

fn parse_clinical_date(value: &str) -> Option<chrono::NaiveDate> {
    let value = value.trim();
    ["%d.%m.%Y", "%Y-%m-%d", "%d/%m/%Y"]
        .into_iter()
        .find_map(|format| chrono::NaiveDate::parse_from_str(value, format).ok())
}
'''

t = replace_once(t, '\n#[cfg(test)]\nmod tests {', helpers + '\n\n#[cfg(test)]\nmod tests {', 'dynamic helpers')

new_tests = r'''

    #[test]
    fn dynamic_fields_are_discovered_from_plain_and_date_expressions() {
        let document = VisitDocument {
            disease_history: "Начало {{therapy_start}}, контроль {{therapy_start + 2w}}; дата {{visit_date}}".to_owned(),
            ..Default::default()
        };
        assert_eq!(
            collect_dynamic_field_names(&document),
            vec!["therapy_start".to_owned(), "visit_date".to_owned()]
        );
    }

    #[test]
    fn dynamic_values_and_date_arithmetic_are_rendered() {
        let mut values = BTreeMap::new();
        values.insert("patient".to_owned(), "Иван".to_owned());
        values.insert("therapy_start".to_owned(), "01.09.2026".to_owned());
        let definitions = vec![
            ClinicalFieldDefinition {
                name: "patient".to_owned(),
                kind: ClinicalFieldKind::Text,
                choices: Vec::new(),
            },
            ClinicalFieldDefinition {
                name: "therapy_start".to_owned(),
                kind: ClinicalFieldKind::Date,
                choices: Vec::new(),
            },
        ];
        assert_eq!(
            render_dynamic_text(
                "Пациент {{patient}}. Контроль {{therapy_start + 2w}}.",
                &values,
                &definitions,
            ),
            "Пациент Иван. Контроль 15.09.2026."
        );
    }

    #[test]
    fn date_month_and_year_operations_clamp_calendar_dates() {
        let mut values = BTreeMap::new();
        values.insert("start_date".to_owned(), "31.01.2025".to_owned());
        assert_eq!(
            render_dynamic_text("{{start_date + 1mo}}", &values, &[]),
            "28.02.2025"
        );
        values.insert("start_date".to_owned(), "29.02.2024".to_owned());
        assert_eq!(
            render_dynamic_text("{{start_date + 1y}}", &values, &[]),
            "28.02.2025"
        );
    }

    #[test]
    fn unresolved_dynamic_expression_stays_visible() {
        assert_eq!(
            render_dynamic_text("Контроль {{missing_date + 3d}}", &BTreeMap::new(), &[]),
            "Контроль {{missing_date + 3d}}"
        );
    }
'''
# Insert tests at the beginning of the tests module, after use super.
t = replace_once(t, 'mod tests {\n    use super::*;\n', 'mod tests {\n    use super::*;\n' + new_tests, 'dynamic tests')

core.write_text(t, encoding='utf-8')

localized = root / 'espanso-editor/src/clinical_extender.rs'
t = localized.read_text(encoding='utf-8')
t = replace_once(
    t,
    '                ui.label(egui::RichText::new(&self.status).weak());\n                ui.separator();\n\n                visit_section_ui(ui, "Жалобы", &mut self.visit.complaints, 3);',
    '                ui.label(egui::RichText::new(&self.status).weak());\n                ui.separator();\n                self.dynamic_fields_ui(ui);\n\n                visit_section_ui(ui, "Жалобы", &mut self.visit.complaints, 3);',
    'localized dynamic fields panel',
)
localized.write_text(t, encoding='utf-8')

print('Clinical dynamic fields patch applied')

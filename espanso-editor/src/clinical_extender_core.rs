use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

const STORE_DIR: &str = "clinical_extender";
const STORE_FILE: &str = "nosologies.yml";
const DATABASE_VERSION: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClinicalTab {
    Visit,
    Nosologies,
}


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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ClinicalSections {
    pub complaints: String,
    pub disease_history: String,
    pub life_history: String,
    pub past_diseases: String,
    pub objective_status: String,
    pub examination_plan: String,
    pub treatment: String,
    pub recommendations: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Investigation {
    pub id: String,
    pub label: String,
}

impl Default for Investigation {
    fn default() -> Self {
        Self {
            id: "custom".to_owned(),
            label: "Новое исследование".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct NosologyTemplate {
    pub is_base: bool,
    pub code_pattern: String,
    pub title: String,
    pub aliases: Vec<String>,
    pub sections: ClinicalSections,
    pub investigation_ids: Vec<String>,
    pub catalog: Vec<Investigation>,
    pub fields: Vec<ClinicalFieldDefinition>,
}

impl Default for NosologyTemplate {
    fn default() -> Self {
        Self {
            is_base: false,
            code_pattern: String::new(),
            title: "Новая нозология".to_owned(),
            aliases: Vec::new(),
            sections: ClinicalSections::default(),
            investigation_ids: Vec::new(),
            catalog: Vec::new(),
            fields: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ClinicalDatabase {
    pub version: u32,
    pub catalog: Vec<Investigation>,
    pub templates: Vec<NosologyTemplate>,
    pub fields: Vec<ClinicalFieldDefinition>,
}

impl Default for ClinicalDatabase {
    fn default() -> Self {
        let catalog = vec![
            investigation("cbc", "ОАК"),
            investigation("urinalysis", "ОАМ"),
            investigation("biochemistry", "БХА"),
            investigation("ecg", "ЭКГ"),
            investigation("echocardiography", "ЭхоКГ"),
            investigation("mau", "МАУ"),
            investigation("nt_probnp", "NT-proBNP"),
            investigation(
                "spine_xray",
                "Рентгенография соответствующего отдела позвоночника",
            ),
        ];

        let mut base = NosologyTemplate {
            is_base: true,
            code_pattern: "BASE".to_owned(),
            title: "Базовый терапевтический профиль".to_owned(),
            ..Default::default()
        };
        base.investigation_ids = vec![
            "cbc".to_owned(),
            "urinalysis".to_owned(),
            "biochemistry".to_owned(),
            "ecg".to_owned(),
        ];

        let mut hypertension = NosologyTemplate {
            code_pattern: "I11.9".to_owned(),
            title: "Гипертензивная болезнь сердца без сердечной недостаточности".to_owned(),
            aliases: vec!["АГ".to_owned(), "ГБ".to_owned()],
            ..Default::default()
        };
        hypertension.sections.disease_history = "Артериальной гипертензией страдает длительное время. Антигипертензивную терапию принимает со слов, рекомендаций придерживается.".to_owned();
        hypertension.investigation_ids = vec!["echocardiography".to_owned(), "mau".to_owned()];

        let mut heart_failure = NosologyTemplate {
            code_pattern: "I50.9".to_owned(),
            title: "Сердечная недостаточность неуточнённая".to_owned(),
            aliases: vec!["ХСН".to_owned(), "HCN".to_owned(), "CHF".to_owned()],
            ..Default::default()
        };
        heart_failure.investigation_ids = vec!["nt_probnp".to_owned()];

        let mut osteochondrosis = NosologyTemplate {
            code_pattern: "M42.1".to_owned(),
            title: "Остеохондроз позвоночника у взрослых".to_owned(),
            aliases: vec!["ОСТЕОХОНДРОЗ".to_owned()],
            ..Default::default()
        };
        osteochondrosis.sections.disease_history = "Остеохондрозом позвоночника страдает длительное время. Постоянную анальгетическую терапию не принимает.".to_owned();
        osteochondrosis.investigation_ids = vec!["spine_xray".to_owned()];

        Self {
            version: DATABASE_VERSION,
            catalog,
            templates: vec![base, hypertension, heart_failure, osteochondrosis],
            fields: Vec::new(),
        }
    }
}

fn investigation(id: &str, label: &str) -> Investigation {
    Investigation {
        id: id.to_owned(),
        label: label.to_owned(),
    }
}

fn merge_fields(target: &mut Vec<ClinicalFieldDefinition>, additions: &[ClinicalFieldDefinition]) {
    for field in additions {
        if let Some(existing) = target.iter_mut().find(|item| item.name == field.name) {
            *existing = field.clone();
        } else {
            target.push(field.clone());
        }
    }
}

fn merge_investigations(target: &mut Vec<Investigation>, additions: &[Investigation]) {
    for item in additions {
        if let Some(existing) = target.iter_mut().find(|entry| entry.id == item.id) {
            *existing = item.clone();
        } else {
            target.push(item.clone());
        }
    }
}

fn active_template_indices(db: &ClinicalDatabase, tokens: &[String]) -> Vec<usize> {
    let mut selected: Vec<(usize, usize)> = db.templates.iter().enumerate()
        .filter(|(_, t)| t.is_base).map(|(i, _)| (0, i)).collect();
    for token in tokens {
        let mut matches: Vec<_> = db.templates.iter().enumerate()
            .filter(|(_, t)| !t.is_base)
            .filter_map(|(i, t)| match_score(t, token).map(|score| (score, i))).collect();
        matches.sort_by_key(|&(score, _)| score);
        for entry in matches {
            if !selected.iter().any(|&(_, i)| i == entry.1) {
                selected.push(entry);
            }
        }
    }
    selected.sort_by_key(|&(score, _)| score);
    selected.into_iter().map(|(_, i)| i).collect()
}

fn effective_fields(db: &ClinicalDatabase, tokens: &[String]) -> Vec<ClinicalFieldDefinition> {
    let mut fields = db.fields.clone();
    for index in active_template_indices(db, tokens) {
        merge_fields(&mut fields, &db.templates[index].fields);
    }
    fields
}

fn section_field_names(sections: &ClinicalSections) -> Vec<String> {
    collect_dynamic_field_names(&VisitDocument {
        complaints: sections.complaints.clone(),
        disease_history: sections.disease_history.clone(),
        life_history: sections.life_history.clone(),
        past_diseases: sections.past_diseases.clone(),
        objective_status: sections.objective_status.clone(),
        examination_plan: sections.examination_plan.clone(),
        treatment: sections.treatment.clone(),
        recommendations: sections.recommendations.clone(),
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct VisitDocument {
    complaints: String,
    disease_history: String,
    life_history: String,
    past_diseases: String,
    objective_status: String,
    examination_plan: String,
    treatment: String,
    recommendations: String,
}

impl VisitDocument {
    fn as_clipboard_text(&self) -> String {
        let sections = [
            ("Жалобы", self.complaints.as_str()),
            ("Анамнез заболевания", self.disease_history.as_str()),
            ("Анамнез жизни", self.life_history.as_str()),
            ("Перенесённые заболевания / операции", self.past_diseases.as_str()),
            ("Объективно", self.objective_status.as_str()),
            ("План обследования", self.examination_plan.as_str()),
            ("Лечение", self.treatment.as_str()),
            ("Рекомендации", self.recommendations.as_str()),
        ];
        sections
            .into_iter()
            .filter(|(_, text)| !text.trim().is_empty())
            .map(|(title, text)| format!("{title}:\n{}", text.trim()))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[derive(Debug, Clone, Default)]
struct EditableSection {
    text: String,
    generated: String,
}

impl EditableSection {
    fn modified(&self) -> bool {
        self.text != self.generated
    }

    fn update_preserving_manual(&mut self, generated: String) {
        if !self.modified() {
            self.text.clone_from(&generated);
            self.generated = generated;
        }
    }

    fn replace(&mut self, generated: String) {
        self.text.clone_from(&generated);
        self.generated = generated;
    }
}

#[derive(Debug, Clone, Default)]
struct VisitEditor {
    complaints: EditableSection,
    disease_history: EditableSection,
    life_history: EditableSection,
    past_diseases: EditableSection,
    objective_status: EditableSection,
    examination_plan: EditableSection,
    treatment: EditableSection,
    recommendations: EditableSection,
}

impl VisitEditor {
    fn refresh(&mut self, generated: VisitDocument, force: bool) {
        update_section(&mut self.complaints, generated.complaints, force);
        update_section(&mut self.disease_history, generated.disease_history, force);
        update_section(&mut self.life_history, generated.life_history, force);
        update_section(&mut self.past_diseases, generated.past_diseases, force);
        update_section(&mut self.objective_status, generated.objective_status, force);
        update_section(
            &mut self.examination_plan,
            generated.examination_plan,
            force,
        );
        update_section(&mut self.treatment, generated.treatment, force);
        update_section(&mut self.recommendations, generated.recommendations, force);
    }

    fn document(&self) -> VisitDocument {
        VisitDocument {
            complaints: self.complaints.text.clone(),
            disease_history: self.disease_history.text.clone(),
            life_history: self.life_history.text.clone(),
            past_diseases: self.past_diseases.text.clone(),
            objective_status: self.objective_status.text.clone(),
            examination_plan: self.examination_plan.text.clone(),
            treatment: self.treatment.text.clone(),
            recommendations: self.recommendations.text.clone(),
        }
    }
}

fn update_section(section: &mut EditableSection, generated: String, force: bool) {
    if force {
        section.replace(generated);
    } else {
        section.update_preserving_manual(generated);
    }
}

pub struct ClinicalExtender {
    root: PathBuf,
    db: ClinicalDatabase,
    saved_db: String,
    tab: ClinicalTab,
    diagnosis_input: String,
    visit: VisitEditor,
    matched: Vec<String>,
    unknown: Vec<String>,
    selected_template: Option<usize>,
    draft: NosologyTemplate,
    aliases_edit: String,
    draft_is_new: bool,
    draft_dirty: bool,
    new_investigation: String,
    new_field: String,
    field_values: BTreeMap<String, String>,
    active_fields: Vec<String>,
    status: String,
    storage_error: bool,
}

impl ClinicalExtender {
    pub fn load(root: PathBuf) -> Self {
        let path = database_path(&root);
        let (mut db, status, storage_error) = match load_database(&path) {
            Ok(Some(db)) => (db, format!("Библиотека загружена: {}", root.join(STORE_DIR).display()), false),
            Ok(None) => (
                ClinicalDatabase::default(),
                "Создана стартовая локальная библиотека нозологий".to_owned(),
                false,
            ),
            Err(error) => (
                ClinicalDatabase::default(),
                format!("Не удалось прочитать библиотеку; загружены стартовые шаблоны: {error}"),
                true,
            ),
        };
        ensure_base_template(&mut db);
        let saved_db = serialize_database(&db).unwrap_or_default();
        let today = today_local_string();
        let mut field_values = BTreeMap::new();
        field_values.insert("today".to_owned(), today.clone());
        field_values.insert("visit_date".to_owned(), today);
        let mut result = Self {
            root,
            db,
            saved_db,
            tab: ClinicalTab::Visit,
            diagnosis_input: String::new(),
            visit: VisitEditor::default(),
            matched: Vec::new(),
            unknown: Vec::new(),
            selected_template: None,
            draft: NosologyTemplate::default(),
            aliases_edit: String::new(),
            draft_is_new: false,
            draft_dirty: false,
            new_investigation: String::new(),
            new_field: String::new(),
            field_values,
            active_fields: Vec::new(),
            status,
            storage_error,
        };
        result.select_template(0);
        result.recompose(false);
        result
    }

    pub fn dirty(&self) -> bool {
        self.draft_dirty || serialize_database(&self.db).unwrap_or_default() != self.saved_db
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.tab, ClinicalTab::Visit, "Осмотр");
            ui.selectable_value(&mut self.tab, ClinicalTab::Nosologies, "Редактор нозологий");
            ui.separator();
            ui.label(
                egui::RichText::new("Локально: данные текущего пациента на диск не сохраняются")
                    .weak(),
            );
        });
        ui.separator();

        match self.tab {
            ClinicalTab::Visit => self.visit_ui(ui),
            ClinicalTab::Nosologies => self.nosologies_ui(ui),
        }
    }

    fn visit_ui(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Clinical Extender · конструктор осмотра");
            ui.label("Введите коды МКБ-10 или псевдонимы через запятую. Базовый профиль применяется всегда; обследования объединяются без дублей.");
            let changed = ui
                .add(
                    egui::TextEdit::singleline(&mut self.diagnosis_input)
                        .hint_text("I11.9, M42.1, ХСН")
                        .desired_width(f32::INFINITY),
                )
                .changed();
            if changed {
                self.recompose(false);
            }

            ui.horizontal_wrapped(|ui| {
                if ui.button("Обновить из шаблонов").clicked() {
                    self.recompose(false);
                    "Обновлены нетронутые поля; ручные правки сохранены"
                        .clone_into(&mut self.status);
                }
                if ui
                    .button("Пересобрать всё")
                    .on_hover_text("Заменяет и поля, отредактированные вручную")
                    .clicked()
                {
                    self.recompose(true);
                    "Осмотр полностью пересобран из шаблонов".clone_into(&mut self.status);
                }
                if ui.button("Копировать весь осмотр").clicked() {
                    ui.ctx().copy_text(self.visit.document().as_clipboard_text());
                    "Осмотр скопирован в буфер".clone_into(&mut self.status);
                }
            });

            if !self.matched.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Применено:");
                    for item in &self.matched {
                        ui.label(egui::RichText::new(item).strong());
                    }
                });
            }
            if !self.unknown.is_empty() {
                ui.colored_label(
                    egui::Color32::from_rgb(210, 135, 25),
                    format!("Нет шаблонов: {}", self.unknown.join(", ")),
                );
            }
            ui.label(egui::RichText::new(&self.status).weak());
            ui.separator();
            self.dynamic_fields_ui(ui);

            visit_section_ui(ui, "Жалобы", &mut self.visit.complaints, 3);
            visit_section_ui(
                ui,
                "Анамнез заболевания",
                &mut self.visit.disease_history,
                5,
            );
            visit_section_ui(ui, "Анамнез жизни", &mut self.visit.life_history, 4);
            visit_section_ui(
                ui,
                "Перенесённые заболевания / операции",
                &mut self.visit.past_diseases,
                3,
            );
            visit_section_ui(ui, "Объективно", &mut self.visit.objective_status, 4);
            visit_section_ui(
                ui,
                "План обследования",
                &mut self.visit.examination_plan,
                4,
            );
            visit_section_ui(ui, "Лечение", &mut self.visit.treatment, 4);
            visit_section_ui(ui, "Рекомендации", &mut self.visit.recommendations, 4);
        });
    }

    fn nosologies_ui(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |columns| {
            columns[0].set_min_width(260.0);
            columns[0].heading("Шаблоны");
            if columns[0].button("+ Новая нозология").clicked() {
                self.start_new_template();
            }
            columns[0].separator();

            let items = self
                .db
                .templates
                .iter()
                .enumerate()
                .map(|(index, template)| {
                    let label = if template.is_base {
                        format!("Базовый профиль · {}", template.title)
                    } else {
                        format!("{} · {}", template.code_pattern, template.title)
                    };
                    (index, label)
                })
                .collect::<Vec<_>>();
            egui::ScrollArea::vertical()
                .id_salt("clinical_template_list")
                .max_height(650.0)
                .show(&mut columns[0], |ui| {
                    for (index, label) in items {
                        if ui
                            .selectable_label(
                                !self.draft_is_new && self.selected_template == Some(index),
                                label,
                            )
                            .clicked()
                        {
                            if self.draft_dirty {
                                "Несохранённые правки шаблона отброшены"
                                    .clone_into(&mut self.status);
                            }
                            self.select_template(index);
                        }
                    }
                });

            egui::ScrollArea::vertical()
                .id_salt("clinical_template_editor")
                .show(&mut columns[1], |ui| self.template_editor_ui(ui));
        });
    }

    fn template_editor_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading(if self.draft.is_base {
            "Базовый профиль"
        } else if self.draft_is_new {
            "Новая нозология"
        } else {
            "Редактирование нозологии"
        });

        if self.draft.is_base {
            ui.label("Базовый профиль добавляется к каждому осмотру и не сопоставляется с кодом МКБ.");
        } else {
            ui.label("Код поддерживает точное совпадение и маску со звёздочкой, например I11.* или M42.*.");
            ui.horizontal(|ui| {
                ui.label("Код / маска");
                if ui
                    .text_edit_singleline(&mut self.draft.code_pattern)
                    .changed()
                {
                    self.draft_dirty = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Псевдонимы");
                if ui.text_edit_singleline(&mut self.aliases_edit).changed() {
                    self.draft_dirty = true;
                }
            });
            ui.small("Псевдонимы через запятую, например АГ, ГБ.");
        }

        ui.horizontal(|ui| {
            ui.label("Название");
            if ui.text_edit_singleline(&mut self.draft.title).changed() {
                self.draft_dirty = true;
            }
        });
        ui.separator();
        ui.small("Динамические поля: {{field_name}}. Даты: {{today}}, {{visit_date}}, {{therapy_start + 2w}}, {{visit_date - 3d}}, также поддерживаются mo и y.");

        template_text_field(
            ui,
            "Жалобы",
            &mut self.draft.sections.complaints,
            &mut self.draft_dirty,
            3,
        );
        template_text_field(
            ui,
            "Анамнез заболевания",
            &mut self.draft.sections.disease_history,
            &mut self.draft_dirty,
            4,
        );
        template_text_field(
            ui,
            "Анамнез жизни",
            &mut self.draft.sections.life_history,
            &mut self.draft_dirty,
            3,
        );
        template_text_field(
            ui,
            "Перенесённые заболевания / операции",
            &mut self.draft.sections.past_diseases,
            &mut self.draft_dirty,
            3,
        );
        template_text_field(
            ui,
            "Объективно",
            &mut self.draft.sections.objective_status,
            &mut self.draft_dirty,
            3,
        );

        ui.group(|ui| {
            ui.label(egui::RichText::new("Структурированный план обследования").strong());
            let mut catalog = self.db.catalog.clone();
            for template in self.db.templates.iter().filter(|t| t.is_base) {
                merge_investigations(&mut catalog, &template.catalog);
            }
            merge_investigations(&mut catalog, &self.draft.catalog);
            let inherited: HashSet<String> = self.db.templates.iter()
                .filter(|t| t.is_base && !self.draft.is_base)
                .flat_map(|t| t.investigation_ids.iter().cloned()).collect();
            for investigation in catalog {
                let mut checked = self
                    .draft
                    .investigation_ids
                    .iter()
                    .any(|id| id == &investigation.id);
                if inherited.contains(&investigation.id) {
                    ui.add_enabled(false, egui::Checkbox::new(&mut true, format!("{} · из базы", investigation.label)));
                    continue;
                }
                if ui.checkbox(&mut checked, &investigation.label).changed() {
                    if checked {
                        self.draft.investigation_ids.push(investigation.id);
                    } else {
                        self.draft
                            .investigation_ids
                            .retain(|id| id != &investigation.id);
                    }
                    self.draft_dirty = true;
                }
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_investigation)
                        .hint_text("Добавить исследование в каталог"),
                );
                if ui.button("Добавить").clicked() {
                    self.add_custom_investigation();
                }
            });
        });

        template_text_field(
            ui,
            "Дополнение к плану обследования",
            &mut self.draft.sections.examination_plan,
            &mut self.draft_dirty,
            3,
        );
        template_text_field(
            ui,
            "Лечение",
            &mut self.draft.sections.treatment,
            &mut self.draft_dirty,
            3,
        );
        template_text_field(
            ui,
            "Рекомендации",
            &mut self.draft.sections.recommendations,
            &mut self.draft_dirty,
            3,
        );

        self.template_fields_ui(ui);
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if ui.button("Применить шаблон").clicked() {
                self.apply_draft();
            }
            if ui
                .button("Сохранить библиотеку")
                .on_hover_text("Сохраняет только шаблоны, не текст текущего пациента")
                .clicked()
            {
                if self.draft_dirty {
                    self.apply_draft();
                }
                if !self.draft_dirty {
                    self.save_database();
                }
            }
            if !self.draft.is_base
                && !self.draft_is_new
                && ui.button("Удалить нозологию").clicked()
            {
                self.delete_selected_template();
            }
        });
        if self.draft_dirty {
            ui.colored_label(
                egui::Color32::from_rgb(210, 135, 25),
                "Шаблон изменён, но ещё не применён",
            );
        }
        ui.label(egui::RichText::new(&self.status).weak());
    }

    fn template_fields_ui(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.label(egui::RichText::new("Поля этого шаблона").strong());
        ui.small("Добавьте {{имя_поля}} латиницей в текст выше. Здесь задаются тип и варианты; значения пациента не сохраняются.");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.new_field).hint_text("Имя поля, например therapy_start"));
            if ui.button("Добавить поле").clicked() {
                let name = self.new_field.trim();
                let valid = !name.is_empty() && name.bytes().enumerate().all(|(i, b)|
                    b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit()));
                if valid && !matches!(name, "today" | "visit_date")
                    && !self.draft.fields.iter().any(|f| f.name == name) {
                    self.draft.fields.push(ClinicalFieldDefinition {
                        name: name.to_owned(), kind: infer_field_kind(name), choices: Vec::new(),
                    });
                    self.new_field.clear();
                    self.draft_dirty = true;
                } else {
                    self.status = "Имя поля должно быть уникальным: латиница, цифры, подчёркивание; первая буква или подчёркивание. today и visit_date зарезервированы.".to_owned();
                }
            }
        });
        let inherited = effective_fields(&self.db, &[]);
        let mut names = section_field_names(&self.draft.sections);
        for field in &self.draft.fields {
            if !names.contains(&field.name) {
                names.push(field.name.clone());
            }
        }
        for name in names {
            if matches!(name.as_str(), "today" | "visit_date") {
                continue;
            }
            let local = self.draft.fields.iter().find(|f| f.name == name).cloned();
            let mut field = local.clone().or_else(|| inherited.iter().find(|f| f.name == name).cloned())
                .unwrap_or_else(|| ClinicalFieldDefinition {
                    name: name.clone(), kind: infer_field_kind(&name), choices: Vec::new(),
                });
            let original = field.clone();
            let mut reset = false;
            ui.push_id(("template_field", &name), |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.monospace(&name);
                    egui::ComboBox::from_id_salt("kind").selected_text(field.kind.label()).show_ui(ui, |ui| {
                        for kind in ClinicalFieldKind::all() {
                            ui.selectable_value(&mut field.kind, kind, kind.label());
                        }
                    });
                    if local.is_some() {
                        reset = ui.small_button("Сбросить определение").clicked();
                    } else {
                        ui.weak("из базы / автоматически");
                    }
                });
                if field.kind == ClinicalFieldKind::Choice {
                    let mut choices = field.choices.join(", ");
                    if ui.text_edit_singleline(&mut choices).changed() {
                        field.choices = choices.split([',', ';']).map(str::trim)
                            .filter(|s| !s.is_empty()).map(ToOwned::to_owned).collect();
                    }
                }
            });
            if reset {
                self.draft.fields.retain(|f| f.name != name);
                self.draft_dirty = true;
            } else if field != original {
                merge_fields(&mut self.draft.fields, &[field]);
                self.draft_dirty = true;
            }
        }
    }

    fn recompose(&mut self, force: bool) {
        let tokens = parse_diagnosis_input(&self.diagnosis_input);
        let (raw_document, matched, unknown) = compose(&self.db, &tokens);
        self.sync_dynamic_fields(&raw_document);
        let definitions = effective_fields(&self.db, &tokens);
        let document = render_dynamic_document(raw_document, &self.field_values, &definitions);
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

        let mut names = collect_dynamic_field_names(document);
        for field in effective_fields(&self.db, &parse_diagnosis_input(&self.diagnosis_input)) {
            if !names.contains(&field.name) {
                names.push(field.name);
            }
        }
        for name in &names {
            self.field_values.entry(name.clone()).or_default();
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
                    effective_fields(&self.db, &parse_diagnosis_input(&self.diagnosis_input))
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
                    // Store an edited definition in its most specific active owner.
                    let tokens = parse_diagnosis_input(&self.diagnosis_input);
                    let owner = active_template_indices(&self.db, &tokens).into_iter().rev()
                        .find(|&i| self.db.templates[i].fields.iter().any(|f| f.name == name)
                            || section_field_names(&self.db.templates[i].sections).contains(&name));
                    if let Some(index) = owner {
                        merge_fields(&mut self.db.templates[index].fields, &[definition]);
                        if self.selected_template == Some(index) && !self.draft_dirty {
                            self.draft = self.db.templates[index].clone();
                        }
                    } else {
                        merge_fields(&mut self.db.fields, &[definition]);
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

    fn select_template(&mut self, index: usize) {
        if let Some(template) = self.db.templates.get(index).cloned() {
            self.aliases_edit = template.aliases.join(", ");
            self.draft = template;
            self.selected_template = Some(index);
            self.draft_is_new = false;
            self.draft_dirty = false;
        }
    }

    fn start_new_template(&mut self) {
        self.selected_template = None;
        self.draft = NosologyTemplate::default();
        self.aliases_edit.clear();
        self.draft_is_new = true;
        self.draft_dirty = true;
        "Заполните новый шаблон и нажмите «Применить шаблон»".clone_into(&mut self.status);
    }

    fn apply_draft(&mut self) {
        self.draft.aliases = self
            .aliases_edit
            .split([',', ';', '\n'])
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        self.draft.code_pattern = self.draft.code_pattern.trim().to_uppercase();
        self.draft.title = self.draft.title.trim().to_owned();

        if !self.draft.is_base && self.draft.code_pattern.is_empty() {
            "Код или маска нозологии не заполнены".clone_into(&mut self.status);
            return;
        }
        if self.draft.title.is_empty() {
            "Название шаблона не заполнено".clone_into(&mut self.status);
            return;
        }

        if self.db.templates.iter().enumerate().any(|(index, template)|
            (self.draft_is_new || Some(index) != self.selected_template)
                && template.code_pattern.eq_ignore_ascii_case(&self.draft.code_pattern)) {
            self.status = "Шаблон с таким кодом уже существует".to_owned();
            return;
        }
        if self.draft_is_new {
            self.db.templates.push(self.draft.clone());
            let index = self.db.templates.len().saturating_sub(1);
            self.selected_template = Some(index);
            self.draft_is_new = false;
        } else if let Some(index) = self.selected_template {
            if let Some(slot) = self.db.templates.get_mut(index) {
                *slot = self.draft.clone();
            }
        }
        self.draft_dirty = false;
        self.recompose(false);
        "Шаблон применён к текущей библиотеке".clone_into(&mut self.status);
    }

    fn delete_selected_template(&mut self) {
        let Some(index) = self.selected_template else {
            return;
        };
        if self
            .db
            .templates
            .get(index)
            .is_some_and(|template| template.is_base)
        {
            "Базовый профиль удалить нельзя".clone_into(&mut self.status);
            return;
        }
        if index < self.db.templates.len() {
            self.db.templates.remove(index);
            self.recompose(false);
            self.select_template(0);
            "Нозология удалена из текущей библиотеки; сохраните библиотеку на диск"
                .clone_into(&mut self.status);
        }
    }

    fn add_custom_investigation(&mut self) {
        let label = self.new_investigation.trim();
        if label.is_empty() {
            return;
        }
        let mut catalog = self.db.catalog.clone();
        for template in &self.db.templates {
            merge_investigations(&mut catalog, &template.catalog);
        }
        merge_investigations(&mut catalog, &self.draft.catalog);
        if let Some(existing) = catalog.iter()
            .find(|item| item.label.eq_ignore_ascii_case(label))
        {
            merge_investigations(&mut self.draft.catalog, std::slice::from_ref(existing));
            self.draft_dirty = true;
            if !self.draft.investigation_ids.contains(&existing.id) {
                self.draft.investigation_ids.push(existing.id.clone());
                self.draft_dirty = true;
            }
            self.new_investigation.clear();
            return;
        }
        let mut counter = catalog.len() + 1;
        let id = loop {
            let candidate = format!("custom_{counter}");
            if catalog.iter().all(|item| item.id != candidate) {
                break candidate;
            }
            counter += 1;
        };
        self.draft.catalog.push(Investigation {
            id: id.clone(),
            label: label.to_owned(),
        });
        self.draft.investigation_ids.push(id);
        self.new_investigation.clear();
        self.draft_dirty = true;
    }

    fn save_database(&mut self) {
        if self.storage_error {
            self.status = "Сохранение заблокировано: исправьте ошибку файла библиотеки и перезапустите редактор. Исходные данные не изменены.".to_owned();
            return;
        }
        ensure_base_template(&mut self.db);
        match save_database(&database_path(&self.root), &self.db) {
            Ok(()) => {
                self.saved_db = serialize_database(&self.db).unwrap_or_default();
                self.draft_dirty = false;
                self.status = format!(
                    "Библиотека сохранена: {}",
                    self.root.join(STORE_DIR).display()
                );
            }
            Err(error) => self.status = format!("Не удалось сохранить библиотеку: {error}"),
        }
    }
}

fn visit_section_ui(ui: &mut egui::Ui, title: &str, section: &mut EditableSection, rows: usize) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(title).strong());
            if section.modified() {
                ui.label(egui::RichText::new("ручная правка").weak());
            }
            if ui.small_button("Копировать").clicked() {
                ui.ctx().copy_text(section.text.clone());
            }
            if section.modified() && ui.small_button("↻ из шаблона").clicked() {
                section.text.clone_from(&section.generated);
            }
        });
        ui.add(
            egui::TextEdit::multiline(&mut section.text)
                .desired_rows(rows)
                .desired_width(f32::INFINITY),
        );
    });
    ui.add_space(4.0);
}

fn template_text_field(
    ui: &mut egui::Ui,
    title: &str,
    text: &mut String,
    dirty: &mut bool,
    rows: usize,
) {
    ui.label(egui::RichText::new(title).strong());
    if ui
        .add(
            egui::TextEdit::multiline(text)
                .desired_rows(rows)
                .desired_width(f32::INFINITY),
        )
        .changed()
    {
        *dirty = true;
    }
}

fn database_path(root: &Path) -> PathBuf {
    root.join(STORE_DIR).join(STORE_FILE)
}

include!("clinical_extender_storage.rs");

fn ensure_base_template(db: &mut ClinicalDatabase) {
    db.version = DATABASE_VERSION;
    if !db.templates.iter().any(|template| template.is_base) {
        let mut base = NosologyTemplate {
            is_base: true,
            code_pattern: "BASE".to_owned(),
            title: "Базовый терапевтический профиль".to_owned(),
            ..Default::default()
        };
        base.investigation_ids = vec![
            "cbc".to_owned(),
            "urinalysis".to_owned(),
            "biochemistry".to_owned(),
            "ecg".to_owned(),
        ];
        db.templates.insert(0, base);
    }
}

fn parse_diagnosis_input(input: &str) -> Vec<String> {
    input
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            let first = value.split_whitespace().next().unwrap_or(value);
            if looks_like_code(first) {
                first.to_uppercase()
            } else {
                value.to_uppercase()
            }
        })
        .collect()
}

fn looks_like_code(value: &str) -> bool {
    value.chars().any(|character| character.is_ascii_digit())
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '*' | '-')
        })
}

fn compose(db: &ClinicalDatabase, tokens: &[String]) -> (VisitDocument, Vec<String>, Vec<String>) {
    let mut matched_indices = db
        .templates
        .iter()
        .enumerate()
        .filter_map(|(index, template)| template.is_base.then_some((0_usize, index)))
        .collect::<Vec<_>>();
    let mut matched_labels = db
        .templates
        .iter()
        .filter(|template| template.is_base)
        .map(|template| template.title.clone())
        .collect::<Vec<_>>();
    let mut unknown = Vec::new();

    for token in tokens {
        let mut token_matches = db
            .templates
            .iter()
            .enumerate()
            .filter(|(_, template)| !template.is_base)
            .filter_map(|(index, template)| match_score(template, token).map(|score| (score, index)))
            .collect::<Vec<_>>();
        token_matches.sort_by_key(|(score, _)| *score);
        token_matches.dedup_by_key(|(_, index)| *index);
        if token_matches.is_empty() {
            unknown.push(token.clone());
            continue;
        }
        for (score, index) in token_matches {
            if matched_indices.iter().all(|(_, existing)| *existing != index) {
                matched_indices.push((score, index));
                if let Some(template) = db.templates.get(index) {
                    matched_labels.push(if template.code_pattern.is_empty() {
                        template.title.clone()
                    } else {
                        template.code_pattern.clone()
                    });
                }
            }
        }
    }

    matched_indices.sort_by_key(|(score, _)| *score);
    let templates = matched_indices
        .iter()
        .filter_map(|(_, index)| db.templates.get(*index))
        .collect::<Vec<_>>();

    let complaints = join_unique(templates.iter().map(|template| &template.sections.complaints));
    let disease_history = join_unique(
        templates
            .iter()
            .map(|template| &template.sections.disease_history),
    );
    let life_history = join_unique(templates.iter().map(|template| &template.sections.life_history));
    let past_diseases = join_unique(templates.iter().map(|template| &template.sections.past_diseases));
    let objective_status = join_unique(
        templates
            .iter()
            .map(|template| &template.sections.objective_status),
    );
    let treatment = join_unique(templates.iter().map(|template| &template.sections.treatment));
    let recommendations = join_unique_sentences(
        templates
            .iter()
            .map(|template| &template.sections.recommendations),
    );

    let mut shared_catalog = db.catalog.clone();
    for template in db.templates.iter().filter(|t| t.is_base) {
        merge_investigations(&mut shared_catalog, &template.catalog);
    }
    let mut seen_investigations = HashSet::new();
    let mut investigation_labels = Vec::new();
    for template in &templates {
        for id in &template.investigation_ids {
            if seen_investigations.insert(id.clone()) {
                investigation_labels.push(
                    template.catalog.iter().chain(shared_catalog.iter())
                        .find(|item| &item.id == id)
                        .map(|item| item.label.clone())
                        .unwrap_or_else(|| id.clone()),
                );
            }
        }
    }
    let free_plan = join_unique(
        templates
            .iter()
            .map(|template| &template.sections.examination_plan),
    );
    let structured_plan = investigation_labels.join(", ");
    let examination_plan = match (structured_plan.is_empty(), free_plan.is_empty()) {
        (true, true) => String::new(),
        (false, true) => structured_plan,
        (true, false) => free_plan,
        (false, false) => format!("{structured_plan}\n{free_plan}"),
    };

    (
        VisitDocument {
            complaints,
            disease_history,
            life_history,
            past_diseases,
            objective_status,
            examination_plan,
            treatment,
            recommendations,
        },
        matched_labels,
        unknown,
    )
}

fn join_unique<'a>(parts: impl Iterator<Item = &'a String>) -> String {
    let mut seen = HashSet::new();
    parts
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.to_lowercase()))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Merge recommendation fragments while removing repeated complete sentences.
///
/// This deliberately performs only conservative textual de-duplication:
/// case, repeated whitespace and terminal sentence punctuation are ignored,
/// but wording, dosage, frequency and other clinical content must still match.
/// The first occurrence wins, so template order and the clinician-authored
/// wording of that occurrence are preserved.
fn join_unique_sentences<'a>(parts: impl Iterator<Item = &'a String>) -> String {
    let mut seen = HashSet::new();
    let mut paragraphs = Vec::new();

    for part in parts {
        let mut unique_sentences = Vec::new();
        for sentence in split_recommendation_sentences(part) {
            let key = normalize_sentence_for_dedupe(&sentence);
            if !key.is_empty() && seen.insert(key) {
                unique_sentences.push(sentence);
            }
        }
        if !unique_sentences.is_empty() {
            paragraphs.push(unique_sentences.join(" "));
        }
    }

    paragraphs.join("\n\n")
}

/// Split on sentence-ending punctuation only when the following non-whitespace
/// character looks like the start of a new sentence. This avoids turning common
/// clinical abbreviations such as "мг. по 1 таблетке" into separate sentences.
fn split_recommendation_sentences(text: &str) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }

    let chars = text.char_indices().collect::<Vec<_>>();
    let mut result = Vec::new();
    let mut start = 0usize;

    for (position, &(byte_index, ch)) in chars.iter().enumerate() {
        if !matches!(ch, '.' | '!' | '?') {
            continue;
        }

        let end = byte_index + ch.len_utf8();
        let mut next_non_whitespace = None;
        let mut has_whitespace_after_punctuation = false;
        for &(_, next) in chars.iter().skip(position + 1) {
            if next.is_whitespace() {
                has_whitespace_after_punctuation = true;
                continue;
            }
            next_non_whitespace = Some(next);
            break;
        }

        let candidate = text[start..end].trim();
        let numeric_marker = candidate
            .trim_end_matches(|mark: char| matches!(mark, '.' | '!' | '?'))
            .chars()
            .all(|item| item.is_ascii_digit());

        let is_boundary = match next_non_whitespace {
            None => true,
            Some(next) => {
                has_whitespace_after_punctuation
                    && !numeric_marker
                    && (next.is_uppercase()
                        || next.is_numeric()
                        || matches!(next, '-' | '—' | '•' | '▪' | '◦'))
            }
        };

        if is_boundary {
            let sentence = text[start..end].trim();
            if !sentence.is_empty() {
                result.push(sentence.to_owned());
            }
            start = end;
            while start < text.len() {
                let Some(next) = text[start..].chars().next() else {
                    break;
                };
                if next.is_whitespace() {
                    start += next.len_utf8();
                } else {
                    break;
                }
            }
        }
    }

    if start < text.len() {
        let tail = text[start..].trim();
        if !tail.is_empty() {
            result.push(tail.to_owned());
        }
    }

    result
}

fn normalize_sentence_for_dedupe(sentence: &str) -> String {
    let trimmed = sentence
        .trim()
        .trim_start_matches(|ch: char| matches!(ch, '-' | '—' | '•' | '▪' | '◦'))
        .trim()
        .trim_end_matches(|ch: char| matches!(ch, '.' | '!' | '?' | ';' | ':'))
        .trim();

    let mut normalized = String::with_capacity(trimmed.len());
    let mut previous_was_whitespace = false;
    for ch in trimmed.chars().flat_map(char::to_lowercase) {
        if ch.is_whitespace() {
            if !previous_was_whitespace && !normalized.is_empty() {
                normalized.push(' ');
            }
            previous_was_whitespace = true;
        } else {
            normalized.push(ch);
            previous_was_whitespace = false;
        }
    }
    normalized.trim().to_owned()
}

fn match_score(template: &NosologyTemplate, token: &str) -> Option<usize> {
    if template
        .aliases
        .iter()
        .any(|alias| alias.trim().eq_ignore_ascii_case(token))
    {
        return Some(30_000);
    }
    let pattern = template.code_pattern.trim().to_uppercase();
    if pattern == token {
        return Some(20_000 + pattern.len());
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        if token.starts_with(prefix) {
            return Some(10_000 + prefix.len());
        }
    }
    None
}


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


#[cfg(test)]
mod tests {
    use super::*;


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

    #[test]
    fn recommendation_sentences_are_deduplicated_across_templates() {
        let mut db = ClinicalDatabase::default();
        db.templates[1].sections.recommendations =
            "Прием антигипертензивной терапии. Бисопролол 5 мг по 1 таблетке утром."
                .to_owned();
        db.templates[2].sections.recommendations =
            "Прием антиангинальной терапии. Бисопролол 5 мг по 1 таблетке утром."
                .to_owned();

        let tokens = parse_diagnosis_input("I11.9, I50.9");
        let (document, _, _) = compose(&db, &tokens);

        assert_eq!(
            document
                .recommendations
                .matches("Бисопролол 5 мг по 1 таблетке утром.")
                .count(),
            1
        );
        assert!(document
            .recommendations
            .contains("Прием антигипертензивной терапии."));
        assert!(document
            .recommendations
            .contains("Прием антиангинальной терапии."));
    }

    #[test]
    fn recommendation_sentence_normalization_is_conservative() {
        let merged = join_unique_sentences(
            [
                "Охранительный ортопедический режим. Бисопролол 5 мг. по 1 таблетке утром."
                    .to_owned(),
                "  охранительный   ортопедический режим!  Бисопролол 10 мг по 1 таблетке утром."
                    .to_owned(),
            ]
            .iter(),
        );

        assert_eq!(
            normalize_sentence_for_dedupe("Охранительный ортопедический режим."),
            normalize_sentence_for_dedupe(" охранительный   ортопедический режим! ")
        );
        assert_eq!(merged.matches("охранительный").count(), 0);
        assert_eq!(merged.matches("Охранительный ортопедический режим.").count(), 1);
        assert!(merged.contains("Бисопролол 5 мг. по 1 таблетке утром."));
        assert!(merged.contains("Бисопролол 10 мг по 1 таблетке утром."));
    }

    #[test]
    fn clinical_abbreviation_does_not_split_sentence() {
        assert_eq!(
            split_recommendation_sentences(
                "Бисопролол 5 мг. по 1 таблетке утром. Контроль ЧСС ежедневно."
            ),
            vec![
                "Бисопролол 5 мг. по 1 таблетке утром.".to_owned(),
                "Контроль ЧСС ежедневно.".to_owned(),
            ]
        );
    }

    #[test]
    fn decimal_dose_is_not_treated_as_sentence_boundary() {
        assert_eq!(
            split_recommendation_sentences(
                "Амлодипин 2.5 мг утром. Контроль АД ежедневно."
            ),
            vec![
                "Амлодипин 2.5 мг утром.".to_owned(),
                "Контроль АД ежедневно.".to_owned(),
            ]
        );
    }

    #[test]
    fn numbered_recommendation_stays_with_its_text() {
        assert_eq!(
            split_recommendation_sentences(
                "1. Контроль АД ежедневно. 2. Вести дневник давления."
            ),
            vec![
                "1. Контроль АД ежедневно.".to_owned(),
                "2. Вести дневник давления.".to_owned(),
            ]
        );
    }

    #[test]
    fn combines_and_deduplicates_investigations() {
        let db = ClinicalDatabase::default();
        let tokens = parse_diagnosis_input("I11.9, M42.1");
        let (document, _, unknown) = compose(&db, &tokens);
        assert!(unknown.is_empty());
        assert_eq!(document.examination_plan.matches("ОАК").count(), 1);
        assert!(document.examination_plan.contains("ЭхоКГ"));
        assert!(document.examination_plan.contains("МАУ"));
        assert!(document.examination_plan.contains("Рентгенография"));
    }

    #[test]
    fn alias_resolves_nosology() {
        let db = ClinicalDatabase::default();
        let tokens = parse_diagnosis_input("ХСН");
        let (document, matched, unknown) = compose(&db, &tokens);
        assert!(unknown.is_empty());
        assert!(matched.iter().any(|item| item == "I50.9"));
        assert!(document.examination_plan.contains("NT-proBNP"));
    }

    #[test]
    fn wildcard_and_exact_templates_are_composed() {
        let mut db = ClinicalDatabase::default();
        let mut wildcard = NosologyTemplate {
            code_pattern: "I11.*".to_owned(),
            title: "Общий блок I11".to_owned(),
            ..Default::default()
        };
        wildcard.sections.complaints = "Общий фрагмент".to_owned();
        db.templates.push(wildcard);
        let tokens = parse_diagnosis_input("I11.9");
        let (document, _, _) = compose(&db, &tokens);
        assert!(document.complaints.contains("Общий фрагмент"));
        assert!(document.disease_history.contains("Артериальной гипертензией"));
    }

    #[test]
    fn manual_text_survives_non_forced_refresh() {
        let mut section = EditableSection::default();
        section.replace("Шаблон".to_owned());
        section.text.push_str(" + ручная правка");
        section.update_preserving_manual("Новый шаблон".to_owned());
        assert_eq!(section.text, "Шаблон + ручная правка");
    }
}

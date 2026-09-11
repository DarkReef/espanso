use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

const STORE_DIR: &str = "clinical_extender";
const STORE_FILE: &str = "nosologies.yml";
const DATABASE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClinicalTab {
    Visit,
    Nosologies,
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
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ClinicalDatabase {
    pub version: u32,
    pub catalog: Vec<Investigation>,
    pub templates: Vec<NosologyTemplate>,
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
        }
    }
}

fn investigation(id: &str, label: &str) -> Investigation {
    Investigation {
        id: id.to_owned(),
        label: label.to_owned(),
    }
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
    status: String,
}

impl ClinicalExtender {
    pub fn load(root: PathBuf) -> Self {
        let path = database_path(&root);
        let (mut db, status) = match load_database(&path) {
            Ok(Some(db)) => (db, format!("Библиотека загружена: {}", path.display())),
            Ok(None) => (
                ClinicalDatabase::default(),
                "Создана стартовая локальная библиотека нозологий".to_owned(),
            ),
            Err(error) => (
                ClinicalDatabase::default(),
                format!("Не удалось прочитать библиотеку; загружены стартовые шаблоны: {error}"),
            ),
        };
        ensure_base_template(&mut db);
        let saved_db = serialize_database(&db).unwrap_or_default();
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
            status,
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
            let catalog = self.db.catalog.clone();
            for investigation in catalog {
                let mut checked = self
                    .draft
                    .investigation_ids
                    .iter()
                    .any(|id| id == &investigation.id);
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
                self.save_database();
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

    fn recompose(&mut self, force: bool) {
        let tokens = parse_diagnosis_input(&self.diagnosis_input);
        let (document, matched, unknown) = compose(&self.db, &tokens);
        self.visit.refresh(document, force);
        self.matched = matched;
        self.unknown = unknown;
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
        if let Some(existing) = self
            .db
            .catalog
            .iter()
            .find(|item| item.label.eq_ignore_ascii_case(label))
        {
            if !self.draft.investigation_ids.contains(&existing.id) {
                self.draft.investigation_ids.push(existing.id.clone());
                self.draft_dirty = true;
            }
            self.new_investigation.clear();
            return;
        }
        let mut counter = self.db.catalog.len() + 1;
        let id = loop {
            let candidate = format!("custom_{counter}");
            if self.db.catalog.iter().all(|item| item.id != candidate) {
                break candidate;
            }
            counter += 1;
        };
        self.db.catalog.push(Investigation {
            id: id.clone(),
            label: label.to_owned(),
        });
        self.draft.investigation_ids.push(id);
        self.new_investigation.clear();
        self.draft_dirty = true;
    }

    fn save_database(&mut self) {
        ensure_base_template(&mut self.db);
        match save_database(&database_path(&self.root), &self.db) {
            Ok(()) => {
                self.saved_db = serialize_database(&self.db).unwrap_or_default();
                self.draft_dirty = false;
                self.status = format!(
                    "Библиотека сохранена: {}",
                    database_path(&self.root).display()
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

fn load_database(path: &Path) -> Result<Option<ClinicalDatabase>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    serde_norway::from_str(&content)
        .map(Some)
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn serialize_database(db: &ClinicalDatabase) -> Result<String, String> {
    serde_norway::to_string(db).map_err(|error| error.to_string())
}

fn save_database(path: &Path, db: &ClinicalDatabase) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Некорректный путь библиотеки".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let content = serialize_database(db)?;
    let temp = path.with_extension("yml.tmp");
    fs::write(&temp, content).map_err(|error| error.to_string())?;
    fs::rename(&temp, path).map_err(|error| error.to_string())
}

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
    let recommendations = join_unique(
        templates
            .iter()
            .map(|template| &template.sections.recommendations),
    );

    let mut seen_investigations = HashSet::new();
    let mut investigation_labels = Vec::new();
    for template in &templates {
        for id in &template.investigation_ids {
            if seen_investigations.insert(id.clone()) {
                investigation_labels.push(
                    db.catalog
                        .iter()
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

#[cfg(test)]
mod tests {
    use super::*;

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

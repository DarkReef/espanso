use crate::clinical_template_engine::{
    example_input_json, example_package_json, TemplatePackage,
};
use eframe::egui;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct ClinicalTemplateWorkspace {
    path: String,
    template_json: String,
    original_json: String,
    input_json: String,
    result_json: String,
    status: String,
}

impl ClinicalTemplateWorkspace {
    pub fn new(config_root: &Path) -> Self {
        let path = config_root
            .join("clinical_extender")
            .join("template-engine.json");
        let path_text = path.to_string_lossy().to_string();
        let template_json = fs::read_to_string(&path).unwrap_or_else(|_| example_package_json());
        let input_json = example_input_json();
        Self {
            path: path_text,
            original_json: template_json.clone(),
            template_json,
            input_json,
            result_json: String::new(),
            status: "JSON Engine готов. Активный пакет не применяется без явной проверки и сохранения."
                .to_owned(),
        }
    }

    pub fn dirty(&self) -> bool {
        self.template_json != self.original_json
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let viewport_height = ui.available_height().max(120.0);
        egui::ScrollArea::vertical()
            .id_salt("clinical_template_engine_page")
            .max_height(viewport_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.heading("Clinical Template Engine · JSON / Rules");
                ui.label(
                    "Пакет описывает именованные поля, вычисления, зависимости, правила, секции и источники. JSON сначала валидируется; движок не исполняет файловые или сетевые функции.",
                );

                ui.horizontal_wrapped(|ui| {
                    ui.label("Файл:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.path)
                            .desired_width(520.0)
                            .hint_text("clinical_extender/template-engine.json"),
                    );
                    if ui.button("Загрузить JSON").clicked() {
                        self.load_file();
                    }
                    if ui.button("Проверить").clicked() {
                        self.validate_current();
                    }
                    if ui.button("Форматировать").clicked() {
                        self.format_current();
                    }
                    if ui.button("Сохранить проверенный").clicked() {
                        self.save_current();
                    }
                });

                if self.dirty() {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        "JSON изменён и ещё не сохранён",
                    );
                }
                ui.label(egui::RichText::new(&self.status).weak());
                ui.separator();

                ui.label(egui::RichText::new("Template Package JSON").strong());
                ui.add(
                    egui::TextEdit::multiline(&mut self.template_json)
                        .code_editor()
                        .desired_rows(24)
                        .desired_width(f32::INFINITY),
                );

                ui.separator();
                ui.label(egui::RichText::new("Тестовый контекст пациента").strong());
                ui.label(
                    egui::RichText::new(
                        "Контекст допускается как вложенный JSON; движок преобразует его в patient.age, labs.creatinine_umol_l и т. п.",
                    )
                    .weak(),
                );
                ui.add(
                    egui::TextEdit::multiline(&mut self.input_json)
                        .code_editor()
                        .desired_rows(10)
                        .desired_width(f32::INFINITY),
                );

                ui.horizontal_wrapped(|ui| {
                    if ui.button("Вычислить").clicked() {
                        self.evaluate_current();
                    }
                    if ui.button("Копировать результат").clicked() {
                        ui.ctx().copy_text(self.result_json.clone());
                    }
                    if ui.button("Вернуть пример").clicked() {
                        self.template_json = example_package_json();
                        self.input_json = example_input_json();
                        self.result_json.clear();
                        self.status = "Загружен встроенный пример; он не является клиническим нормативом."
                            .to_owned();
                    }
                });

                if !self.result_json.is_empty() {
                    ui.label(egui::RichText::new("Результат evaluation API").strong());
                    ui.add(
                        egui::TextEdit::multiline(&mut self.result_json)
                            .code_editor()
                            .desired_rows(18)
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
                }
            });
    }

    fn load_file(&mut self) {
        let path = PathBuf::from(self.path.trim());
        match fs::read_to_string(&path) {
            Ok(content) => match TemplatePackage::from_json(&content) {
                Ok(_) => {
                    self.template_json = content;
                    self.original_json = self.template_json.clone();
                    self.result_json.clear();
                    self.status = format!("JSON загружен и валиден: {}", path.display());
                }
                Err(error) => {
                    self.status = format!(
                        "Файл прочитан, но не активирован из-за ошибки валидации: {error}"
                    );
                }
            },
            Err(error) => {
                self.status = format!("Не удалось прочитать {}: {error}", path.display());
            }
        }
    }

    fn validate_current(&mut self) {
        match TemplatePackage::from_json(&self.template_json) {
            Ok(package) => {
                self.status = format!(
                    "Пакет валиден: {} {} · полей {} · вычислений {} · правил {}",
                    package.package.name,
                    package.package.version,
                    package.fields.len(),
                    package.calculations.len(),
                    package.rules.len()
                );
            }
            Err(error) => self.status = format!("Ошибка проверки: {error}"),
        }
    }

    fn format_current(&mut self) {
        match TemplatePackage::from_json(&self.template_json)
            .and_then(|package| package.to_pretty_json())
        {
            Ok(formatted) => {
                self.template_json = formatted;
                self.status = "JSON отформатирован после успешной валидации".to_owned();
            }
            Err(error) => self.status = format!("Нельзя форматировать: {error}"),
        }
    }

    fn save_current(&mut self) {
        let package = match TemplatePackage::from_json(&self.template_json) {
            Ok(package) => package,
            Err(error) => {
                self.status = format!("Сохранение отменено: {error}");
                return;
            }
        };
        let content = match package.to_pretty_json() {
            Ok(content) => content,
            Err(error) => {
                self.status = format!("Сохранение отменено: {error}");
                return;
            }
        };
        let path = PathBuf::from(self.path.trim());
        if let Some(parent) = path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                self.status = format!("Не удалось создать {}: {error}", parent.display());
                return;
            }
        }
        match fs::write(&path, &content) {
            Ok(()) => {
                self.template_json = content.clone();
                self.original_json = content;
                self.status = format!("Проверенный пакет сохранён: {}", path.display());
            }
            Err(error) => {
                self.status = format!("Не удалось сохранить {}: {error}", path.display());
            }
        }
    }

    fn evaluate_current(&mut self) {
        let package = match TemplatePackage::from_json(&self.template_json) {
            Ok(package) => package,
            Err(error) => {
                self.status = format!("Вычисление отменено: {error}");
                return;
            }
        };
        let input: Value = match serde_json::from_str(&self.input_json) {
            Ok(input) => input,
            Err(error) => {
                self.status = format!("Ошибка тестового контекста JSON: {error}");
                return;
            }
        };
        let result = package.evaluate(&input);
        match serde_json::to_string_pretty(&result) {
            Ok(json) => {
                let error_count = result.errors.len();
                self.result_json = json;
                self.status = if error_count == 0 {
                    format!(
                        "Вычисление завершено: сработало правил {}",
                        result.activated_rules.len()
                    )
                } else {
                    format!(
                        "Вычисление завершено с диагностикой: ошибок {}",
                        error_count
                    )
                };
            }
            Err(error) => self.status = format!("Не удалось сериализовать результат: {error}"),
        }
    }
}


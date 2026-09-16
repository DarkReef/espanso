include!("clinical_extender_core.rs");
include!("clinical_extender_seed.rs");

impl ClinicalExtender {
    pub fn ui_localized(&mut self, ui: &mut egui::Ui) {
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
            ClinicalTab::Visit => self.visit_ui_localized(ui),
            ClinicalTab::Nosologies => self.nosologies_ui(ui),
        }
    }

    fn visit_ui_localized(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Клинический редактор · конструктор осмотра");
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
}

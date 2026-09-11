#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellTab {
    Rules,
    Settings,
    Rhai,
    Clinical,
    Ai,
}

struct StudioShell {
    studio: MatchStudioApp,
    clinical: crate::clinical_extender::ClinicalExtender,
    active_tab: ShellTab,
}

impl StudioShell {
    fn new(config_root: PathBuf) -> Self {
        Self {
            studio: MatchStudioApp::new(config_root.clone()),
            clinical: crate::clinical_extender::ClinicalExtender::load_seeded(config_root),
            active_tab: ShellTab::Rules,
        }
    }

    fn sync_legacy_tab(&mut self) {
        self.studio.active_tab = match self.active_tab {
            ShellTab::Rules => MainTab::Rules,
            ShellTab::Settings => MainTab::Settings,
            ShellTab::Rhai => MainTab::Rhai,
            ShellTab::Ai => MainTab::Ai,
            ShellTab::Clinical => return,
        };
    }

    fn top_bar(&mut self, root: &mut egui::Ui) {
        egui::Panel::top("toolbar").show(root, |ui| {
            ui.horizontal(|ui| {
                storm_logo::show(ui);
                ui.heading(APP_TITLE);
                ui.separator();

                let (runtime_color, runtime_text) = if self.studio.runtime.running() {
                    (
                        egui::Color32::from_rgb(40, 160, 90),
                        format!(
                            "rEspanso запущен · {} проц.",
                            self.studio.runtime.process_ids().len()
                        ),
                    )
                } else {
                    (
                        egui::Color32::from_rgb(190, 70, 70),
                        "rEspanso не запущен".to_owned(),
                    )
                };
                ui.colored_label(runtime_color, runtime_text).on_hover_text(format!(
                    "Проверяется каждую секунду. Последнее изменение состояния: {} сек. назад. PID: {}",
                    self.studio.runtime.seconds_since_change(),
                    self.studio
                        .runtime
                        .process_ids()
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            });

            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.active_tab, ShellTab::Rules, "Правила");
                ui.selectable_value(
                    &mut self.active_tab,
                    ShellTab::Settings,
                    "Настройки rEspanso",
                );
                ui.selectable_value(&mut self.active_tab, ShellTab::Rhai, "Rhai");
                ui.selectable_value(
                    &mut self.active_tab,
                    ShellTab::Clinical,
                    "Clinical Extender",
                );
                ui.selectable_value(&mut self.active_tab, ShellTab::Ai, "ИИ / MCP");
                ui.separator();

                if ui
                    .button("Импорт / экспорт")
                    .on_hover_text("Перенос match и scripts единым проверяемым пакетом")
                    .clicked()
                {
                    self.studio.show_config_transfer = true;
                    self.studio.refresh_config_packages();
                }
                ui.separator();

                match self.active_tab {
                    ShellTab::Clinical => {
                        if self.clinical.dirty() {
                            ui.colored_label(
                                egui::Color32::from_rgb(210, 135, 25),
                                "Библиотека Clinical Extender не сохранена",
                            );
                        } else {
                            ui.label(
                                egui::RichText::new("Локальная клиническая библиотека").weak(),
                            );
                        }
                    }
                    ShellTab::Ai => {}
                    ShellTab::Rules => {
                        if ui
                            .button("Новое правило")
                            .on_hover_text("Ctrl+N. Добавляет правило в выбранный YAML-файл")
                            .clicked()
                        {
                            self.studio.create_rule();
                        }
                        if ui
                            .button("Переменные")
                            .on_hover_text("Просмотр, создание и изменение общих переменных")
                            .clicked()
                        {
                            self.studio.show_global_variables = true;
                        }
                        if ui
                            .button("Сохранить всё")
                            .on_hover_text(
                                "Ctrl+S. Записывает изменения и создаёт резервные копии",
                            )
                            .clicked()
                        {
                            self.studio.save_all();
                        }
                        if ui
                            .button("Обновить")
                            .on_hover_text("Ctrl+R. Перечитывает YAML-файлы с диска")
                            .clicked()
                        {
                            self.studio.request_reload();
                        }
                        let has_selection = self.studio.selected.is_some();
                        if ui
                            .add_enabled(has_selection, egui::Button::new("Дублировать"))
                            .on_hover_text("Ctrl+D")
                            .clicked()
                        {
                            self.studio.duplicate_selected();
                        }
                        if ui
                            .add_enabled(has_selection, egui::Button::new("Удалить"))
                            .on_hover_text("Ctrl+Shift+D")
                            .clicked()
                        {
                            self.studio.confirm_delete = true;
                        }
                        ui.separator();
                        ui.checkbox(&mut self.studio.show_diagnostics, "Диагностика")
                            .on_hover_text(
                                "Ctrl+L. Показать или скрыть устойчивые экземпляры ошибок",
                            );
                        if let Some(workspace) = &self.studio.workspace {
                            let dirty = workspace.dirty_files().len();
                            if dirty > 0 {
                                ui.separator();
                                ui.colored_label(
                                    egui::Color32::from_rgb(210, 135, 25),
                                    format!("Не сохранено файлов: {dirty}"),
                                );
                            }
                        }
                    }
                    ShellTab::Settings => {
                        if ui
                            .button("Сохранить настройки")
                            .on_hover_text("Ctrl+S")
                            .clicked()
                        {
                            self.studio.save_settings();
                        }
                        if ui
                            .button("Обновить настройки")
                            .on_hover_text("Ctrl+R")
                            .clicked()
                        {
                            self.studio.reload_settings();
                        }
                        if self.studio.settings.dirty() {
                            ui.colored_label(
                                egui::Color32::from_rgb(210, 135, 25),
                                "Настройки не сохранены",
                            );
                        }
                    }
                    ShellTab::Rhai => {
                        if ui.button("Новый скрипт").on_hover_text("Ctrl+N").clicked() {
                            let result = self.studio.rhai_lab.start_new_script();
                            self.studio.report_rhai_action(result);
                        }
                        if ui.button("Сохранить").on_hover_text("Ctrl+S").clicked() {
                            self.studio.save_rhai_current();
                        }
                        if ui
                            .button("Скомпилировать")
                            .on_hover_text("Ctrl+Shift+Enter")
                            .clicked()
                        {
                            self.studio.rhai_lab.compile_current();
                        }
                        if ui
                            .button("Запустить")
                            .on_hover_text("Ctrl+Enter")
                            .clicked()
                        {
                            self.studio.rhai_lab.run_current();
                        }
                        if self.studio.rhai_lab.dirty() {
                            ui.colored_label(
                                egui::Color32::from_rgb(210, 135, 25),
                                "Скрипт не сохранён",
                            );
                        }
                    }
                }

                ui.separator();
                let restart_message = self.studio.runtime.restart_button(ui);
                if ui.button("Горячие клавиши").on_hover_text("F1").clicked() {
                    self.studio.show_shortcuts = true;
                }
                if let Some(message) = restart_message {
                    self.studio.status = message;
                }
            });
        });
    }

    fn status_bar(&mut self, root: &mut egui::Ui) {
        egui::Panel::bottom("status").show(root, |ui| {
            ui.horizontal(|ui| {
                if self.active_tab == ShellTab::Clinical {
                    ui.label(if self.clinical.dirty() {
                        "Clinical Extender: есть несохранённые изменения библиотеки"
                    } else {
                        "Clinical Extender: готов к работе"
                    });
                } else {
                    ui.label(self.studio.status.as_str());
                }
                ui.separator();
                ui.label(
                    egui::RichText::new(format!(
                        "Конфигурация: {}",
                        self.studio.config_root.display()
                    ))
                    .weak(),
                );
                if self.studio.external_change_pending {
                    ui.separator();
                    if ui.button("Обновить внешние изменения").clicked() {
                        self.studio.request_external_reload();
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
    }
}

impl eframe::App for StudioShell {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.studio.runtime.update(ui.ctx());
        self.studio.handle_dropped_config_packages(ui.ctx());
        self.studio.check_external_file_changes(ui.ctx());

        if self.active_tab != ShellTab::Clinical {
            self.sync_legacy_tab();
            self.studio.handle_shortcuts(ui.ctx());
        }

        self.top_bar(ui);
        self.status_bar(ui);

        match self.active_tab {
            ShellTab::Clinical => self.clinical.ui(ui),
            ShellTab::Ai => self.studio.ai.ui(ui),
            ShellTab::Rules => {
                self.studio.rules_panel(ui);
                self.studio.diagnostics_panel(ui);
                self.studio.central_editor(ui);
            }
            ShellTab::Settings => {
                let config_root = self.studio.config_root.clone();
                self.studio
                    .settings
                    .ui(ui, &config_root, &mut self.studio.status);
            }
            ShellTab::Rhai => {
                self.studio.rhai_lab.ui(ui, &mut self.studio.status);
            }
        }
        self.studio.dialogs(ui.ctx());
    }
}

pub fn run_shell(config_root: PathBuf) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            .with_inner_size([1500.0, 900.0])
            .with_min_inner_size([800.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        APP_TITLE,
        options,
        Box::new(move |_creation_context| Ok(Box::new(StudioShell::new(config_root)))),
    )
}

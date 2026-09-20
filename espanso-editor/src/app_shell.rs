#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellTab {
    Rules,
    Settings,
    Rhai,
    Clinical,
    Templates,
    Ai,
}

fn respanso_build_version() -> &'static str {
    option_env!("RESPANSO_VERSION").unwrap_or("2.4.0")
}

fn respanso_build_date() -> &'static str {
    option_env!("RESPANSO_BUILD_DATE").unwrap_or("локальная сборка")
}

fn respanso_build_sha() -> &'static str {
    option_env!("RESPANSO_BUILD_SHA").unwrap_or("dev")
}

struct StudioShell {
    studio: MatchStudioApp,
    clinical: crate::clinical_extender::ClinicalExtender,
    template_engine: crate::clinical_template_engine::ClinicalTemplateWorkspace,
    active_tab: ShellTab,
    theme: crate::theme::StudioTheme,
}

impl StudioShell {
    fn new(config_root: PathBuf) -> Self {
        let theme = crate::theme::StudioTheme::load(&config_root);
        Self {
            studio: MatchStudioApp::new(config_root.clone()),
            clinical: crate::clinical_extender::ClinicalExtender::load_seeded(config_root.clone()),
            template_engine: crate::clinical_template_engine::ClinicalTemplateWorkspace::new(&config_root),
            active_tab: ShellTab::Rules,
            theme,
        }
    }

    fn sync_legacy_tab(&mut self) {
        self.studio.active_tab = match self.active_tab {
            ShellTab::Rules => MainTab::Rules,
            ShellTab::Settings => MainTab::Settings,
            ShellTab::Rhai => MainTab::Rhai,
            ShellTab::Ai => MainTab::Ai,
            ShellTab::Clinical | ShellTab::Templates => return,
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
                        self.theme.success(),
                        format!(
                            "rEspanso запущен · {} проц.",
                            self.studio.runtime.process_ids().len()
                        ),
                    )
                } else {
                    (self.theme.error(), "rEspanso не запущен".to_owned())
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

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    for theme in [
                        crate::theme::StudioTheme::Dark,
                        crate::theme::StudioTheme::Light,
                    ] {
                        if ui
                            .selectable_label(self.theme == theme, theme.label())
                            .on_hover_text(format!(
                                "Переключить Match Studio: {} тема",
                                theme.label().to_lowercase()
                            ))
                            .clicked()
                        {
                            self.theme = theme;
                            self.theme.apply_to_ui(ui);
                            ui.ctx().request_repaint();
                            self.studio.status = match self.theme.save(&self.studio.config_root) {
                                Ok(()) => format!("Тема Match Studio: {}", self.theme.label()),
                                Err(error) => error,
                            };
                        }
                    }
                    ui.label("Тема:");
                });
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
                    "Клинический редактор",
                );
                ui.selectable_value(
                    &mut self.active_tab,
                    ShellTab::Templates,
                    "Template Engine",
                );
                ui.selectable_value(&mut self.active_tab, ShellTab::Ai, "Вспомогательная");
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
                                self.theme.warning(),
                                "Библиотека клинического редактора не сохранена",
                            );
                        } else {
                            ui.label(
                                egui::RichText::new("Локальная клиническая библиотека").weak(),
                            );
                        }
                    }
                    ShellTab::Templates => {
                        if self.template_engine.dirty() {
                            ui.colored_label(self.theme.warning(), "JSON-пакет не сохранён");
                        } else {
                            ui.label(egui::RichText::new("JSON / Rules Engine").weak());
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
                                    self.theme.warning(),
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
                            ui.colored_label(self.theme.warning(), "Настройки не сохранены");
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
                            ui.colored_label(self.theme.warning(), "Скрипт не сохранён");
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
                        "Клинический редактор: есть несохранённые изменения библиотеки"
                    } else {
                        "Клинический редактор: готов к работе"
                    });
                } else if self.active_tab == ShellTab::Templates {
                    ui.label(self.template_engine.status());
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

    fn help_dialog(&mut self, context: &egui::Context) {
        if !self.studio.show_shortcuts {
            return;
        }

        let mut open = self.studio.show_shortcuts;
        egui::Window::new("Справка · горячие клавиши")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(context, |ui| {
                ui.heading("rEspanso Match Studio");
                ui.label(format!(
                    "rEspanso {} · сборка {}",
                    respanso_build_version(),
                    respanso_build_date()
                ));
                let sha = respanso_build_sha();
                if sha != "dev" {
                    ui.label(egui::RichText::new(format!("Commit: {sha}")).weak());
                }
                ui.separator();
                egui::Grid::new("shell_shortcut_grid")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        shortcut_row(ui, "Alt+L", "Обработать выделенный текст во «Вспомогательной»");
                        shortcut_row(ui, "Ctrl+S", "Сохранить изменения активной вкладки");
                        shortcut_row(ui, "Ctrl+N", "Создать правило или новый Rhai-скрипт");
                        shortcut_row(
                            ui,
                            "Ctrl+Enter",
                            "Проверить исходный YAML или запустить Rhai-скрипт",
                        );
                        shortcut_row(ui, "Ctrl+Alt+M", "Найти триггер по выделенному тексту");
                        shortcut_row(ui, "Ctrl+F", "Перейти к поиску правил");
                        shortcut_row(ui, "Ctrl+D", "Дублировать правило");
                        shortcut_row(ui, "Ctrl+Shift+D", "Удалить правило");
                        shortcut_row(
                            ui,
                            "Ctrl+R",
                            "Обновить активный YAML или Rhai-файл с диска",
                        );
                        shortcut_row(ui, "Ctrl+L", "Показать или скрыть диагностику");
                        shortcut_row(ui, "Ctrl+Shift+Enter", "Скомпилировать Rhai-скрипт");
                        shortcut_row(ui, "F1", "Открыть эту справку");
                    });
                ui.separator();
                ui.label(
                    egui::RichText::new(
                        "Astra/X11: клавиатура отслеживается через XQueryKeymap; занятые глобальные сочетания работают через polling-fallback.",
                    )
                    .weak(),
                );
            });
        self.studio.show_shortcuts = open;
    }

    fn dialogs(&mut self, context: &egui::Context) {
        // app_legacy also contains the old shortcuts window. Hide only that
        // flag while it renders its other dialogs, then draw the current help
        // window with build identity here.
        let help_open = self.studio.show_shortcuts;
        self.studio.show_shortcuts = false;
        self.studio.dialogs(context);
        self.studio.show_shortcuts = help_open;
        self.help_dialog(context);
    }
}

impl eframe::App for StudioShell {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Apply the palette to the current root Ui, not only Context. This
        // prevents a one-frame (and on some X11 drivers persistent) mixture of
        // old panel colors with newly themed TextEdit/widget colors.
        self.theme.apply_to_ui(ui);
        self.studio.runtime.update(ui.ctx());
        self.studio.handle_dropped_config_packages(ui.ctx());

        // Agent edits should become visible promptly. The legacy Studio monitor
        // already resolves dirty-buffer conflicts and waits for stable file content;
        // cap its next poll at 500 ms instead of waiting for the normal 3 s cycle.
        let fast_deadline = Instant::now() + Duration::from_millis(500);
        if self.studio.next_file_check > fast_deadline {
            self.studio.next_file_check = fast_deadline;
        }
        ui.ctx().request_repaint_after(Duration::from_millis(500));
        self.studio.check_external_file_changes(ui.ctx());

        if !matches!(self.active_tab, ShellTab::Clinical | ShellTab::Templates) {
            self.sync_legacy_tab();
            self.studio.handle_shortcuts(ui.ctx());
        }

        self.top_bar(ui);
        // The theme may have changed inside the toolbar. Re-apply it to the
        // root Ui before drawing status/body so the rest of this frame is
        // internally consistent.
        self.theme.apply_to_ui(ui);
        self.status_bar(ui);
        self.theme.apply_to_ui(ui);

        // Bound the page to the remaining native-window viewport. Long child
        // forms (notably AI/MCP and clinical editors) then receive a finite
        // available height and their ScrollAreas can actually scroll instead
        // of extending below the X11 window.
        let body_size = ui.available_size();
        // allocate_ui_with_layout does not paint a background. On X11 that made
        // the native window clear color (black) visible through the central page,
        // even while light-theme controls correctly remained white. Paint the
        // entire remaining viewport explicitly from the active theme before any
        // child content is added.
        let body_rect = ui.available_rect_before_wrap();
        ui.painter()
            .rect_filled(body_rect, 0.0, ui.visuals().panel_fill);
        ui.allocate_ui_with_layout(
            body_size,
            egui::Layout::top_down(egui::Align::Min),
            |body| match self.active_tab {
                ShellTab::Clinical => self.clinical.ui_localized(body),
                ShellTab::Templates => self.template_engine.ui(body),
                ShellTab::Ai => self.studio.ai.ui(body),
                ShellTab::Rules => {
                    self.studio.rules_panel(body);
                    self.studio.diagnostics_panel(body);
                    self.studio.central_editor(body);
                }
                ShellTab::Settings => {
                    let config_root = self.studio.config_root.clone();
                    self.studio
                        .settings
                        .ui(body, &config_root, &mut self.studio.status);
                }
                ShellTab::Rhai => {
                    self.studio.rhai_lab.ui(body, &mut self.studio.status);
                }
            },
        );
        self.dialogs(ui.ctx());
    }
}

pub fn run_shell(config_root: PathBuf) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            .with_inner_size([1500.0, 900.0])
            .with_min_inner_size([800.0, 420.0])
            .with_resizable(true),
        ..Default::default()
    };
    eframe::run_native(
        APP_TITLE,
        options,
        Box::new(move |creation_context| {
            let shell = StudioShell::new(config_root);
            shell.theme.apply(&creation_context.egui_ctx);
            Ok(Box::new(shell))
        }),
    )
}

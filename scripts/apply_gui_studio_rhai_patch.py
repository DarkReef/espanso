from pathlib import Path

APP = Path("espanso-editor/src/app.rs")

app = APP.read_text(encoding="utf-8")

if "rhai_lab::RhaiLab" not in app:
    app = app.replace(
        "use crate::{\n    runtime::RuntimeMonitor,",
        "use crate::{\n    rhai_lab::RhaiLab,\n    runtime::RuntimeMonitor,",
        1,
    )

app = app.replace(
    "enum MainTab {\n    Rules,\n    Settings,\n}",
    "enum MainTab {\n    Rules,\n    Settings,\n    Rhai,\n}",
    1,
)
app = app.replace(
    "    settings: SettingsEditor,\n    filter: String,",
    "    settings: SettingsEditor,\n    rhai_lab: RhaiLab,\n    filter: String,",
    1,
)
app = app.replace(
    "        let settings = SettingsEditor::load(&config_root);\n        Self {",
    "        let settings = SettingsEditor::load(&config_root);\n        let rhai_lab = RhaiLab::new();\n        Self {",
    1,
)
app = app.replace(
    "            settings,\n            filter: String::new(),",
    "            settings,\n            rhai_lab,\n            filter: String::new(),",
    1,
)

settings_shortcut = """        if self.active_tab == MainTab::Settings {
"""
rhai_shortcut = """        if self.active_tab == MainTab::Rhai {
            let (run, compile, help) = context.input(|input| {
                let primary = input.modifiers.ctrl || input.modifiers.command;
                (
                    primary
                        && !input.modifiers.shift
                        && input.key_pressed(egui::Key::Enter),
                    primary
                        && input.modifiers.shift
                        && input.key_pressed(egui::Key::Enter),
                    input.key_pressed(egui::Key::F1),
                )
            });
            if compile {
                self.rhai_lab.compile_current();
            }
            if run {
                self.rhai_lab.run_current();
            }
            if help {
                self.show_shortcuts = true;
            }
            return;
        }

        if self.active_tab == MainTab::Settings {
"""
if "if self.active_tab == MainTab::Rhai" not in app:
    app = app.replace(settings_shortcut, rhai_shortcut, 1)

start = app.index("    fn top_bar(&mut self, root: &mut egui::Ui) {")
end = app.index("    fn rules_panel(&mut self, root: &mut egui::Ui) {", start)
new_top_bar = r'''    fn top_bar(&mut self, root: &mut egui::Ui) {
        egui::Panel::top("toolbar").show(root, |ui| {
            ui.horizontal(|ui| {
                ui.heading(APP_TITLE);
                ui.separator();

                let (runtime_color, runtime_text) = if self.runtime.running() {
                    (
                        egui::Color32::from_rgb(40, 160, 90),
                        format!("rEspanso запущен · {} проц.", self.runtime.process_ids().len()),
                    )
                } else {
                    (
                        egui::Color32::from_rgb(190, 70, 70),
                        "rEspanso не запущен".to_owned(),
                    )
                };
                ui.colored_label(runtime_color, runtime_text).on_hover_text(format!(
                    "Проверяется каждую секунду. Последнее изменение состояния: {} сек. назад. PID: {}",
                    self.runtime.seconds_since_change(),
                    self.runtime
                        .process_ids()
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ));

                let right_width = ui.available_width();
                let restart_message = ui
                    .allocate_ui_with_layout(
                        egui::vec2(right_width, 30.0),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            let restart_message = self.runtime.restart_button(ui);
                            if ui.button("Горячие клавиши").on_hover_text("F1").clicked() {
                                self.show_shortcuts = true;
                            }
                            restart_message
                        },
                    )
                    .inner;
                if let Some(message) = restart_message {
                    self.status = message;
                }
            });

            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.active_tab, MainTab::Rules, "Правила");
                ui.selectable_value(
                    &mut self.active_tab,
                    MainTab::Settings,
                    "Настройки rEspanso",
                );
                ui.selectable_value(&mut self.active_tab, MainTab::Rhai, "Rhai");
                ui.separator();

                match self.active_tab {
                    MainTab::Rules => {
                        if ui
                            .button("Новое правило")
                            .on_hover_text("Ctrl+N. Добавляет правило в выбранный YAML-файл")
                            .clicked()
                        {
                            self.create_rule();
                        }
                        if ui
                            .button("Сохранить всё")
                            .on_hover_text(
                                "Ctrl+S. Записывает изменения и создаёт резервные копии",
                            )
                            .clicked()
                        {
                            self.save_all();
                        }
                        if ui
                            .button("Обновить")
                            .on_hover_text("Ctrl+R. Перечитывает YAML-файлы с диска")
                            .clicked()
                        {
                            self.request_reload();
                        }
                        let has_selection = self.selected.is_some();
                        if ui
                            .add_enabled(has_selection, egui::Button::new("Дублировать"))
                            .on_hover_text("Ctrl+D")
                            .clicked()
                        {
                            self.duplicate_selected();
                        }
                        if ui
                            .add_enabled(has_selection, egui::Button::new("Удалить"))
                            .on_hover_text("Ctrl+Shift+D")
                            .clicked()
                        {
                            self.confirm_delete = true;
                        }
                        ui.separator();
                        ui.checkbox(&mut self.show_diagnostics, "Проверка").on_hover_text(
                            "Ctrl+L. Ошибки YAML, RegExp, дубликаты и проблемы импортов",
                        );
                        if let Some(workspace) = &self.workspace {
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
                    MainTab::Settings => {
                        if ui
                            .button("Сохранить настройки")
                            .on_hover_text("Ctrl+S")
                            .clicked()
                        {
                            self.save_settings();
                        }
                        if ui
                            .button("Обновить настройки")
                            .on_hover_text("Ctrl+R")
                            .clicked()
                        {
                            self.reload_settings();
                        }
                        if self.settings.dirty() {
                            ui.colored_label(
                                egui::Color32::from_rgb(210, 135, 25),
                                "Настройки не сохранены",
                            );
                        }
                    }
                    MainTab::Rhai => {
                        if ui
                            .button("Скомпилировать")
                            .on_hover_text("Ctrl+Shift+Enter")
                            .clicked()
                        {
                            self.rhai_lab.compile_current();
                        }
                        if ui
                            .button("Запустить")
                            .on_hover_text("Ctrl+Enter")
                            .clicked()
                        {
                            self.rhai_lab.run_current();
                        }
                    }
                }
            });
        });
    }

'''
app = app[:start] + new_top_bar + app[end:]

old_disabled = '''        ui.horizontal_wrapped(|ui| {
            ui.label("Условие срабатывания:");
            ui.selectable_value(&mut self.draft.kind, MatchKind::Trigger, "Обычный триггер");
            ui.selectable_value(&mut self.draft.kind, MatchKind::Regex, "Гибкий RegExp");
            ui.separator();
            ui.checkbox(&mut self.draft.disabled, "Правило временно отключено");
        });
'''
new_disabled = '''        let disabled_changed = ui
            .horizontal_wrapped(|ui| {
                ui.label("Условие срабатывания:");
                ui.selectable_value(&mut self.draft.kind, MatchKind::Trigger, "Обычный триггер");
                ui.selectable_value(&mut self.draft.kind, MatchKind::Regex, "Гибкий RegExp");
                ui.separator();
                ui.checkbox(&mut self.draft.disabled, "Выключить триггер / правило")
                    .on_hover_text(
                        "Сразу применяет disabled: true к рабочей копии. Для записи на диск нажмите Ctrl+S",
                    )
                    .changed()
            })
            .inner;
        if disabled_changed {
            self.apply_structured();
        }
'''
if old_disabled not in app:
    raise SystemExit("structured disabled control marker not found")
app = app.replace(old_disabled, new_disabled, 1)

old_tabs = '''            MainTab::Settings => {
                let config_root = self.config_root.clone();
                self.settings.ui(ui, &config_root, &mut self.status);
            }
        }
'''
new_tabs = '''            MainTab::Settings => {
                let config_root = self.config_root.clone();
                self.settings.ui(ui, &config_root, &mut self.status);
            }
            MainTab::Rhai => {
                self.rhai_lab.ui(ui, &mut self.status);
            }
        }
'''
if old_tabs not in app:
    raise SystemExit("main tab render marker not found")
app = app.replace(old_tabs, new_tabs, 1)

shortcut_marker = '                            shortcut_row(ui, "Ctrl+L", "Показать или скрыть проверку");\n'
if "Скомпилировать Rhai-скрипт" not in app:
    app = app.replace(
        shortcut_marker,
        shortcut_marker
        + '                            shortcut_row(ui, "Ctrl+Enter", "Запустить Rhai-скрипт");\n'
        + '                            shortcut_row(ui, "Ctrl+Shift+Enter", "Скомпилировать Rhai-скрипт");\n',
        1,
    )

APP.write_text(app, encoding="utf-8")

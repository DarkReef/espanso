from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / "espanso-editor/src/app.rs"
NATIVE = ROOT / "espanso-ui/src/win32/native.cpp"
IMPORTS = ROOT / "espanso-editor/src/yaml_imports.rs"


def replace_function(source: str, name: str, next_name: str, replacement: str) -> str:
    start_marker = f"    fn {name}"
    end_marker = f"    fn {next_name}"
    start = source.index(start_marker)
    end = source.index(end_marker, start)
    return source[:start] + replacement.rstrip() + "\n\n" + source[end:]


def patch_app() -> None:
    source = APP.read_text(encoding="utf-8")

    if "    yaml_imports,\n" not in source:
        source = source.replace(
            "        RegexExampleResult, RuleDraft, RuleId,\n    },\n};\n",
            "        RegexExampleResult, RuleDraft, RuleId,\n    },\n    yaml_imports,\n};\n",
            1,
        )

    if "fn set_file_import_enabled" not in source:
        marker = "    fn create_rule(&mut self) {"
        method = r'''    fn set_file_import_enabled(&mut self, file: PathBuf, enabled: bool) {
        let display_name = relative_display(&self.config_root, &file);
        let result = (|| -> Result<String, String> {
            let workspace = self
                .workspace
                .as_mut()
                .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())?;
            let files = workspace.files();
            let base_file = yaml_imports::find_base_file(&files, workspace.match_root())
                .ok_or_else(|| "Не найден match/base.yml или match/base.yaml".to_owned())?;
            let base_content = workspace
                .raw_file(&base_file)
                .map_err(|error| ru_message(&error.to_string()))?
                .to_owned();
            let updated = yaml_imports::update_import(
                &base_content,
                &base_file,
                &file,
                enabled,
            )?;

            if updated != base_content {
                workspace
                    .set_raw_file(&base_file, updated)
                    .map_err(|error| ru_message(&error.to_string()))?;
            }

            Ok(if enabled {
                format!(
                    "Файл {display_name} подключён через base.yml. Сохраните изменения (Ctrl+S)"
                )
            } else {
                format!(
                    "Файл {display_name} исключён из imports base.yml. Сохраните изменения (Ctrl+S)"
                )
            })
        })();

        self.status = match result {
            Ok(message) => message,
            Err(error) => format!("Не удалось изменить imports base.yml: {error}"),
        };
    }

'''
        source = source.replace(marker, method + marker, 1)

    top_bar = r'''    fn top_bar(&mut self, root: &mut egui::Ui) {
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
            });

            ui.separator();
            ui.horizontal(|ui| {
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
        });
    }'''
    source = replace_function(source, "top_bar", "rules_panel", top_bar)

    rules_panel = r'''    fn rules_panel(&mut self, root: &mut egui::Ui) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let files = workspace.files();
        let rules = workspace.rules();
        let config_root = workspace.config_root().to_path_buf();
        let match_root = workspace.match_root().to_path_buf();
        let files_count = files.len();
        let rules_count = rules.len();
        let base_file = yaml_imports::find_base_file(&files, &match_root);
        let (import_entries, import_error) = match base_file.as_ref() {
            Some(base_file) => match workspace.raw_file(base_file) {
                Ok(base_content) => match yaml_imports::import_entries(
                    &files,
                    base_file,
                    base_content,
                ) {
                    Ok(entries) => (entries, None),
                    Err(error) => (Vec::new(), Some(error)),
                },
                Err(error) => (Vec::new(), Some(ru_message(&error.to_string()))),
            },
            None => (Vec::new(), None),
        };
        let mut pending_import_toggle = None;

        egui::Panel::left("rules")
            .resizable(true)
            .default_size(350.0)
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Правила");
                    ui.label(
                        egui::RichText::new(format!("{rules_count} правил · {files_count} файлов"))
                            .weak(),
                    );
                });
                let search = ui.add(
                    egui::TextEdit::singleline(&mut self.filter)
                        .desired_width(f32::INFINITY)
                        .hint_text("Поиск по триггеру, названию или тексту…"),
                );
                if self.focus_filter {
                    search.request_focus();
                    self.focus_filter = false;
                }

                ui.add_space(4.0);
                ui.group(|ui| {
                    ui.strong("YAML-файлы");
                    ui.label(
                        egui::RichText::new(
                            "Флажок подключает файл через imports в base.yml; название фильтрует правила",
                        )
                        .weak(),
                    );
                    if base_file.is_none() {
                        ui.colored_label(
                            ui.visuals().error_fg_color,
                            "Не найден base.yml или base.yaml — подключение файлов недоступно",
                        );
                    }
                    if let Some(error) = &import_error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }

                    egui::ScrollArea::vertical()
                        .id_salt("yaml_file_list")
                        .max_height(165.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.add_space(24.0);
                                if ui
                                    .selectable_label(self.file_filter.is_none(), "Все YAML-файлы")
                                    .clicked()
                                {
                                    self.file_filter = None;
                                }
                            });

                            for file in &files {
                                let is_base = base_file.as_ref() == Some(file);
                                let mut enabled = if is_base {
                                    true
                                } else {
                                    import_entries.iter().any(|entry| {
                                        entry.path.as_path() == file.as_path() && entry.enabled
                                    })
                                };
                                let can_toggle = !is_base
                                    && base_file.is_some()
                                    && import_error.is_none();

                                ui.horizontal(|ui| {
                                    let checkbox = ui.add_enabled(
                                        can_toggle,
                                        egui::Checkbox::new(&mut enabled, ""),
                                    );
                                    let changed = checkbox.changed();
                                    if is_base {
                                        checkbox.on_hover_text(
                                            "Основной файл: он всегда загружается напрямую",
                                        );
                                    } else if can_toggle {
                                        checkbox.on_hover_text(
                                            "Добавить или удалить файл из imports в base.yml",
                                        );
                                    } else {
                                        checkbox.on_hover_text(
                                            "Сначала исправьте base.yml или создайте основной файл",
                                        );
                                    }
                                    if changed {
                                        pending_import_toggle = Some((file.clone(), enabled));
                                    }

                                    let selected = self.file_filter.as_ref() == Some(file);
                                    if ui
                                        .selectable_label(
                                            selected,
                                            relative_display(&config_root, file),
                                        )
                                        .clicked()
                                    {
                                        self.file_filter = Some(file.clone());
                                    }
                                    if is_base {
                                        ui.label(egui::RichText::new("основной").weak());
                                    }
                                });
                            }
                        });
                });
                ui.separator();

                let filter = self.filter.to_lowercase();
                let mut visible = 0_usize;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for rule in rules {
                        if self
                            .file_filter
                            .as_ref()
                            .is_some_and(|file| file != &rule.id.file)
                        {
                            continue;
                        }
                        let haystack = format!(
                            "{} {} {} {}",
                            rule.display_name(),
                            rule.replacement_preview,
                            rule.draft.regex,
                            rule.draft.triggers.join(" ")
                        )
                        .to_lowercase();
                        if !filter.is_empty() && !haystack.contains(&filter) {
                            continue;
                        }
                        visible += 1;
                        let selected = self.selected.as_ref() == Some(&rule.id);
                        let kind = if rule.draft.kind == MatchKind::Regex {
                            "RegExp"
                        } else {
                            "Триггер"
                        };
                        let label = format!(
                            "{kind}  ·  {}\n{}",
                            rule.display_name(),
                            relative_display(&config_root, &rule.id.file)
                        );
                        if ui.selectable_label(selected, label).clicked() {
                            self.select_rule(rule.id);
                        }
                        if !rule.replacement_preview.is_empty() {
                            ui.label(egui::RichText::new(rule.replacement_preview.as_str()).weak());
                        }
                        ui.separator();
                    }
                    if visible == 0 {
                        ui.vertical_centered(|ui| {
                            ui.add_space(20.0);
                            ui.label("По заданному фильтру ничего не найдено");
                        });
                    }
                });
            });

        if let Some((file, enabled)) = pending_import_toggle {
            self.set_file_import_enabled(file, enabled);
        }
    }'''
    source = replace_function(source, "rules_panel", "central_editor", rules_panel)

    APP.write_text(source, encoding="utf-8")


def patch_native() -> None:
    source = NATIVE.read_text(encoding="utf-8")
    helper = r'''
std::wstring utf8_to_wide(const std::string &value) {
    if (value.empty()) {
        return std::wstring();
    }

    int length = MultiByteToWideChar(
        CP_UTF8, MB_ERR_INVALID_CHARS, value.c_str(),
        static_cast<int>(value.size()), nullptr, 0);
    if (length <= 0) {
        return std::wstring();
    }

    std::wstring result(static_cast<size_t>(length), L'\0');
    MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value.c_str(),
                        static_cast<int>(value.size()), result.data(), length);
    return result;
}
'''
    if "std::wstring utf8_to_wide" not in source:
        source = source.replace(
            "using json = nlohmann::json;\n",
            "using json = nlohmann::json;\n" + helper + "\n",
            1,
        )

    old_conversion = """    // Convert to wide chars
    std::wstring wide_label(label.length(), L'#');
    mbstowcs(&wide_label[0], label.c_str(), label.length());
"""
    new_conversion = """    // Labels arrive from Rust as UTF-8 JSON strings.
    std::wstring wide_label = utf8_to_wide(label);
"""
    source = source.replace(old_conversion, new_conversion)
    source = source.replace(
        "ARRAYSIZE(variables->nid.szTip), L\"espanso\");",
        "ARRAYSIZE(variables->nid.szTip), L\"rEspanso\");",
        1,
    )
    NATIVE.write_text(source, encoding="utf-8")


def patch_imports() -> None:
    source = IMPORTS.read_text(encoding="utf-8")
    source = source.replace(
        """            if insertion > 0 && !content[..insertion].ends_with(['\\n', '\\r']) {
                result.push_str(newline);
            }
""",
        """            let prefix = &content[..insertion];
            if insertion > 0 && !prefix.ends_with('\\n') && !prefix.ends_with('\\r') {
                result.push_str(newline);
            }
""",
        1,
    )
    source = source.replace(
        'update_import("matches:\\r\\n  []\\r\\n", &base, &target, true)',
        'update_import("matches: []\\r\\n", &base, &target, true)',
        1,
    )
    IMPORTS.write_text(source, encoding="utf-8")


if __name__ == "__main__":
    patch_app()
    patch_native()
    patch_imports()

           if let Ok(rule) = workspace.rule(&id) {
                    self.raw_rule = rule.raw;
                }
            }
            self.status = format!("Не удалось добавить переменную: {error}");
        }
    }
}

''', 4)
app = replace_between(
    app,
    "    fn add_dynamic_variable(&mut self, action: DynamicVariableAction) {",
    "    fn report_rhai_action",
    add_dynamic,
    "dynamic variable action",
)

app = replace_once(
    app,
    "let (save, new_rule, duplicate, delete, reload, search, apply, diagnostics, help)",
    "let (save, new_rule, duplicate, delete, reload, search, apply_raw, diagnostics, help)",
    "raw shortcut tuple",
)
app = replace_once(
    app,
    "        if apply && self.selected.is_some() {\n            self.apply_current();\n        }",
    "        if apply_raw && self.selected.is_some() && self.mode == EditorMode::Raw {\n"
    "            self.apply_raw();\n"
    "        }",
    "raw shortcut action",
)

app = replace_once(
    app,
    "        let mut pending_import_toggle = None;\n",
    "        let mut pending_import_toggle = None;\n"
    "        let mut request_create_yaml_file = false;\n"
    "        let mut request_delete_yaml_file = None;\n",
    "yaml pending actions",
)

yaml_buttons = rust(r'''
ui.horizontal(|ui| {
    if ui
        .button("Создать файл")
        .on_hover_text("Создать новый YAML-файл в папке match")
        .clicked()
    {
        request_create_yaml_file = true;
    }
    let selected_file = self.file_filter.clone();
    let can_delete = selected_file
        .as_ref()
        .is_some_and(|file| base_file.as_ref() != Some(file));
    if ui
        .add_enabled(can_delete, egui::Button::new("Удалить файл"))
        .on_hover_text("Удалить выбранный YAML-файл; base.yml удалить нельзя")
        .clicked()
    {
        request_delete_yaml_file = selected_file;
    }
});
''', 24)
app = insert_before(
    app,
    "                        ui.add(\n                            egui::TextEdit::singleline(&mut self.yaml_file_filter)",
    yaml_buttons,
    "yaml file buttons",
)

rules_tail_old = (
    "        if let Some((file, enabled)) = pending_import_toggle {\n"
    "            self.set_file_import_enabled(file, enabled);\n"
    "        }\n"
    "    }\n\n"
    "    fn central_editor"
)
rules_tail_new = (
    "        if let Some((file, enabled)) = pending_import_toggle {\n"
    "            self.set_file_import_enabled(file, enabled);\n"
    "        }\n"
    "        if request_create_yaml_file {\n"
    "            self.show_create_yaml_file = true;\n"
    "        }\n"
    "        if let Some(file) = request_delete_yaml_file {\n"
    "            self.pending_delete_yaml_file = Some(file);\n"
    "            self.confirm_delete_yaml_file = true;\n"
    "        }\n"
    "    }\n\n"
    "    fn central_editor"
)
app = replace_once(app, rules_tail_old, rules_tail_new, "yaml actions after panel")

app = replace_once(
    app,
    'ui.selectable_value(&mut self.mode, EditorMode::Structured, "Удобный редактор");',
    'ui.selectable_value(&mut self.mode, EditorMode::Structured, "Редактор");',
    "editor tab title",
)
app = replace_once(
    app,
    '"Динамические переменные: {{date}}, {{time}}, {{clipboard}} и свои"',
    '"Переменные: date, time, string, clipboard, echo, choice, form, random, rhai, script, shell"',
    "dynamic variable hover",
)

structured_editor = rust(r'''
fn structured_editor(&mut self, ui: &mut egui::Ui) {
    let previous_draft = self.draft.clone();
    ui.horizontal_wrapped(|ui| {
        ui.label("Условие срабатывания:");
        ui.selectable_value(&mut self.draft.kind, MatchKind::Trigger, "Обычный триггер");
        ui.selectable_value(&mut self.draft.kind, MatchKind::Regex, "Гибкий RegExp");
    });
    ui.horizontal(|ui| {
        ui.label("Название правила");
        ui.add(
            egui::TextEdit::singleline(&mut self.draft.label)
                .desired_width(f32::INFINITY)
                .hint_text("Необязательно, например: Номер пациента"),
        );
    });

    match self.draft.kind {
        MatchKind::Trigger => {
            let mut triggers = self.draft.triggers.join(", ");
            ui.label("Триггеры");
            ui.label(
                egui::RichText::new(
                    "Можно указать несколько вариантов через запятую, например :привет, :здр",
                )
                .weak(),
            );
            if ui
                .add(
                    egui::TextEdit::singleline(&mut triggers)
                        .desired_width(f32::INFINITY)
                        .hint_text(":привет, :здр"),
                )
                .changed()
            {
                self.draft.triggers = triggers
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
        }
        MatchKind::Regex => self.regex_editor(ui),
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label("Текст подстановки");
        if ui
            .small_button("?")
            .on_hover_text(
                "Переменные: date, time, string, clipboard, echo, choice, form, random, rhai, script, shell",
            )
            .clicked()
        {
            self.show_dynamic_variables = true;
        }
    });
    ui.add(
        egui::TextEdit::multiline(&mut self.draft.replace)
            .desired_rows(10)
            .desired_width(f32::INFINITY)
            .hint_text("Текст, который rEspanso вставит вместо триггера"),
    );
    if self.draft != previous_draft {
        self.apply_structured();
    }
}

''', 4)
app = replace_between(
    app,
    "    fn structured_editor(&mut self, ui: &mut egui::Ui) {",
    "    fn regex_editor(&mut self, ui: &mut egui::Ui) {",
    structured_editor,
    "structured editor UI",
)

yaml_dialogs = rust(r'''
if self.show_create_yaml_file {
    let mut open = self.show_create_yaml_file;
    let mut create = false;
    let mut cancel = false;
    egui::Window::new("Создание YAML-файла")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(context, |ui| {
            ui.label("Имя нового файла в папке match");
            ui.add(
                egui::TextEdit::singleline(&mut self.new_yaml_file_name)
                    .desired_width(360.0)
                    .hint_text("Например: терапия.yml"),
            );
            ui.label(
                egui::RichText::new("Расширение .yml добавится автоматически").weak(),
            );
            ui.horizontal(|ui| {
                if ui.button("Создать").clicked() {
                    create = true;
                }
                if ui.button("Отмена").clicked() {
                    cancel = true;
                }
            });
        });
    if create && self.create_yaml_file() {
        open = false;
    } else if cancel {
        open = false;
    }
    self.show_create_yaml_file = open;
}

if self.confirm_delete_yaml_file {
    let mut open = self.confirm_delete_yaml_file;
    let mut delete = false;
    let mut cancel = false;
    let path = self.pending_delete_yaml_file.clone();
    egui::Window::new("Удаление YAML-файла")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(context, |ui| {
            if let Some(path) = &path {
                ui.label(format!(
                    "Удалить {} и все правила внутри?",
                    relative_display(&self.config_root, path)
                ));
            }
            ui.colored_label(
                egui::Color32::from_rgb(190, 70, 70),
                "Файл будет удалён сразу. Сначала сохраните нужные изменения.",
            );
            ui.horizontal(|ui| {
                if ui.button("Удалить файл").clicked() {
                    delete = true;
                }
                if ui.button("Отмена").clicked() {
                    cancel = true;
                }
            });
        });
    if delete {
        if let Some(path) = path {
            if self.delete_yaml_file(path) {
                self.pending_delete_yaml_file = None;
                open = false;
            }
        }
    } else if cancel {
        self.pending_delete_yaml_file = None;
        open = false;
    }
    self.confirm_delete_yaml_file = open;
}

''', 8)
app = insert_before(
    app,
    "        if self.show_dynamic_variables {\n",
    yaml_dialogs,
    "yaml dialogs",
)

app = replace_once(
    app,
    '"Применить правило или запустить Rhai-скрипт",',
    '"Проверить исходный YAML или запустить Rhai-скрипт",',
    "Ctrl Enter help",
)
app = insert_before(
    app,
    '                            shortcut_row(ui, "Ctrl+F", "Перейти к поиску правил");\n',
    rust(r'''
shortcut_row(
    ui,
    "Ctrl+Alt+M",
    "Найти триггер по выделенному тексту",
);
''', 28),
    "Ctrl Alt M help",
)

status_p
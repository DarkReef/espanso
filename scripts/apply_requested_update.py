from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


app_path = Path("espanso-editor/src/app.rs")
app = app_path.read_text(encoding="utf-8")
app = replace_once(
    app,
    "    config_transfer::{self, PackageSummary},\n",
    "    config_transfer::{self, PackageSummary},\n    dynamic_variables::{self, DynamicVariableAction, DynamicVariableDialog},\n",
    "dynamic variable import",
)
app = replace_once(
    app,
    "    show_diagnostics: bool,\n    show_shortcuts: bool,\n",
    "    show_diagnostics: bool,\n    show_shortcuts: bool,\n    show_dynamic_variables: bool,\n    dynamic_variables: DynamicVariableDialog,\n",
    "dynamic variable fields",
)
app = replace_once(
    app,
    "            show_diagnostics: true,\n            show_shortcuts: false,\n",
    "            show_diagnostics: true,\n            show_shortcuts: false,\n            show_dynamic_variables: false,\n            dynamic_variables: DynamicVariableDialog::default(),\n",
    "dynamic variable initialization",
)
app = app.replace(
    "Проверен устойчивый снимок проекта: изменено файлов {changed_files}",
    "Проект проверен после устойчивого изменения файлов: изменено {changed_files}",
)
method_marker = "    fn report_rhai_action(&mut self, result: Result<String, String>) {\n"
method = r'''    fn add_dynamic_variable(&mut self, action: DynamicVariableAction) {
        let Some(id) = self.selected.clone() else {
            "Сначала выберите правило".clone_into(&mut self.status);
            return;
        };
        let previous_replace = self.draft.replace.clone();
        self.draft.replace.push_str(&action.placeholder);
        self.apply_structured();

        match dynamic_variables::upsert_rule_variable(&self.raw_rule, &action.definition) {
            Ok((updated, added)) => {
                let result = self
                    .workspace
                    .as_mut()
                    .ok_or_else(|| "Рабочая область YAML не загружена".to_owned())
                    .and_then(|workspace| {
                        workspace
                            .update_rule_raw(&id, &updated)
                            .map_err(|error| ru_message(&error.to_string()))
                    });
                match result {
                    Ok(()) => {
                        self.refresh_selected();
                        self.status = if added {
                            format!("{}. Нажмите Ctrl+S для записи YAML", action.message)
                        } else {
                            format!(
                                "Переменная {} уже была объявлена; шаблон добавлен в текст. Нажмите Ctrl+S",
                                action.placeholder
                            )
                        };
                    }
                    Err(error) => {
                        self.draft.replace = previous_replace;
                        self.apply_structured();
                        self.status = format!("Не удалось добавить переменную: {error}");
                    }
                }
            }
            Err(error) => {
                self.draft.replace = previous_replace;
                self.apply_structured();
                self.status = format!("Не удалось добавить переменную: {error}");
            }
        }
    }

'''
app = replace_once(app, method_marker, method + method_marker, "dynamic variable method")
app = replace_once(
    app,
    "        ui.add_space(8.0);\n        ui.label(\"Текст подстановки\");\n        ui.add(\n",
    "        ui.add_space(8.0);\n        ui.horizontal(|ui| {\n            ui.label(\"Текст подстановки\");\n            if ui\n                .small_button(\"?\")\n                .on_hover_text(\"Динамические переменные: {{date}}, {{time}}, {{clipboard}} и свои\")\n                .clicked()\n            {\n                self.show_dynamic_variables = true;\n            }\n        });\n        ui.add(\n",
    "replacement help button",
)
app = replace_once(
    app,
    "        let generation = self.diagnostics.generation();\n        let active_count = self.diagnostics.active_count();\n",
    "        let active_count = self.diagnostics.active_count();\n",
    "diagnostic generation variable",
)
app = replace_once(
    app,
    "                ui.label(\n                    egui::RichText::new(format!(\n                        \"Снимок #{generation} · активных проблем: {active_count}\"\n                    ))\n                    .weak(),\n                );\n",
    "                ui.label(\n                    egui::RichText::new(format!(\"Активных проблем: {active_count}\")).weak(),\n                );\n",
    "diagnostic snapshot header",
)
app = replace_once(
    app,
    "                                    \"Наблюдений: {} · впервые: #{} · последнее: #{}\",\n                                    diagnostic.occurrence_count,\n                                    diagnostic.first_seen_generation,\n                                    diagnostic.last_seen_generation\n",
    "                                    \"Наблюдений: {}\",\n                                    diagnostic.occurrence_count\n",
    "diagnostic snapshot details",
)
dialog_marker = "        self.config_import_confirmation(context);\n\n        if self.show_shortcuts {\n"
dialog_code = '''        self.config_import_confirmation(context);

        if self.show_dynamic_variables {
            let mut open = self.show_dynamic_variables;
            if let Some(action) = self.dynamic_variables.show(context, &mut open) {
                self.add_dynamic_variable(action);
            }
            self.show_dynamic_variables = open;
        }

        if self.show_shortcuts {
'''
app = replace_once(app, dialog_marker, dialog_code, "dynamic variable dialog")
app_path.write_text(app, encoding="utf-8")

lib_path = Path("espanso-editor/src/lib.rs")
lib = lib_path.read_text(encoding="utf-8")
lib = replace_once(
    lib,
    "pub mod diagnostics;\n",
    "pub mod diagnostics;\npub mod dynamic_variables;\n",
    "dynamic variable module",
)
lib_path.write_text(lib, encoding="utf-8")

docs_path = Path("docs/respanso/MATCH_STUDIO.ru.md")
docs = docs_path.read_text(encoding="utf-8")
docs += """

## Динамические переменные в редакторе правил

Рядом с заголовком «Текст подстановки» находится небольшая кнопка `?`. Она открывает список готовых переменных `{{date}}`, `{{time}}`, `{{weekday}}`, `{{clipboard}}` и форму для собственной переменной.

При выборе Studio добавляет шаблон в конец текста и создаёт или переиспользует блок `vars` внутри выбранного правила. Для своей переменной задаются имя, тип расширения rEspanso и необязательные параметры YAML без строки `params:`.

Пример параметров даты:

```yaml
format: "%d.%m.%Y"
```

После добавления нажмите `Ctrl+S`, чтобы записать изменения в YAML-файл.
"""
docs_path.write_text(docs, encoding="utf-8")

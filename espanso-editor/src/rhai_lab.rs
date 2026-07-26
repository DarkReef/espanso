use eframe::egui;
use rhai::{Dynamic, Engine, Scope, AST};

const DEFAULT_SCRIPT: &str = r#"let length = input.len;
if length > 0 {
    `Триггер ${trigger}: получено ${length} символов`
} else {
    "Пустая строка"
}"#;

struct CompiledScript {
    source: String,
    ast: AST,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckKind {
    Idle,
    Success,
    Error,
}

struct CheckMessage {
    kind: CheckKind,
    text: String,
}

impl Default for CheckMessage {
    fn default() -> Self {
        Self {
            kind: CheckKind::Idle,
            text: String::new(),
        }
    }
}

pub struct RhaiLab {
    engine: Engine,
    script: String,
    trigger: String,
    input: String,
    clipboard: String,
    compiled: Option<CompiledScript>,
    syntax: CheckMessage,
    compilation: CheckMessage,
    execution: CheckMessage,
    output: String,
}

impl RhaiLab {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        engine.set_max_operations(200_000);
        Self {
            engine,
            script: DEFAULT_SCRIPT.to_owned(),
            trigger: ":пример".to_owned(),
            input: "тестовая строка".to_owned(),
            clipboard: "содержимое буфера".to_owned(),
            compiled: None,
            syntax: CheckMessage::default(),
            compilation: CheckMessage::default(),
            execution: CheckMessage::default(),
            output: String::new(),
        }
    }

    pub fn compile_current(&mut self) {
        match compile_source(&self.engine, &self.script) {
            Ok(ast) => {
                self.compiled = Some(CompiledScript {
                    source: self.script.clone(),
                    ast,
                });
                self.syntax = success("Синтаксис корректен");
                self.compilation = success("Скрипт скомпилирован в AST и готов к запуску");
                self.execution = CheckMessage::default();
            }
            Err(error) => {
                self.compiled = None;
                self.syntax = failure(format!("Ошибка синтаксиса: {error}"));
                self.compilation = failure("Компиляция не выполнена из-за ошибки синтаксиса");
                self.execution = CheckMessage::default();
                self.output.clear();
            }
        }
    }

    pub fn run_current(&mut self) {
        let needs_compile = self
            .compiled
            .as_ref()
            .is_none_or(|compiled| compiled.source != self.script);
        if needs_compile {
            self.compile_current();
        }
        let Some(compiled) = &self.compiled else {
            return;
        };

        let mut scope = Scope::new();
        scope.push("trigger", self.trigger.clone());
        scope.push("input", self.input.clone());
        scope.push("clipboard", self.clipboard.clone());

        match execute_ast(&self.engine, &mut scope, &compiled.ast) {
            Ok(output) => {
                self.output = output;
                self.execution = success("Скрипт выполнен без ошибок");
            }
            Err(error) => {
                self.output.clear();
                self.execution = failure(format!("Ошибка выполнения: {error}"));
            }
        }
    }

    pub fn ui(&mut self, root: &mut egui::Ui, app_status: &mut String) {
        egui::CentralPanel::default().show(root, |ui| {
            ui.heading("Лаборатория Rhai");
            ui.label(
                "Проверяйте синтаксис, компилируйте скрипт в AST и запускайте его на тестовых данных до добавления в правила rEspanso.",
            );
            ui.label(
                egui::RichText::new(
                    "Доступны стандартные возможности Rhai и тестовые переменные ниже. Файловые и сетевые функции не зарегистрированы.",
                )
                .weak(),
            );
            ui.separator();

            ui.horizontal_wrapped(|ui| {
                ui.label("Примеры:");
                if ui.button("Условие и строка").clicked() {
                    DEFAULT_SCRIPT.clone_into(&mut self.script);
                    self.invalidate();
                }
                if ui.button("Арифметика").clicked() {
                    "let total = 40 + 2; total".clone_into(&mut self.script);
                    self.invalidate();
                }
                if ui.button("Функция").clicked() {
                    "fn normalize(value) { value.trim().to_lower() }\nnormalize(input)"
                        .clone_into(&mut self.script);
                    self.invalidate();
                }
                if ui.button("Пример ошибки").clicked() {
                    "let value = ;".clone_into(&mut self.script);
                    self.invalidate();
                }
            });

            ui.add_space(6.0);
            ui.group(|ui| {
                ui.label(egui::RichText::new("Тестовые переменные").strong());
                egui::Grid::new("rhai_variables")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.monospace("trigger");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.trigger)
                                .desired_width(f32::INFINITY),
                        );
                        ui.end_row();
                        ui.monospace("input");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.input)
                                .desired_width(f32::INFINITY),
                        );
                        ui.end_row();
                        ui.monospace("clipboard");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.clipboard)
                                .desired_width(f32::INFINITY),
                        );
                        ui.end_row();
                    });
            });

            ui.add_space(8.0);
            ui.label("Исходный Rhai-скрипт");
            if ui
                .add(
                    egui::TextEdit::multiline(&mut self.script)
                        .code_editor()
                        .desired_rows(20)
                        .desired_width(f32::INFINITY),
                )
                .changed()
            {
                self.invalidate();
            }

            ui.horizontal_wrapped(|ui| {
                if ui.button("Проверить синтаксис").clicked() {
                    self.check_syntax();
                }
                if ui
                    .button("Скомпилировать")
                    .on_hover_text("Ctrl+Shift+Enter")
                    .clicked()
                {
                    self.compile_current();
                }
                if ui
                    .add_sized(
                        [170.0, 30.0],
                        egui::Button::new(egui::RichText::new("Запустить").strong()),
                    )
                    .on_hover_text("Ctrl+Enter")
                    .clicked()
                {
                    self.run_current();
                }
            });

            show_check(ui, "Синтаксис", &self.syntax);
            show_check(ui, "Компиляция", &self.compilation);
            show_check(ui, "Выполнение", &self.execution);

            if !self.output.is_empty() {
                ui.separator();
                ui.label(egui::RichText::new("Результат").strong());
                ui.add(
                    egui::TextEdit::multiline(&mut self.output)
                        .code_editor()
                        .desired_rows(5)
                        .desired_width(f32::INFINITY)
                        .interactive(false),
                );
            }

            if self.execution.kind == CheckKind::Success {
                "Rhai-скрипт успешно скомпилирован и выполнен".clone_into(app_status);
            } else if self.compilation.kind == CheckKind::Success {
                "Rhai-скрипт успешно скомпилирован".clone_into(app_status);
            } else if self.syntax.kind == CheckKind::Error {
                "Rhai-скрипт содержит ошибку синтаксиса".clone_into(app_status);
            }
        });
    }

    fn check_syntax(&mut self) {
        match compile_source(&self.engine, &self.script) {
            Ok(_) => self.syntax = success("Синтаксис корректен"),
            Err(error) => self.syntax = failure(format!("Ошибка синтаксиса: {error}")),
        }
    }

    fn invalidate(&mut self) {
        self.compiled = None;
        self.syntax = CheckMessage::default();
        self.compilation = CheckMessage::default();
        self.execution = CheckMessage::default();
        self.output.clear();
    }
}

impl Default for RhaiLab {
    fn default() -> Self {
        Self::new()
    }
}

fn compile_source(engine: &Engine, source: &str) -> Result<AST, String> {
    engine.compile(source).map_err(|error| error.to_string())
}

fn execute_ast(engine: &Engine, scope: &mut Scope<'_>, ast: &AST) -> Result<String, String> {
    engine
        .eval_ast_with_scope::<Dynamic>(scope, ast)
        .map(format_dynamic)
        .map_err(|error| error.to_string())
}

fn format_dynamic(value: Dynamic) -> String {
    if value.is_unit() {
        return "()".to_owned();
    }
    format!("{}\nТип: {}", value, value.type_name())
}

fn success(text: impl Into<String>) -> CheckMessage {
    CheckMessage {
        kind: CheckKind::Success,
        text: text.into(),
    }
}

fn failure(text: impl Into<String>) -> CheckMessage {
    CheckMessage {
        kind: CheckKind::Error,
        text: text.into(),
    }
}

fn show_check(ui: &mut egui::Ui, label: &str, check: &CheckMessage) {
    if check.kind == CheckKind::Idle {
        return;
    }
    let color = if check.kind == CheckKind::Success {
        egui::Color32::from_rgb(40, 150, 90)
    } else {
        ui.visuals().error_fg_color
    };
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!("{label}:"));
        ui.colored_label(color, check.text.as_str());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_script_compiles_and_runs() {
        let engine = Engine::new();
        let ast = compile_source(&engine, "let value = 40 + 2; value").expect("compile");
        let mut scope = Scope::new();
        let output = execute_ast(&engine, &mut scope, &ast).expect("execute");
        assert!(output.starts_with("42"));
    }

    #[test]
    fn invalid_script_is_rejected_during_compilation() {
        let engine = Engine::new();
        let error = compile_source(&engine, "let value = ;").expect_err("invalid script");
        assert!(!error.is_empty());
    }

    #[test]
    fn test_variables_are_available_to_script() {
        let engine = Engine::new();
        let ast = compile_source(&engine, "trigger + input").expect("compile");
        let mut scope = Scope::new();
        scope.push("trigger", ":test".to_owned());
        scope.push("input", " value".to_owned());
        let output = execute_ast(&engine, &mut scope, &ast).expect("execute");
        assert!(output.contains(":test value"));
    }
}

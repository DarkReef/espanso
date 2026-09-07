use eframe::egui;
use espanso_ai::{redact, Provider, Settings};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    time::Duration,
};

pub struct AiPanel {
    root: PathBuf,
    settings: Settings,
    saved_settings: String,
    key: String,
    status: String,
    text: String,
    output: String,
    reviewed: bool,
    pending: Option<Receiver<Result<String, String>>>,
    target: Option<u64>,
    insert_pending: Option<Receiver<Result<String, String>>>,
}
impl AiPanel {
    pub fn new(root: PathBuf, selected: String, target: Option<u64>) -> Self {
        let (settings, status) = match Settings::load(&root) {
            Ok(s) => (s, String::new()),
            Err(e) => (Settings::default(), e),
        };
        let key = espanso_ai::load_key(&root, settings.provider).unwrap_or_default();
        let saved_settings = serde_json::to_string(&settings).unwrap_or_default();
        Self {
            root,
            saved_settings,
            settings,
            key,
            status,
            text: redact(&selected),
            output: String::new(),
            reviewed: false,
            pending: None,
            target,
            insert_pending: None,
        }
    }
    pub fn dirty(&self) -> bool {
        serde_json::to_string(&self.settings).unwrap_or_default() != self.saved_settings
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if let Some(rx) = &self.pending {
            if let Ok(result) = rx.try_recv() {
                self.pending = None;
                match result {
                    Ok(text) => {
                        self.output = text;
                        self.status = "Готово. Проверьте факты перед вставкой.".into();
                    }
                    Err(e) => self.status = e,
                }
            } else {
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
        }
        if let Some(rx) = &self.insert_pending {
            if let Ok(result) = rx.try_recv() {
                self.insert_pending = None;
                self.status = result.unwrap_or_else(|e| e);
            } else {
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("ИИ · медицинский текст");
            ui.label("Выделите текст в приложении и нажмите Alt+L. Исходник отправляется только после проверки в этом окне.");
            ui.add_enabled_ui(self.pending.is_none(), |ui| {
                egui::CollapsingHeader::new("Настройки подключения и контекст").default_open(self.text.is_empty() || !self.settings.enabled).show(ui, |ui| {
                    ui.checkbox(&mut self.settings.enabled, "Включить обработку через ИИ");
                    let old_provider = self.settings.provider;
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.settings.provider, Provider::Openai, "OpenAI");
                        ui.selectable_value(&mut self.settings.provider, Provider::Gigachat, "GigaChat");
                    });
                    if old_provider != self.settings.provider {
                        self.settings.model = match self.settings.provider { Provider::Openai => "gpt-4.1-mini", Provider::Gigachat => "GigaChat" }.into();
                        self.key = espanso_ai::load_key(&self.root, self.settings.provider).unwrap_or_default(); self.reviewed = false;
                    }
                    ui.label("Идентификатор модели (можно изменить)"); ui.text_edit_singleline(&mut self.settings.model);
                    ui.label(match self.settings.provider { Provider::Openai => "API-ключ OpenAI", Provider::Gigachat => "Ключ авторизации GigaChat (Base64, без префикса Basic)" });
                    ui.add(egui::TextEdit::singleline(&mut self.key).password(true).desired_width(f32::INFINITY));
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Сохранить ключ на этом устройстве").clicked() { self.status = espanso_ai::save_key(&self.root, self.settings.provider, &self.key).map(|_| "Ключ сохранён отдельно от настроек".into()).unwrap_or_else(|e|e); }
                        if ui.button("Удалить сохранённый ключ").clicked() {
                            self.status = espanso_ai::save_key(&self.root, self.settings.provider, "").map(|_| { self.key.clear(); "Ключ удалён. Переменная окружения, если задана, сохраняет действие для MCP.".into() }).unwrap_or_else(|e|e);
                        }
                    });
                    ui.small("Без сохранения ключ действует до закрытия окна. Сохранённый ключ — отдельный локальный файл, не зашифрован; на Linux доступен владельцу (0600). Не пересылайте всю рабочую папку приложения.");
                    if self.settings.provider == Provider::Gigachat {
                        ui.label("Scope GigaChat"); ui.text_edit_singleline(&mut self.settings.gigachat_scope);
                        ui.label("Доверенный корневой сертификат PEM (полный путь, при необходимости)"); ui.text_edit_singleline(&mut self.settings.ca_file);
                    }
                    ui.label("Ваш контекст (тоже передаётся провайдеру)");
                    if ui.add(egui::TextEdit::multiline(&mut self.settings.context).desired_rows(4).desired_width(f32::INFINITY)).changed() { self.reviewed = false; }
                    ui.checkbox(&mut self.settings.mcp_allow_rewrite, "Разрешить отправку текста через MCP-инструмент rewrite_text");
                    if ui.button("Сохранить настройки ИИ").clicked() { self.status = self.settings.save(&self.root).map(|_| { self.saved_settings = serde_json::to_string(&self.settings).unwrap_or_default(); "Настройки сохранены".into() }).unwrap_or_else(|e|e); }
                });
                ui.separator();
                ui.label("Текст для отправки — удалите оставшиеся идентификаторы вручную");
                if ui.add(egui::TextEdit::multiline(&mut self.text).desired_rows(7).desired_width(f32::INFINITY)).changed() { self.reviewed = false; self.output.clear(); }
                if ui.button("Повторить локальную маскировку").clicked() { self.text = redact(&self.text); self.settings.context = redact(&self.settings.context); self.reviewed = false; self.output.clear(); }
                ui.small("Маскировка эвристическая: ФИО, даты, контакты и номера могут быть пропущены. Инструкция модели не заменяет удаление данных до отправки.");
                egui::CollapsingHeader::new("Точный текст и контекст после маскировки").show(ui, |ui| {
                    ui.label(espanso_ai::CLINICAL_RULES); ui.separator(); ui.label(redact(&self.settings.context)); ui.separator(); ui.label(redact(&self.text));
                });
                ui.checkbox(&mut self.reviewed, "Проверил отправляемый текст и контекст: персональных данных нет");
                if ui.add_enabled(self.reviewed && self.settings.enabled && !self.text.trim().is_empty(), egui::Button::new("Переформулировать")).clicked() {
                    self.output.clear(); self.status = "Обработка… Можно продолжать работу в других приложениях.".into();
                    let settings = self.settings.clone(); let key = self.key.clone(); let text = self.text.clone(); let (tx, rx) = mpsc::channel();
                    self.pending = Some(rx); let context = ui.ctx().clone();
                    std::thread::spawn(move || { let _ = tx.send(espanso_ai::rewrite(&settings, &key, &text)); context.request_repaint(); });
                }
            });
            if self.pending.is_some() { ui.spinner(); }
            ui.separator(); ui.label(&self.status);
            if !self.output.is_empty() {
                ui.heading("Результат — проверьте и отредактируйте");
                ui.add(egui::TextEdit::multiline(&mut self.output).desired_rows(8).desired_width(f32::INFINITY));
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Копировать результат").clicked() { ui.ctx().copy_text(self.output.clone()); self.status = "Скопировано. Вернитесь в исходное приложение и вставьте Ctrl+V.".into(); }
                    #[cfg(target_os = "linux")]
                    if let Some(target) = self.target {
                        if ui.add_enabled(self.insert_pending.is_none(), egui::Button::new("Вставить в исходное окно")).clicked() {
                            ui.ctx().copy_text(self.output.clone());
                            let (tx, rx) = mpsc::channel(); self.insert_pending = Some(rx);
                            std::thread::spawn(move || {
                                // Let egui publish clipboard ownership before requesting a paste.
                                std::thread::sleep(Duration::from_millis(300));
                                let _ = tx.send(paste_to_window(target));
                            });
                        }
                    }
                });
                ui.small("Вставка заменяет текущее выделение исходного окна. Если вы перемещали курсор или меняли выделение, восстановите нужное место. Буфер содержит результат до следующего копирования.");
            }
        });
    }
}

#[cfg(target_os = "linux")]
fn paste_to_window(target: u64) -> Result<String, String> {
    use std::process::Command;
    let status = Command::new("xdotool")
        .args(["windowactivate", &target.to_string()])
        .status()
        .map_err(|_| "Нет xdotool: используйте копирование и Ctrl+V")?;
    if !status.success() {
        return Err("Исходное окно закрыто или недоступно. Результат остаётся в буфере.".into());
    }
    std::thread::sleep(Duration::from_millis(250));
    let active = Command::new("xdotool")
        .arg("getactivewindow")
        .output()
        .map_err(|_| "Не удалось проверить активное окно")?;
    if !active.status.success()
        || String::from_utf8_lossy(&active.stdout).trim() != target.to_string()
    {
        return Err("Фокус изменился: автоматическая вставка отменена, используйте Ctrl+V".into());
    }
    let status = Command::new("xdotool")
        .args(["key", "--clearmodifiers", "ctrl+v"])
        .status()
        .map_err(|_| "Не удалось вставить текст")?;
    if status.success() {
        Ok("Команда вставки отправлена. Проверьте текст в исходном окне.".into())
    } else {
        Err("Не удалось вставить текст; используйте Ctrl+V".into())
    }
}

impl eframe::App for AiPanel {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.ui(ui);
    }
}
pub fn run(root: PathBuf, selected: String, target: Option<u64>) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([820.0, 740.0])
            .with_min_inner_size([540.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "rEspanso · ИИ",
        options,
        Box::new(move |_| Ok(Box::new(AiPanel::new(root, selected, target)))),
    )
}

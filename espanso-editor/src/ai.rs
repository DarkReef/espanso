use eframe::egui;
use espanso_ai::{
    agents::{self, AgentPermissions, AgentSummary},
    audit, redact, Provider, Settings,
};
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
    agents: Vec<AgentSummary>,
    new_agent_name: String,
    new_agent_permissions: AgentPermissions,
    pairing: Option<(String, String, String)>,
}

impl AiPanel {
    pub fn new(root: PathBuf, selected: String, target: Option<u64>) -> Self {
        let (settings, status) = match Settings::load(&root) {
            Ok(s) => (s, String::new()),
            Err(e) => (Settings::default(), e),
        };
        let key = espanso_ai::load_key(&root, settings.provider).unwrap_or_default();
        let saved_settings = serde_json::to_string(&settings).unwrap_or_default();
        let agents = agents::list(&root).unwrap_or_default();
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
            agents,
            new_agent_name: String::new(),
            new_agent_permissions: AgentPermissions::default(),
            pairing: None,
        }
    }

    pub fn dirty(&self) -> bool {
        serde_json::to_string(&self.settings).unwrap_or_default() != self.saved_settings
    }

    fn refresh_agents(&mut self) {
        match agents::list(&self.root) {
            Ok(items) => self.agents = items,
            Err(error) => self.status = error,
        }
    }

    fn show_agent_manager(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("MCP-агенты · регистрация и права")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("Workspace-инструменты доступны только зарегистрированному агенту с парой Agent ID + Token. Токен хранится у агента; rEspanso сохраняет только его SHA-256-хэш.");
                ui.small("Для процесса MCP задайте RESPANSO_MCP_AGENT_ID и RESPANSO_MCP_TOKEN. Отключение агента, изменение прав или перевыпуск токена проверяется заново при каждом workspace-вызове.");

                if let Some((name, id, token)) = self.pairing.clone() {
                    ui.separator();
                    ui.colored_label(
                        egui::Color32::from_rgb(210, 135, 25),
                        format!("Токен для «{name}» показывается только сейчас"),
                    );
                    ui.label(format!("Agent ID: {id}"));
                    ui.add(
                        egui::TextEdit::singleline(&mut token.clone())
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
                    let snippet = format!(
                        "RESPANSO_MCP_AGENT_ID={id}\nRESPANSO_MCP_TOKEN={token}"
                    );
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Копировать параметры подключения").clicked() {
                            ui.ctx().copy_text(snippet);
                            self.status = "Параметры MCP-агента скопированы".into();
                        }
                        if ui.button("Скрыть токен").clicked() {
                            self.pairing = None;
                        }
                    });
                }

                ui.separator();
                ui.label("Новый агент");
                ui.horizontal(|ui| {
                    ui.label("Имя");
                    ui.text_edit_singleline(&mut self.new_agent_name);
                });
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut self.new_agent_permissions.read_workspace, "читать");
                    ui.checkbox(&mut self.new_agent_permissions.write_workspace, "изменять");
                    ui.checkbox(&mut self.new_agent_permissions.delete_workspace, "удалять");
                    ui.checkbox(&mut self.new_agent_permissions.rewrite_ai, "rewrite_text");
                });
                if ui.button("Зарегистрировать агента").clicked() {
                    match agents::register(
                        &self.root,
                        &self.new_agent_name,
                        self.new_agent_permissions,
                    ) {
                        Ok(pairing) => {
                            self.pairing = Some((
                                pairing.agent.name.clone(),
                                pairing.agent.id.clone(),
                                pairing.token,
                            ));
                            self.new_agent_name.clear();
                            self.refresh_agents();
                            self.status = "MCP-агент зарегистрирован".into();
                        }
                        Err(error) => self.status = error,
                    }
                }

                if !self.agents.is_empty() {
                    ui.separator();
                    ui.label("Зарегистрированные агенты");
                }
                let snapshot = self.agents.clone();
                for agent in snapshot {
                    ui.group(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(&agent.name);
                            ui.monospace(&agent.id);
                            if agent.enabled {
                                ui.colored_label(egui::Color32::from_rgb(40, 160, 90), "активен");
                            } else {
                                ui.colored_label(egui::Color32::from_rgb(190, 70, 70), "отключён");
                            }
                        });

                        let mut permissions = agent.permissions;
                        let mut changed = false;
                        ui.horizontal_wrapped(|ui| {
                            changed |= ui.checkbox(&mut permissions.read_workspace, "читать").changed();
                            changed |= ui.checkbox(&mut permissions.write_workspace, "изменять").changed();
                            changed |= ui.checkbox(&mut permissions.delete_workspace, "удалять").changed();
                            changed |= ui.checkbox(&mut permissions.rewrite_ai, "rewrite_text").changed();
                        });
                        if changed {
                            match agents::set_permissions(&self.root, &agent.id, permissions) {
                                Ok(()) => {
                                    self.status = format!("Права агента «{}» обновлены", agent.name);
                                    self.refresh_agents();
                                }
                                Err(error) => self.status = error,
                            }
                        }

                        ui.horizontal_wrapped(|ui| {
                            let toggle = if agent.enabled { "Отключить" } else { "Включить" };
                            if ui.button(toggle).clicked() {
                                match agents::set_enabled(&self.root, &agent.id, !agent.enabled) {
                                    Ok(()) => {
                                        self.status = format!("Состояние агента «{}» изменено", agent.name);
                                        self.refresh_agents();
                                    }
                                    Err(error) => self.status = error,
                                }
                            }
                            if ui.button("Перевыпустить токен").clicked() {
                                match agents::rotate_token(&self.root, &agent.id) {
                                    Ok(token) => {
                                        self.pairing = Some((agent.name.clone(), agent.id.clone(), token));
                                        self.status = "Старый токен отозван; скопируйте новый".into();
                                        self.refresh_agents();
                                    }
                                    Err(error) => self.status = error,
                                }
                            }
                            if ui.button("Удалить регистрацию").clicked() {
                                match agents::remove(&self.root, &agent.id) {
                                    Ok(()) => {
                                        self.status = format!("Регистрация «{}» удалена", agent.name);
                                        self.refresh_agents();
                                    }
                                    Err(error) => self.status = error,
                                }
                            }
                        });
                    });
                }
            });
    }

    fn show_audit(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Журнал действий MCP-агентов")
            .default_open(false)
            .show(ui, |ui| {
                ui.small("Локальный журнал содержит только время, агента, действие, относительный путь и success/failure. Содержимое файлов, токены, API-ключи и медицинский текст не записываются.");
                ui.small(format!("Файл: {}", audit::log_path(&self.root).display()));
                ui.separator();
                match audit::recent(&self.root, 100) {
                    Ok(entries) if entries.is_empty() => {
                        ui.label("Журнал пока пуст.");
                    }
                    Ok(entries) => {
                        for entry in entries {
                            ui.group(|ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.monospace(&entry.timestamp_utc);
                                    ui.strong(&entry.agent_name)
                                        .on_hover_text(format!("Agent ID: {}", entry.agent_id));
                                    ui.monospace(&entry.action);
                                    if entry.result == "success" {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(40, 160, 90),
                                            "success",
                                        );
                                    } else {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(190, 70, 70),
                                            "failure",
                                        );
                                    }
                                });
                                ui.monospace(&entry.path);
                            });
                        }
                    }
                    Err(error) => {
                        ui.colored_label(
                            egui::Color32::from_rgb(190, 70, 70),
                            format!("Не удалось прочитать журнал: {error}"),
                        );
                    }
                }
            });
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
                egui::CollapsingHeader::new("Настройки подключения и контекст")
                    .default_open(self.text.is_empty() || !self.settings.enabled)
                    .show(ui, |ui| {
                        ui.checkbox(&mut self.settings.enabled, "Включить обработку через ИИ");
                        let old_provider = self.settings.provider;
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut self.settings.provider, Provider::Openai, "OpenAI");
                            ui.selectable_value(&mut self.settings.provider, Provider::Gigachat, "GigaChat");
                        });
                        if old_provider != self.settings.provider {
                            self.settings.model = match self.settings.provider {
                                Provider::Openai => "gpt-4.1-mini",
                                Provider::Gigachat => "GigaChat",
                            }
                            .into();
                            self.key = espanso_ai::load_key(&self.root, self.settings.provider)
                                .unwrap_or_default();
                            self.reviewed = false;
                        }
                        ui.label("Идентификатор модели (можно изменить)");
                        ui.text_edit_singleline(&mut self.settings.model);
                        ui.label(match self.settings.provider {
                            Provider::Openai => "API-ключ OpenAI",
                            Provider::Gigachat => "Ключ авторизации GigaChat (Base64, без префикса Basic)",
                        });
                        ui.add(
                            egui::TextEdit::singleline(&mut self.key)
                                .password(true)
                                .desired_width(f32::INFINITY),
                        );
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Сохранить ключ на этом устройстве").clicked() {
                                self.status = espanso_ai::save_key(
                                    &self.root,
                                    self.settings.provider,
                                    &self.key,
                                )
                                .map(|_| "Ключ сохранён отдельно от настроек".into())
                                .unwrap_or_else(|e| e);
                            }
                            if ui.button("Удалить сохранённый ключ").clicked() {
                                self.status = espanso_ai::save_key(
                                    &self.root,
                                    self.settings.provider,
                                    "",
                                )
                                .map(|_| {
                                    self.key.clear();
                                    "Ключ удалён. Переменная окружения, если задана, сохраняет действие для MCP.".into()
                                })
                                .unwrap_or_else(|e| e);
                            }
                        });
                        ui.small("Без сохранения ключ действует до закрытия окна. Сохранённый ключ — отдельный локальный файл, не зашифрован; на Linux доступен владельцу (0600). Не пересылайте всю рабочую папку приложения.");
                        if self.settings.provider == Provider::Gigachat {
                            ui.label("Scope GigaChat");
                            ui.text_edit_singleline(&mut self.settings.gigachat_scope);
                            ui.label("Доверенный корневой сертификат PEM (полный путь, при необходимости)");
                            ui.text_edit_singleline(&mut self.settings.ca_file);
                        }
                        ui.label("Ваш контекст (тоже передаётся провайдеру)");
                        if ui
                            .add(
                                egui::TextEdit::multiline(&mut self.settings.context)
                                    .desired_rows(4)
                                    .desired_width(f32::INFINITY),
                            )
                            .changed()
                        {
                            self.reviewed = false;
                        }
                        ui.checkbox(
                            &mut self.settings.mcp_allow_rewrite,
                            "Разрешить отправку текста через MCP-инструмент rewrite_text",
                        );
                        ui.checkbox(
                            &mut self.settings.mcp_allow_workspace_write,
                            "Главный выключатель: разрешить зарегистрированным MCP-агентам изменять триггеры и Rhai-скрипты",
                        );
                        ui.small("Даже при включённом главном разрешении конкретному агенту нужны отдельные права. Запись ограничена match/**/*.yml|yaml и scripts/**/*.rhai; YAML/Rhai проверяются до записи.");
                        if ui.button("Сохранить настройки ИИ").clicked() {
                            self.status = self
                                .settings
                                .save(&self.root)
                                .map(|_| {
                                    self.saved_settings =
                                        serde_json::to_string(&self.settings).unwrap_or_default();
                                    "Настройки сохранены".into()
                                })
                                .unwrap_or_else(|e| e);
                        }
                    });

                self.show_agent_manager(ui);
                self.show_audit(ui);
                ui.separator();
                ui.label("Текст для отправки — удалите оставшиеся идентификаторы вручную");
                if ui
                    .add(
                        egui::TextEdit::multiline(&mut self.text)
                            .desired_rows(7)
                            .desired_width(f32::INFINITY),
                    )
                    .changed()
                {
                    self.reviewed = false;
                    self.output.clear();
                }
                if ui.button("Повторить локальную маскировку").clicked() {
                    self.text = redact(&self.text);
                    self.settings.context = redact(&self.settings.context);
                    self.reviewed = false;
                    self.output.clear();
                }
                ui.small("Маскировка эвристическая: ФИО, даты, контакты и номера могут быть пропущены. Инструкция модели не заменяет удаление данных до отправки.");
                egui::CollapsingHeader::new("Точный текст и контекст после маскировки").show(ui, |ui| {
                    ui.label(espanso_ai::CLINICAL_RULES);
                    ui.separator();
                    ui.label(redact(&self.settings.context));
                    ui.separator();
                    ui.label(redact(&self.text));
                });
                ui.checkbox(&mut self.reviewed, "Проверил отправляемый текст и контекст: персональных данных нет");
                if ui
                    .add_enabled(
                        self.reviewed && self.settings.enabled && !self.text.trim().is_empty(),
                        egui::Button::new("Переформулировать"),
                    )
                    .clicked()
                {
                    self.output.clear();
                    self.status = "Обработка… Можно продолжать работу в других приложениях.".into();
                    let settings = self.settings.clone();
                    let key = self.key.clone();
                    let text = self.text.clone();
                    let (tx, rx) = mpsc::channel();
                    self.pending = Some(rx);
                    let context = ui.ctx().clone();
                    std::thread::spawn(move || {
                        let _ = tx.send(espanso_ai::rewrite(&settings, &key, &text));
                        context.request_repaint();
                    });
                }
            });
            if self.pending.is_some() {
                ui.spinner();
            }
            ui.separator();
            ui.label(&self.status);
            if !self.output.is_empty() {
                ui.heading("Результат — проверьте и отредактируйте");
                ui.add(
                    egui::TextEdit::multiline(&mut self.output)
                        .desired_rows(8)
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Копировать результат").clicked() {
                        ui.ctx().copy_text(self.output.clone());
                        self.status = "Скопировано. Вернитесь в исходное приложение и вставьте Ctrl+V.".into();
                    }
                    #[cfg(target_os = "linux")]
                    if let Some(target) = self.target {
                        if ui
                            .add_enabled(
                                self.insert_pending.is_none(),
                                egui::Button::new("Вставить в исходное окно"),
                            )
                            .clicked()
                        {
                            ui.ctx().copy_text(self.output.clone());
                            let (tx, rx) = mpsc::channel();
                            self.insert_pending = Some(rx);
                            std::thread::spawn(move || {
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

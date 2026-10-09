use super::{
    open_config::{studio_candidates, ConfigPathProvider},
    selection_match::SelectedTextProvider,
};
use crate::{
    event::{ui::ShowTextEvent, Event, EventType},
    process::Middleware,
};
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
};

static WINDOW_OPEN: AtomicBool = AtomicBool::new(false);
pub struct AiRewriteMiddleware<'a> {
    selection: &'a dyn SelectedTextProvider,
    paths: &'a dyn ConfigPathProvider,
}
impl<'a> AiRewriteMiddleware<'a> {
    pub fn new(selection: &'a dyn SelectedTextProvider, paths: &'a dyn ConfigPathProvider) -> Self {
        Self { selection, paths }
    }
    fn launch(&self) -> Result<(), String> {
        let text = self
            .selection
            .get_selected_text()
            .filter(|s| !s.trim().is_empty())
            .ok_or("Не удалось получить выделенный текст")?;
        if text.len() > 24_000 {
            return Err("Выделите не более 24 КБ текста".into());
        }
        let exe = std::env::current_exe().map_err(|_| "Не найден путь приложения")?;
        let root = exe.parent().ok_or("Не найден каталог приложения")?;
        let studio = studio_candidates(root)
            .into_iter()
            .find(|p| p.is_file())
            .ok_or("Match Studio не найден рядом с движком")?;
        let mut command = Command::new(studio);
        command
            .arg("--ai-selection")
            .arg("--config-dir")
            .arg(self.paths.get_config_path());
        // Capture the destination BEFORE opening the review window. Text never goes in argv.
        #[cfg(target_os = "linux")]
        if std::env::var_os("DISPLAY").is_some() {
            if let Ok(output) = Command::new("xdotool").arg("getactivewindow").output() {
                if output.status.success() {
                    if let Ok(id) = String::from_utf8(output.stdout)
                        .unwrap_or_default()
                        .trim()
                        .parse::<u64>()
                    {
                        command.arg("--ai-target-window").arg(id.to_string());
                    }
                }
            }
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|_| "Не удалось открыть окно ИИ")?;
        // Reap the child and keep the expansion engine responsive during the entire request.
        std::thread::spawn(move || {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
            WINDOW_OPEN.store(false, Ordering::Release);
        });
        Ok(())
    }
}
impl Middleware for AiRewriteMiddleware<'_> {
    fn name(&self) -> &'static str {
        "ai_rewrite"
    }
    fn next(&self, event: Event, _: &mut dyn FnMut(Event)) -> Event {
        if !matches!(event.etype, EventType::AiRewriteRequested) {
            return event;
        }
        if WINDOW_OPEN.swap(true, Ordering::AcqRel) {
            return Event::caused_by(event.source_id, EventType::NOOP);
        }
        match self.launch() {
            Ok(()) => Event::caused_by(event.source_id, EventType::NOOP),
            Err(text) => {
                WINDOW_OPEN.store(false, Ordering::Release);
                Event::caused_by(
                    event.source_id,
                    EventType::ShowText(ShowTextEvent {
                        title: "rEspanso ИИ".into(),
                        text,
                    }),
                )
            }
        }
    }
}

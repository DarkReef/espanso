use eframe::egui;
use std::{
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};
use sysinfo::{PidExt, ProcessExt, System, SystemExt};

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

pub struct RuntimeMonitor {
    system: System,
    running: bool,
    process_ids: Vec<u32>,
    last_refresh: Option<Instant>,
    last_change: Instant,
}

impl RuntimeMonitor {
    pub fn new() -> Self {
        Self {
            system: System::new(),
            running: false,
            process_ids: Vec::new(),
            last_refresh: None,
            last_change: Instant::now(),
        }
    }

    pub fn update(&mut self, context: &egui::Context) {
        context.request_repaint_after(REFRESH_INTERVAL);
        if self
            .last_refresh
            .is_some_and(|updated| updated.elapsed() < REFRESH_INTERVAL)
        {
            return;
        }

        self.system.refresh_processes();
        let mut process_ids = self
            .system
            .processes()
            .iter()
            .filter_map(|(pid, process)| {
                is_respanso_process(process.name()).then_some(pid.as_u32())
            })
            .collect::<Vec<_>>();
        process_ids.sort_unstable();

        let running = !process_ids.is_empty();
        if running != self.running || process_ids != self.process_ids {
            self.last_change = Instant::now();
        }
        self.running = running;
        self.process_ids = process_ids;
        self.last_refresh = Some(Instant::now());
    }

    pub fn running(&self) -> bool {
        self.running
    }

    pub fn process_ids(&self) -> &[u32] {
        &self.process_ids
    }

    pub fn seconds_since_change(&self) -> u64 {
        self.last_change.elapsed().as_secs()
    }

    pub fn restart_button(&mut self, ui: &mut egui::Ui) -> Option<String> {
        let clicked = ui
            .button("(Пере)запустить rEspanso")
            .on_hover_text(
                "Завершает найденные процессы rEspanso и запускает нативную portable-версию рядом с Match Studio",
            )
            .clicked();
        if !clicked {
            return None;
        }

        let result = self.restart_respanso();
        self.last_refresh = None;
        Some(match result {
            Ok(message) => message,
            Err(message) => format!("Ошибка управления rEspanso: {message}"),
        })
    }

    fn restart_respanso(&mut self) -> Result<String, String> {
        self.system.refresh_processes();
        let stopped = self
            .system
            .processes()
            .values()
            .filter(|process| is_respanso_process(process.name()))
            .filter(|process| process.kill())
            .count();

        if stopped > 0 {
            thread::sleep(Duration::from_millis(300));
        }

        let executable = find_respanso_executable()?;
        let working_directory = executable
            .parent()
            .ok_or_else(|| "Не удалось определить папку запуска rEspanso".to_owned())?;

        let mut command = Command::new(&executable);
        command.current_dir(working_directory);
        match launch_mode(&executable) {
            LaunchMode::NativePortable => {}
            LaunchMode::CoreOrDaemon => {
                command.arg("launcher");
            }
        }
        command
            .spawn()
            .map_err(|error| format!("Не удалось запустить {}: {error}", executable.display()))?;

        Ok(if stopped == 0 {
            format!("rEspanso запущен: {}", executable.display())
        } else {
            format!(
                "rEspanso перезапущен: остановлено процессов {stopped}; запуск {}",
                executable.display()
            )
        })
    }
}

impl Default for RuntimeMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LaunchMode {
    NativePortable,
    CoreOrDaemon,
}

fn launch_mode(executable: &Path) -> LaunchMode {
    let name = executable
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name == "respanso.exe" || name == "respanso" || name.contains("portable") {
        LaunchMode::NativePortable
    } else {
        LaunchMode::CoreOrDaemon
    }
}

fn find_respanso_executable() -> Result<PathBuf, String> {
    let current_executable = std::env::current_exe()
        .map_err(|error| format!("Не удалось определить путь Match Studio: {error}"))?;
    let root = current_executable
        .parent()
        .ok_or_else(|| "Не удалось определить корень portable-сборки".to_owned())?;

    executable_candidates()
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            format!(
                "Не найден исполняемый файл rEspanso рядом с Match Studio: {}",
                root.display()
            )
        })
}

#[cfg(target_os = "windows")]
fn executable_candidates() -> &'static [&'static str] {
    &[
        "rEspanso.exe",
        "respanso.exe",
        "rEspanso-core.exe",
        "respanso-core.exe",
        "rEspansod.exe",
        "respansod.exe",
    ]
}

#[cfg(not(target_os = "windows"))]
fn executable_candidates() -> &'static [&'static str] {
    &[
        "rEspanso",
        "respanso",
        "rEspanso-core",
        "respanso-core",
        "rEspansod",
        "respansod",
    ]
}

fn is_respanso_process(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    normalized.contains("respanso") && !normalized.contains("match studio")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_respanso_process_names() {
        assert!(is_respanso_process("rEspansod.exe"));
        assert!(is_respanso_process("rEspanso.exe"));
        assert!(is_respanso_process("rEspanso-core.exe"));
        assert!(is_respanso_process("RESPANSO-service.exe"));
        assert!(!is_respanso_process("espansod.exe"));
        assert!(!is_respanso_process("espanso.exe"));
        assert!(!is_respanso_process("rEspanso Match Studio.exe"));
    }

    #[test]
    fn native_portable_launcher_is_checked_first() {
        let candidates = executable_candidates();
        assert_eq!(candidates[0].to_ascii_lowercase(), "respanso.exe");
    }

    #[test]
    fn native_portable_launcher_uses_default_launcher_mode() {
        assert_eq!(
            launch_mode(Path::new("rEspanso.exe")),
            LaunchMode::NativePortable
        );
        assert_eq!(
            launch_mode(Path::new("rEspanso-core.exe")),
            LaunchMode::CoreOrDaemon
        );
    }

    #[test]
    fn candidate_path_is_resolved_relative_to_root() {
        let root = Path::new("portable-root");
        assert_eq!(
            root.join("rEspanso.exe"),
            PathBuf::from("portable-root/rEspanso.exe")
        );
    }
}

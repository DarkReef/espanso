use eframe::egui;
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};
#[cfg(not(target_os = "linux"))]
use std::thread;
use sysinfo::{PidExt, ProcessExt, System, SystemExt};

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

pub struct RuntimeMonitor {
    config_root: PathBuf,
    system: System,
    running: bool,
    process_ids: Vec<u32>,
    last_refresh: Option<Instant>,
    last_change: Instant,
}

impl RuntimeMonitor {
    pub fn new(config_root: PathBuf) -> Self {
        Self {
            config_root,
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
        let current_pid = std::process::id();
        let mut process_ids = self
            .system
            .processes()
            .iter()
            .filter_map(|(pid, process)| {
                let pid = pid.as_u32();
                (pid != current_pid && is_respanso_runtime_process(process.name())).then_some(pid)
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
            .on_hover_text(if cfg!(target_os = "linux") {
                "Перезапускает portable rEspanso через service restart --unmanaged, не завершая процессы вслепую"
            } else {
                "Перезапускает найденный runtime rEspanso"
            })
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

    #[cfg(target_os = "linux")]
    fn restart_respanso(&mut self) -> Result<String, String> {
        let executable = find_respanso_executable()?;
        let executable_root = executable
            .parent()
            .ok_or_else(|| "Не удалось определить папку запуска rEspanso".to_owned())?;
        if self.config_root.as_os_str().is_empty() {
            return Err("Не определён корень конфигурации rEspanso".into());
        }

        let package_dir = self.config_root.join("packages");
        let runtime_dir = self.config_root.join("runtime");
        std::fs::create_dir_all(&package_dir)
            .map_err(|error| format!("Не удалось создать packages/: {error}"))?;
        std::fs::create_dir_all(&runtime_dir)
            .map_err(|error| format!("Не удалось создать runtime/: {error}"))?;

        // The Astra/X11 portable build is deliberately unmanaged. Use the
        // service lifecycle instead of killing processes by their executable
        // name: the daemon owns the worker and already waits for its locks to
        // be released before starting the next generation.
        let status = Command::new(&executable)
            .current_dir(executable_root)
            .arg("--config_dir")
            .arg(&self.config_root)
            .arg("--package_dir")
            .arg(&package_dir)
            .arg("--runtime_dir")
            .arg(&runtime_dir)
            .arg("service")
            .arg("restart")
            .arg("--unmanaged")
            .status()
            .map_err(|error| {
                format!(
                    "Не удалось выполнить service restart через {}: {error}",
                    executable.display()
                )
            })?;

        if !status.success() {
            return Err(format!(
                "service restart завершился с кодом {}. Проверьте runtime/startup.log и runtime/espanso.log",
                status.code().map_or_else(|| "signal".to_owned(), |code| code.to_string())
            ));
        }

        // The service restart does not kill the tray. If the tray had already
        // disappeared, restore it only for the canonical portable layout where
        // scripts and the active config root are the same directory.
        if executable_root == self.config_root {
            let tray_script = executable_root.join("start-tray.sh");
            if tray_script.is_file() {
                let _ = Command::new(&tray_script)
                    .current_dir(executable_root)
                    .spawn();
            }
        }

        Ok(format!(
            "rEspanso перезапущен через unmanaged service: {}",
            executable.display()
        ))
    }

    #[cfg(not(target_os = "linux"))]
    fn restart_respanso(&mut self) -> Result<String, String> {
        self.system.refresh_processes();
        let current_pid = std::process::id();
        let stopped = self
            .system
            .processes()
            .iter()
            .filter(|(pid, process)| {
                pid.as_u32() != current_pid && is_respanso_runtime_process(process.name())
            })
            .filter(|(_, process)| process.kill())
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
        Self::new(PathBuf::new())
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

fn is_respanso_runtime_process(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    normalized.contains("respanso")
        && !normalized.contains("match")
        && !normalized.contains("studio")
        && !normalized.contains("tray")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_respanso_runtime_process_names() {
        assert!(is_respanso_runtime_process("rEspansod.exe"));
        assert!(is_respanso_runtime_process("rEspanso.exe"));
        assert!(is_respanso_runtime_process("rEspanso-core.exe"));
        assert!(is_respanso_runtime_process("RESPANSO-service.exe"));
        assert!(!is_respanso_runtime_process("espansod.exe"));
        assert!(!is_respanso_runtime_process("espanso.exe"));
        assert!(!is_respanso_runtime_process("rEspanso Match Studio.exe"));
        assert!(!is_respanso_runtime_process("rEspanso-Match-S"));
        assert!(!is_respanso_runtime_process("rEspanso-Tray"));
    }

    #[test]
    fn native_portable_launcher_is_checked_first() {
        let first = executable_candidates()[0].to_ascii_lowercase();
        assert!(first.contains("respanso"));
        assert!(!first.contains("core"));
        assert!(!first.contains("daemon"));
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

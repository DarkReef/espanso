#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::SystemTime,
};

fn main() {
    install_panic_hook();

    if let Err(error) = launch() {
        report_fatal_error(&format!(
            "Не удалось запустить rEspanso Match Studio.\n\n{error}\n\nПодробности записаны в:\n{}",
            startup_log_path().display()
        ));
    };
}

fn launch() -> Result<(), String> {
    let config_root = parse_config_root().unwrap_or_else(default_config_root);
    initialize_match_directory(&config_root)?;

    append_startup_log(&format!(
        "Запуск. EXE: {}; конфигурация: {}; текущая папка: {}",
        std::env::current_exe().map_or_else(
            |_| "не определён".to_owned(),
            |path| path.display().to_string()
        ),
        config_root.display(),
        std::env::current_dir().map_or_else(
            |_| "не определена".to_owned(),
            |path| path.display().to_string()
        )
    ));

    espanso_editor::run(config_root).map_err(|error| error.to_string())
}

fn parse_config_root() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--config-dir" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

fn default_config_root() -> PathBuf {
    portable_config_root()
        .or_else(|| std::env::var_os("ESPANSO_CONFIG_DIR").map(PathBuf::from))
        .or_else(|| dirs::config_dir().map(|path| path.join("espanso")))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn portable_config_root() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let directory = executable.parent()?;
    let portable_directory = directory.join("portable");
    let is_named_standalone = executable
        .file_stem()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("rEspanso Match Studio"));

    (portable_directory.is_dir() || is_named_standalone).then(|| portable_directory.join("config"))
}

fn portable_config_for(executable: &Path) -> Option<PathBuf> {
    executable
        .parent()
        .map(|directory| directory.join("portable").join("config"))
}

fn initialize_match_directory(config_root: &Path) -> Result<(), String> {
    let match_directory = config_root.join("match");
    fs::create_dir_all(&match_directory).map_err(|error| {
        format!(
            "Не удалось создать папку правил {}: {error}",
            match_directory.display()
        )
    })?;

    let has_yaml = fs::read_dir(&match_directory)
        .map_err(|error| {
            format!(
                "Не удалось прочитать папку правил {}: {error}",
                match_directory.display()
            )
        })?
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml")
                })
        });

    if !has_yaml {
        let base_file = match_directory.join("base.yml");
        fs::write(
            &base_file,
            "matches:\n  - label: \"Portable-конфигурация готова\"\n    trigger: \":respanso_example\"\n    replace: \"rEspanso Match Studio готов к работе\"\n    disabled: true\n",
        )
        .map_err(|error| {
            format!(
                "Не удалось создать начальный YAML-файл {}: {error}",
                base_file.display()
            )
        })?;
    }

    Ok(())
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("неизвестная критическая ошибка");
        let location = info.location().map_or_else(
            || "место ошибки не определено".to_owned(),
            |location| {
                format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            },
        );
        let message = format!(
            "Критическая ошибка rEspanso Match Studio.\n\n{payload}\n\n{location}\n\nЖурнал: {}",
            startup_log_path().display()
        );
        append_startup_log(&message);
        show_error_dialog(&message);
    }));
}

fn report_fatal_error(message: &str) {
    append_startup_log(message);
    show_error_dialog(message);
}

fn append_startup_log(message: &str) {
    let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(startup_log_path())
    else {
        return;
    };

    let _ = writeln!(file, "[{:?}] {message}", SystemTime::now());
}

fn startup_log_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|directory| directory.join("rEspanso Match Studio.log"))
        })
        .unwrap_or_else(|| PathBuf::from("rEspanso Match Studio.log"))
}

#[cfg(windows)]
fn show_error_dialog(message: &str) {
    use windows::{
        core::HSTRING,
        Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK},
    };

    let text = HSTRING::from(message);
    let title = HSTRING::from("rEspanso Match Studio — ошибка запуска");
    unsafe {
        let _ = MessageBoxW(None, &text, &title, MB_OK | MB_ICONERROR);
    };
}

#[cfg(not(windows))]
fn show_error_dialog(message: &str) {
    eprintln!("{message}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_root_is_relative_to_executable_directory() {
        let executable = Path::new("bundle").join("espanso-editor");
        assert_eq!(
            portable_config_for(&executable),
            Some(PathBuf::from("bundle/portable/config"))
        );
    }

    #[test]
    fn portable_root_supports_bundle_paths_with_spaces() {
        let executable = Path::new("rEspanso Match Studio").join("rEspanso Match Studio.exe");
        assert_eq!(
            portable_config_for(&executable),
            Some(PathBuf::from("rEspanso Match Studio/portable/config"))
        );
    }

    #[test]
    fn portable_root_supports_cyrillic_bundle_paths() {
        let executable = Path::new("Мои программы").join("rEspanso Match Studio.exe");
        assert_eq!(
            portable_config_for(&executable),
            Some(PathBuf::from("Мои программы/portable/config"))
        );
    }
}

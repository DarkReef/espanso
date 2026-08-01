#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{
    env,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::{SystemTime, UNIX_EPOCH},
};

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code.clamp(0, u8::MAX as i32) as u8),
        Err(error) => {
            report_fatal_error(&error);
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<i32, String> {
    let executable = env::current_exe()
        .map_err(|error| format!("unable to resolve portable executable path: {error}"))?;
    let root = executable
        .parent()
        .ok_or_else(|| "portable executable has no parent directory".to_owned())?;

    let core = root.join(core_binary_name());
    if !core.is_file() {
        return Err(format!(
            "rEspanso core executable is missing: {}",
            core.display()
        ));
    }

    let paths = PortablePaths::new(root);
    paths
        .ensure()
        .map_err(|error| format!("unable to prepare portable directories: {error}"))?;

    let user_args = env::args_os().skip(1).collect::<Vec<OsString>>();
    let mut command = Command::new(core);
    command
        .current_dir(root)
        .arg("--config_dir")
        .arg(&paths.config_root)
        .arg("--runtime_dir")
        .arg(&paths.runtime)
        .arg("--package_dir")
        .arg(&paths.packages);

    if user_args.is_empty() {
        command.arg("launcher");
    } else {
        command.args(user_args);
    }

    let status = command
        .status()
        .map_err(|error| format!("unable to start rEspanso core: {error}"))?;
    let code = status.code().unwrap_or(1);
    if code != 0 {
        return Err(format!("rEspanso core stopped with exit code {code}"));
    }
    Ok(code)
}

fn report_fatal_error(error: &str) {
    let root = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let runtime = root.join("runtime");
    let _ = fs::create_dir_all(&runtime);
    let log_path = runtime.join("rEspanso-bootstrap.log");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    if let Ok(mut log) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let _ = writeln!(log, "[{timestamp}] {error}");
    }
    show_error_message(&format!(
        "rEspanso не удалось запустить.\n\n{error}\n\nДиагностика: {}",
        log_path.display()
    ));
}

#[cfg(target_os = "windows")]
fn show_error_message(message: &str) {
    use std::ffi::c_void;

    const MB_OK: u32 = 0;
    const MB_ICONERROR: u32 = 0x10;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(
            window: *mut c_void,
            text: *const u16,
            caption: *const u16,
            kind: u32,
        ) -> i32;
    }

    let title = "rEspanso\0".encode_utf16().collect::<Vec<_>>();
    let message = format!("{message}\0").encode_utf16().collect::<Vec<_>>();
    unsafe {
        let _ = MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn show_error_message(message: &str) {
    eprintln!("{message}");
}

#[cfg(target_os = "windows")]
fn core_binary_name() -> &'static str {
    "rEspanso-core.exe"
}

#[cfg(not(target_os = "windows"))]
fn core_binary_name() -> &'static str {
    "rEspanso-core"
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PortablePaths {
    config_root: PathBuf,
    config: PathBuf,
    matches: PathBuf,
    runtime: PathBuf,
    packages: PathBuf,
}

impl PortablePaths {
    fn new(root: &Path) -> Self {
        Self {
            config_root: root.to_path_buf(),
            config: root.join("config"),
            matches: root.join("match"),
            runtime: root.join("runtime"),
            packages: root.join("packages"),
        }
    }

    fn ensure(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.config)?;
        fs::create_dir_all(&self.matches)?;
        fs::create_dir_all(&self.runtime)?;
        fs::create_dir_all(&self.packages)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_paths_are_relative_to_executable_directory() {
        let paths = PortablePaths::new(Path::new("bundle"));
        assert_eq!(paths.config_root, PathBuf::from("bundle"));
        assert_eq!(paths.config, PathBuf::from("bundle/config"));
        assert_eq!(paths.matches, PathBuf::from("bundle/match"));
        assert_eq!(paths.runtime, PathBuf::from("bundle/runtime"));
        assert_eq!(paths.packages, PathBuf::from("bundle/packages"));
        assert_ne!(paths.config, paths.config_root.join("config/config"));
    }
}

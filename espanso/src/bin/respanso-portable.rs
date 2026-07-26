#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code.clamp(0, u8::MAX as i32) as u8),
        Err(_) => ExitCode::FAILURE,
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
        return Err(format!("rEspanso core executable is missing: {}", core.display()));
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
        .arg(&paths.config)
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
    Ok(status.code().unwrap_or(1))
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
    config: PathBuf,
    runtime: PathBuf,
    packages: PathBuf,
}

impl PortablePaths {
    fn new(root: &Path) -> Self {
        let portable = root.join("portable");
        Self {
            config: portable.join("config"),
            runtime: portable.join("runtime"),
            packages: portable.join("packages"),
        }
    }

    fn ensure(&self) -> std::io::Result<()> {
        fs::create_dir_all(self.config.join("match"))?;
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
        assert_eq!(paths.config, PathBuf::from("bundle/portable/config"));
        assert_eq!(paths.runtime, PathBuf::from("bundle/portable/runtime"));
        assert_eq!(paths.packages, PathBuf::from("bundle/portable/packages"));
    }
}

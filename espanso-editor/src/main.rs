#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

fn main() -> eframe::Result {
    let config_root = parse_config_root().unwrap_or_else(default_config_root);
    espanso_editor::run(config_root)
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
    let config = portable_config_for(&executable)?;
    config.is_dir().then_some(config)
}

fn portable_config_for(executable: &Path) -> Option<PathBuf> {
    executable
        .parent()
        .map(|directory| directory.join("portable").join("config"))
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
}

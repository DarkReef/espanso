use std::path::PathBuf;

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
    std::env::var_os("ESPANSO_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(portable_config_root)
        .or_else(|| dirs::config_dir().map(|path| path.join("espanso")))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn portable_config_root() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let directory = executable.parent()?;
    let config = directory.join(".espanso");
    config.is_dir().then_some(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_root_is_relative_to_executable_directory() {
        let executable = std::path::Path::new("bundle").join("espanso-editor");
        assert_eq!(
            executable.parent().map(|path| path.join(".espanso")),
            Some(PathBuf::from("bundle/.espanso"))
        );
    }
}

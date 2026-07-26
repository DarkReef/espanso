pub mod app;
pub mod runtime;
pub mod settings;
pub mod workspace;

use std::path::PathBuf;

pub fn run(config_root: PathBuf) -> eframe::Result {
    app::run(config_root)
}

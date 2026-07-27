pub mod app;
pub mod rhai_lab;
#[allow(clippy::pedantic)]
pub mod runtime;
#[allow(clippy::pedantic)]
pub mod settings;
pub mod workspace;
pub mod yaml_imports;

use std::path::PathBuf;

pub fn run(config_root: PathBuf) -> eframe::Result {
    app::run(config_root)
}

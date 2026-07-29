pub mod app;
pub mod config_transfer;
pub mod diagnostics;
pub mod dynamic_variables;
pub mod file_monitor;
pub mod global_variables;
pub mod rhai_lab;
#[allow(clippy::pedantic)]
pub mod runtime;
#[allow(clippy::pedantic)]
pub mod settings;
pub mod storm_logo;
pub mod workspace;
pub mod yaml_imports;

use std::path::PathBuf;

pub fn run(config_root: PathBuf) -> eframe::Result {
    app::run(config_root)
}

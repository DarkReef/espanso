pub mod app;
#[allow(clippy::pedantic)]
pub mod runtime;
#[allow(clippy::pedantic)]
pub mod settings;
pub mod workspace;

use std::path::PathBuf;

pub fn run(config_root: PathBuf) -> eframe::Result {
    app::run(config_root)
}

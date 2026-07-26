/*
 * This file is part of espanso.
 *
 * Copyright (C) 2019-2021 Federico Terzi
 *
 * espanso is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

use super::super::Middleware;
use crate::event::{Event, EventType};
use log::error;
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

pub trait ConfigPathProvider {
    fn get_config_path(&self) -> &Path;
}

pub struct ConfigMiddleware<'a> {
    provider: &'a dyn ConfigPathProvider,
}

impl<'a> ConfigMiddleware<'a> {
    pub fn new(provider: &'a dyn ConfigPathProvider) -> Self {
        Self { provider }
    }
}

impl Middleware for ConfigMiddleware<'_> {
    fn name(&self) -> &'static str {
        "open_config"
    }

    fn next(&self, event: Event, _dispatch: &mut dyn FnMut(Event)) -> Event {
        match event.etype {
            EventType::ShowConfigFolder => {
                open_config_folder(self.provider.get_config_path());
                Event::caused_by(event.source_id, EventType::NOOP)
            }
            EventType::ShowMatchStudio => {
                if let Err(message) = launch_match_studio(self.provider.get_config_path()) {
                    error!("{message}");
                }
                Event::caused_by(event.source_id, EventType::NOOP)
            }
            _ => event,
        }
    }
}

fn open_config_folder(config_path: &Path) {
    let config_path = match config_path.canonicalize() {
        Ok(path) => path,
        Err(err) => {
            error!("unable to canonicalize the config path: {err}");
            config_path.to_owned()
        }
    };

    let program = match env::consts::OS {
        "macos" => "open",
        "windows" => "explorer",
        "linux" => "xdg-open",
        _ => panic!("Unsupported OS"),
    };

    if let Err(err) = Command::new(program).arg(config_path).spawn() {
        error!("unable to open config folder: {err}");
    }
}

fn launch_match_studio(config_path: &Path) -> Result<(), String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("unable to determine rEspanso executable path: {error}"))?;
    let root = executable
        .parent()
        .ok_or_else(|| "unable to determine rEspanso portable root".to_owned())?;

    let studio = studio_candidates(root)
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            format!(
                "rEspanso Match Studio was not found beside {}",
                executable.display()
            )
        })?;

    Command::new(&studio)
        .arg("--config-dir")
        .arg(config_path)
        .current_dir(root)
        .spawn()
        .map_err(|error| format!("unable to start {}: {error}", studio.display()))?;
    Ok(())
}

fn studio_candidates(root: &Path) -> Vec<PathBuf> {
    studio_binary_names()
        .iter()
        .map(|name| root.join(name))
        .collect()
}

#[cfg(target_os = "windows")]
fn studio_binary_names() -> &'static [&'static str] {
    &["rEspanso Match Studio.exe", "espanso-editor.exe"]
}

#[cfg(not(target_os = "windows"))]
fn studio_binary_names() -> &'static [&'static str] {
    &["rEspanso-Match-Studio", "espanso-editor"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_facing_studio_name_is_checked_first() {
        let candidates = studio_candidates(Path::new("portable"));
        assert!(candidates[0]
            .to_string_lossy()
            .to_ascii_lowercase()
            .contains("respanso"));
    }
}

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

use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
use crossbeam::{channel::Sender, select};
use log::{error, info, warn};
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};

const WATCHER_NOTIFY_DELAY: Duration = Duration::from_millis(500);
const WATCHER_DEBOUNCE_DURATION: Duration = Duration::from_millis(1000);
const FILE_STABILITY_DELAY: Duration = Duration::from_millis(250);
const WATCHER_RETRY_DELAY: Duration = Duration::from_secs(1);

pub fn initialize_and_spawn(config_dir: &Path, watcher_notify: Sender<()>) -> Result<()> {
    let config_dir = config_dir.to_path_buf();
    let debounce_root = config_dir.clone();
    let (debounce_tx, debounce_rx) = crossbeam::channel::unbounded();

    thread::Builder::new()
        .name("watcher".to_string())
        .spawn(move || watcher_main(&config_dir, debounce_tx))
        .context("unable to spawn watcher thread")?;

    thread::Builder::new()
        .name("watcher-debouncer".to_string())
        .spawn(move || debouncer_main(debounce_rx, &watcher_notify, &debounce_root))
        .context("unable to spawn watcher debouncer thread")?;

    Ok(())
}

fn watcher_main(config_dir: &Path, debounce_tx: Sender<()>) {
    loop {
        if let Err(error) = watcher_session(config_dir, &debounce_tx) {
            error!("config watcher stopped: {error}; retrying");
            thread::sleep(WATCHER_RETRY_DELAY);
        }
    }
}

fn watcher_session(config_dir: &Path, debounce_tx: &Sender<()>) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher: RecommendedWatcher =
        Watcher::new(tx, WATCHER_NOTIFY_DELAY).context("unable to create file watcher")?;
    watcher
        .watch(config_dir, RecursiveMode::Recursive)
        .context("unable to start file watcher")?;
    info!("watching for changes in path: {}", config_dir.display());

    loop {
        let event = rx.recv().context("file watcher event channel closed")?;
        if event_should_reload(&event) {
            debounce_tx
                .send(())
                .context("unable to send watcher event to debouncer")?;
        }
    }
}

fn event_should_reload(event: &DebouncedEvent) -> bool {
    match event {
        DebouncedEvent::Create(path)
        | DebouncedEvent::Write(path)
        | DebouncedEvent::Remove(path) => path_should_reload(path),
        DebouncedEvent::Rename(old, new) => path_should_reload(old) || path_should_reload(new),
        _ => false,
    }
}

fn path_should_reload(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension == "yml" || extension == "yaml" {
        !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with('.'))
    } else {
        extension.is_empty()
    }
}

#[allow(clippy::too_many_lines)]
fn debouncer_main(
    debounce_rx: crossbeam::channel::Receiver<()>,
    watcher_notify: &Sender<()>,
    config_root: &Path,
) {
    let mut pending = false;
    loop {
        select! {
          recv(debounce_rx) -> event => {
            if event.is_err() {
                warn!("watcher debouncer input channel closed");
                return;
            }
            pending = true;
          },
          default(WATCHER_DEBOUNCE_DURATION) => {
            if !pending {
                continue;
            }
            let first = config_fingerprint(config_root);
            thread::sleep(FILE_STABILITY_DELAY);
            let second = config_fingerprint(config_root);
            if first == second {
                if let Err(error) = watcher_notify.send(()) {
                    error!("unable to send stable config change event: {error}");
                    return;
                }
                pending = false;
            } else {
                info!("configuration files are still changing; delaying reload");
            }
          },
        }
    }
}

fn config_fingerprint(root: &Path) -> u64 {
    let mut paths = Vec::new();
    collect_yaml_paths(root, &mut paths);
    paths.sort();
    let mut hasher = DefaultHasher::new();
    for path in paths {
        path.hash(&mut hasher);
        match fs::read(&path) {
            Ok(content) => content.hash(&mut hasher),
            Err(error) => error.kind().hash(&mut hasher),
        }
    }
    hasher.finish()
}

fn collect_yaml_paths(root: &Path, paths: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_yaml_paths(&path, paths);
        } else if path_should_reload(&path) {
            paths.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_to_disabled_still_requests_reload() {
        assert!(event_should_reload(&DebouncedEvent::Rename(
            PathBuf::from("match/base.yml"),
            PathBuf::from("match/base.yml.disabled"),
        )));
    }

    #[test]
    fn fingerprint_changes_after_yaml_write() {
        let temp = tempdir::TempDir::new("respanso-watcher").expect("temp dir");
        fs::create_dir_all(temp.path().join("match")).expect("match dir");
        let file = temp.path().join("match/base.yml");
        fs::write(&file, "matches: []\n").expect("initial write");
        let before = config_fingerprint(temp.path());
        fs::write(&file, "matches:\n  - trigger: \":a\"\n    replace: a\n").expect("second write");
        assert_ne!(before, config_fingerprint(temp.path()));
    }
}

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content, encoding="utf-8", newline="\n")


def replace(path: str, old: str, new: str, count: int = -1) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    if old not in text:
        if new in text:
            return
        raise RuntimeError(f"anchor not found in {path}: {old[:120]!r}")
    updated = text.replace(old, new, count)
    target.write_text(updated, encoding="utf-8", newline="\n")


# Match Studio: identical RegExp causes are valid selection alternatives too.
replace(
    "espanso-editor/src/workspace.rs",
    "use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};",
    "use std::collections::{BTreeMap, BTreeSet, HashSet};",
)
replace(
    "espanso-editor/src/workspace.rs",
    """        let rules = self.rules();
        let mut causes: HashMap<String, Vec<RuleId>> = HashMap::new();
        for rule in &rules {
""",
    """        let rules = self.rules();
        for rule in &rules {
""",
)
replace(
    "espanso-editor/src/workspace.rs",
    """                    // Identical simple triggers intentionally open the selection window.
                    // Only duplicate RegExp causes are diagnostic.
""",
    """                    // Identical triggers intentionally open the selection window.
""",
)
replace(
    "espanso-editor/src/workspace.rs",
    """                    causes
                        .entry(format!(\"regex:{}\", rule.draft.regex))
                        .or_default()
                        .push(rule.id.clone());
""",
    """                    // Identical RegExp causes are also valid selection alternatives.
""",
)
replace(
    "espanso-editor/src/workspace.rs",
    """
        for (cause, ids) in causes {
            if ids.len() > 1 {
                for id in ids {
                    diagnostics.push(Diagnostic {
                        level: DiagnosticLevel::Warning,
                        message: format!(\"Duplicate match cause: {}\", cause.replace(':', \": \")),
                        file: Some(id.file.clone()),
                        rule: Some(id),
                    });
                }
            }
        }
""",
    "\n",
)
replace(
    "espanso-editor/src/workspace.rs",
    """    fn simple_trigger_duplicates_are_not_reported_but_regex_duplicates_are() {
        let (_temp, mut workspace, _base, extra) = fixture();
        workspace
            .create_rule(
                &extra,
                &RuleDraft {
                    triggers: vec![\":hello\".to_owned()],
                    replace: \"Selection option\".to_owned(),
                    ..RuleDraft::default()
                },
            )
            .expect(\"create simple duplicate\");
        assert!(!workspace
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains(\"trigger:\")));

        workspace
            .create_rule(
                &extra,
                &RuleDraft {
                    kind: MatchKind::Regex,
                    regex: r\":id_(?P<id>\\d+)\".to_owned(),
                    replace: \"Duplicate regexp\".to_owned(),
                    ..RuleDraft::default()
                },
            )
            .expect(\"create regexp duplicate\");
        assert!(workspace
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains(\"Duplicate match cause: regex:\")));
    }
""",
    """    fn trigger_and_regexp_duplicates_are_valid_selection_alternatives() {
        let (_temp, mut workspace, _base, extra) = fixture();
        workspace
            .create_rule(
                &extra,
                &RuleDraft {
                    triggers: vec![\":hello\".to_owned()],
                    replace: \"Selection option\".to_owned(),
                    ..RuleDraft::default()
                },
            )
            .expect(\"create simple duplicate\");
        workspace
            .create_rule(
                &extra,
                &RuleDraft {
                    kind: MatchKind::Regex,
                    regex: r\":id_(?P<id>\\d+)\".to_owned(),
                    replace: \"Duplicate regexp\".to_owned(),
                    ..RuleDraft::default()
                },
            )
            .expect(\"create regexp duplicate\");

        assert!(!workspace
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains(\"Duplicate match cause\")));
    }
""",
)
replace(
    "espanso-editor/src/diagnostics.rs",
    "        (\"duplicate match cause\", \"duplicate-match\"),\n",
    "",
)
replace(
    "espanso-editor/src/diagnostics.rs",
    "            if code == \"duplicate-match\"\n                || code == \"duplicate-global-variable\"",
    "            if code == \"duplicate-global-variable\"",
)

# Runtime worker supervisor: never leave a live daemon without a worker.
write(
    "espanso/src/cli/daemon/mod.rs",
    r'''/*
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
    path::Path,
    process::Command,
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, Context, Result};
use crossbeam::{
    channel::{unbounded, Sender},
    select,
};
use espanso_ipc::IPCClient;
use log::{error, info, warn};

use crate::{
    cli::util::{prevent_running_as_root_on_macos, CommandExt},
    common_flags::*,
    exit_code::{
        DAEMON_ALREADY_RUNNING, DAEMON_FATAL_CONFIG_ERROR, DAEMON_GENERAL_ERROR, DAEMON_SUCCESS,
        WORKER_ERROR_EXIT_NO_CODE, WORKER_EXIT_ALL_PROCESSES, WORKER_RESTART,
    },
    ipc::{create_ipc_client_to_worker, IPCEvent},
    lock::{acquire_daemon_lock, acquire_worker_lock},
    path::Paths,
    VERSION,
};

use super::{CliModule, CliModuleArgs, PathsOverrides};

mod ipc;
mod keyboard_layout_watcher;
mod troubleshoot;
mod watcher;

const WORKER_STOP_TIMEOUT: Duration = Duration::from_secs(30);
const FORCED_STOP_TIMEOUT: Duration = Duration::from_secs(5);
const WORKER_STABLE_UPTIME: Duration = Duration::from_secs(60);
const MAX_RESTART_BACKOFF: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy)]
struct WorkerExitEvent {
    pid: u32,
    code: i32,
}

pub fn new() -> CliModule {
    #[allow(clippy::needless_update)]
    CliModule {
        requires_paths: true,
        enable_logs: true,
        log_mode: super::LogMode::CleanAndAppend,
        subcommand: "daemon".to_string(),
        entry: daemon_main,
        ..Default::default()
    }
}

fn daemon_main(args: CliModuleArgs) -> i32 {
    prevent_running_as_root_on_macos();

    let paths = args.paths.expect("missing paths in daemon main");
    let paths_overrides = args
        .paths_overrides
        .expect("missing paths_overrides in daemon main");

    let lock_file = acquire_daemon_lock(&paths.runtime);
    if lock_file.is_none() {
        error!("daemon is already running!");
        return DAEMON_ALREADY_RUNNING;
    }

    let mut _current_troubleshoot_guard = None;
    let (watcher_notify, watcher_signal) = unbounded::<()>();
    if let Err(error) = watcher::initialize_and_spawn(&paths.config, watcher_notify) {
        error!("unable to initialize config watcher thread: {error}");
        return DAEMON_GENERAL_ERROR;
    }

    let (keyboard_layout_watcher_notify, keyboard_layout_watcher_signal) = unbounded::<()>();
    #[allow(clippy::redundant_clone)]
    if let Err(error) = keyboard_layout_watcher::initialize_and_spawn(
        keyboard_layout_watcher_notify.clone(),
    ) {
        error!("unable to initialize keyboard layout watcher thread: {error}");
        return DAEMON_GENERAL_ERROR;
    }

    let mut config_store =
        match troubleshoot::load_config_or_troubleshoot_until_config_is_correct_or_abort(
            &paths,
            &paths_overrides,
            watcher_signal.clone(),
        ) {
            Ok((result, guard)) => {
                _current_troubleshoot_guard = guard;
                result.config_store
            }
            Err(err) => {
                error!("critical error while loading config: {err}");
                return DAEMON_FATAL_CONFIG_ERROR;
            }
        };

    info!("rEspanso version: {VERSION}");

    if !terminate_worker_if_already_running(&paths.runtime) {
        return DAEMON_GENERAL_ERROR;
    }

    let (exit_notify, exit_signal) = unbounded::<WorkerExitEvent>();
    let (daemon_notify, daemon_signal) = unbounded::<i32>();
    let mut current_worker_pid = match spawn_worker_with_retry(
        &paths_overrides,
        exit_notify.clone(),
        None,
    ) {
        Ok(pid) => pid,
        Err(error) => {
            error!("unable to start initial worker: {error}");
            return DAEMON_GENERAL_ERROR;
        }
    };
    let mut expected_worker_exit = None;
    let mut worker_started_at = Instant::now();
    let mut consecutive_failures = 0_u32;

    if let Err(error) = ipc::initialize_and_spawn(&paths.runtime, daemon_notify) {
        error!("unable to initialize ipc server for daemon: {error}");
        return DAEMON_GENERAL_ERROR;
    }

    loop {
        select! {
          recv(daemon_signal) -> code => {
            match code {
              Ok(DAEMON_SUCCESS) => {
                info!("daemon exit requested through IPC");
                break;
              }
              Ok(other) => warn!("unexpected daemon IPC exit code: {other}"),
              Err(error) => {
                error!("daemon IPC channel closed unexpectedly: {error}");
                return DAEMON_GENERAL_ERROR;
              }
            }
          }
          recv(watcher_signal) -> signal => {
            if signal.is_err() {
                error!("config watcher channel closed unexpectedly");
                return DAEMON_GENERAL_ERROR;
            }
            if !config_store.default().auto_restart() {
                continue;
            }

            info!("stable configuration change detected, restarting worker process...");
            let should_restart_worker = match troubleshoot::load_config_or_troubleshoot(&paths, &paths_overrides) {
              troubleshoot::LoadResult::Correct(result) => {
                config_store = result.config_store;
                _current_troubleshoot_guard = None;
                true
              },
              troubleshoot::LoadResult::Warning(result, guard) => {
                config_store = result.config_store;
                _current_troubleshoot_guard = guard;
                true
              }
              troubleshoot::LoadResult::Fatal(guard) => {
                _current_troubleshoot_guard = Some(guard);
                error!("critical error while loading config; keeping the last healthy worker");
                false
              }
            };

            if should_restart_worker {
                expected_worker_exit = Some(current_worker_pid);
                match restart_worker(
                    &paths,
                    &paths_overrides,
                    exit_notify.clone(),
                    current_worker_pid,
                    Some(WORKER_START_REASON_CONFIG_CHANGED.to_string()),
                ) {
                    Ok(pid) => {
                        current_worker_pid = pid;
                        worker_started_at = Instant::now();
                        consecutive_failures = 0;
                    }
                    Err(error) => {
                        error!("worker restart after config change failed: {error}");
                        expected_worker_exit = None;
                        match spawn_worker_with_retry(
                            &paths_overrides,
                            exit_notify.clone(),
                            Some(WORKER_START_REASON_CONFIG_CHANGED.to_string()),
                        ) {
                            Ok(pid) => {
                                current_worker_pid = pid;
                                worker_started_at = Instant::now();
                            }
                            Err(spawn_error) => {
                                error!("worker recovery failed: {spawn_error}");
                                return DAEMON_GENERAL_ERROR;
                            }
                        }
                    }
                }
            }
          }
          recv(keyboard_layout_watcher_signal) -> signal => {
            if signal.is_err() {
                error!("keyboard layout watcher channel closed unexpectedly");
                return DAEMON_GENERAL_ERROR;
            }
            info!("keyboard layout change detected, restarting worker...");
            expected_worker_exit = Some(current_worker_pid);
            match restart_worker(
                &paths,
                &paths_overrides,
                exit_notify.clone(),
                current_worker_pid,
                Some(WORKER_START_REASON_KEYBOARD_LAYOUT_CHANGED.to_string()),
            ) {
                Ok(pid) => {
                    current_worker_pid = pid;
                    worker_started_at = Instant::now();
                    consecutive_failures = 0;
                }
                Err(error) => {
                    error!("worker restart after keyboard layout change failed: {error}");
                    expected_worker_exit = None;
                }
            }
          }
          recv(exit_signal) -> event => {
            let event = match event {
                Ok(event) => event,
                Err(error) => {
                    error!("worker monitor channel closed: {error}");
                    return DAEMON_GENERAL_ERROR;
                }
            };

            if expected_worker_exit == Some(event.pid) {
                info!("expected worker generation {} exited with code {}", event.pid, event.code);
                expected_worker_exit = None;
                continue;
            }
            if event.pid != current_worker_pid {
                warn!("ignoring stale worker exit: pid={}, current={}", event.pid, current_worker_pid);
                continue;
            }

            match event.code {
                WORKER_EXIT_ALL_PROCESSES => {
                    info!("worker requested a general exit, quitting the daemon");
                    break;
                }
                WORKER_RESTART => {
                    info!("worker requested a restart, spawning a new generation...");
                    match spawn_worker_with_retry(
                        &paths_overrides,
                        exit_notify.clone(),
                        Some(WORKER_START_REASON_MANUAL.to_string()),
                    ) {
                        Ok(pid) => {
                            current_worker_pid = pid;
                            worker_started_at = Instant::now();
                            consecutive_failures = 0;
                        }
                        Err(error) => {
                            error!("manual worker restart failed: {error}");
                            return DAEMON_GENERAL_ERROR;
                        }
                    }
                }
                code => {
                    let uptime = worker_started_at.elapsed();
                    if uptime >= WORKER_STABLE_UPTIME {
                        consecutive_failures = 0;
                    }
                    consecutive_failures = consecutive_failures.saturating_add(1);
                    let delay = restart_backoff(consecutive_failures);
                    error!(
                        "worker pid {} exited unexpectedly with code {} after {:?}; restarting in {:?}",
                        event.pid,
                        code,
                        uptime,
                        delay
                    );
                    thread::sleep(delay);
                    match spawn_worker_with_retry(
                        &paths_overrides,
                        exit_notify.clone(),
                        Some("crash_recovery".to_string()),
                    ) {
                        Ok(pid) => {
                            current_worker_pid = pid;
                            worker_started_at = Instant::now();
                        }
                        Err(error) => {
                            error!("automatic worker recovery failed: {error}");
                            return DAEMON_GENERAL_ERROR;
                        }
                    }
                }
            }
          },
        }
    }

    DAEMON_SUCCESS
}

fn terminate_worker_if_already_running(runtime_dir: &Path) -> bool {
    let lock_file = acquire_worker_lock(runtime_dir);
    if lock_file.is_some() {
        return true;
    }

    warn!("a worker process is already running, sending termination signal...");
    match create_ipc_client_to_worker(runtime_dir) {
        Ok(mut worker_ipc) => {
            if let Err(err) = worker_ipc.send_async(IPCEvent::Exit) {
                error!("unable to send termination signal to worker process: {err}");
            }
        }
        Err(err) => error!("could not establish IPC connection with worker: {err}"),
    }

    if wait_for_worker_release(runtime_dir, Duration::from_secs(3)) {
        true
    } else {
        error!("could not terminate an unknown existing worker process");
        false
    }
}

fn spawn_worker(
    paths_overrides: &PathsOverrides,
    exit_notify: Sender<WorkerExitEvent>,
    start_reason: Option<String>,
) -> Result<u32> {
    info!("spawning the worker process...");
    let espanso_exe_path = std::env::current_exe().context("unable to obtain executable path")?;
    let mut command = Command::new(espanso_exe_path);
    let mut args = vec!["worker", "--monitor-daemon"];
    if let Some(start_reason) = &start_reason {
        args.push("--start-reason");
        args.push(start_reason);
    }
    command.args(&args);
    command.with_paths_overrides(paths_overrides);

    let mut child = command.spawn().context("unable to spawn worker process")?;
    let pid = child.id();
    thread::Builder::new()
        .name(format!("worker-status-monitor-{pid}"))
        .spawn(move || {
            let code = match child.wait() {
                Ok(status) => status.code().unwrap_or(WORKER_ERROR_EXIT_NO_CODE),
                Err(error) => {
                    error!("unable to wait for worker pid {pid}: {error}");
                    WORKER_ERROR_EXIT_NO_CODE
                }
            };
            if let Err(error) = exit_notify.send(WorkerExitEvent { pid, code }) {
                error!("unable to forward worker exit for pid {pid}: {error}");
            }
        })
        .context("unable to spawn worker monitor thread")?;
    Ok(pid)
}

fn spawn_worker_with_retry(
    paths_overrides: &PathsOverrides,
    exit_notify: Sender<WorkerExitEvent>,
    start_reason: Option<String>,
) -> Result<u32> {
    let mut last_error = None;
    for attempt in 0_u32..5 {
        match spawn_worker(paths_overrides, exit_notify.clone(), start_reason.clone()) {
            Ok(pid) => return Ok(pid),
            Err(error) => {
                let delay = Duration::from_millis(250 * u64::from(attempt + 1));
                error!("worker start attempt {} failed: {error}", attempt + 1);
                last_error = Some(error);
                thread::sleep(delay);
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("worker could not be started")))
}

fn restart_worker(
    paths: &Paths,
    paths_overrides: &PathsOverrides,
    exit_notify: Sender<WorkerExitEvent>,
    current_worker_pid: u32,
    start_reason: Option<String>,
) -> Result<u32> {
    match create_ipc_client_to_worker(&paths.runtime) {
        Ok(mut worker_ipc) => {
            if let Err(err) = worker_ipc.send_async(IPCEvent::Exit) {
                warn!("unable to send graceful worker termination: {err}");
            }
        }
        Err(err) => warn!("could not establish IPC connection with worker: {err}"),
    }

    if !wait_for_worker_release(&paths.runtime, WORKER_STOP_TIMEOUT) {
        error!(
            "worker pid {} did not stop within {:?}; forcing termination",
            current_worker_pid,
            WORKER_STOP_TIMEOUT
        );
        force_terminate_process(current_worker_pid)?;
        if !wait_for_worker_release(&paths.runtime, FORCED_STOP_TIMEOUT) {
            bail!(
                "worker pid {} kept its lock after forced termination",
                current_worker_pid
            );
        }
    }

    spawn_worker_with_retry(paths_overrides, exit_notify, start_reason)
}

fn wait_for_worker_release(runtime_dir: &Path, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if acquire_worker_lock(runtime_dir).is_some() {
            return true;
        }
        thread::sleep(Duration::from_millis(100));
    }
    false
}

fn restart_backoff(failures: u32) -> Duration {
    let shift = failures.saturating_sub(1).min(4);
    let millis = 500_u64.saturating_mul(1_u64 << shift);
    Duration::from_millis(millis).min(MAX_RESTART_BACKOFF)
}

#[cfg(target_os = "windows")]
fn force_terminate_process(pid: u32) -> Result<()> {
    let status = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status()
        .context("unable to execute taskkill")?;
    if status.success() {
        Ok(())
    } else {
        bail!("taskkill returned {status}")
    }
}

#[cfg(not(target_os = "windows"))]
fn force_terminate_process(pid: u32) -> Result<()> {
    let status = Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .status()
        .context("unable to execute kill")?;
    if status.success() {
        Ok(())
    } else {
        bail!("kill returned {status}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_backoff_is_bounded() {
        assert_eq!(restart_backoff(1), Duration::from_millis(500));
        assert_eq!(restart_backoff(2), Duration::from_secs(1));
        assert_eq!(restart_backoff(20), MAX_RESTART_BACKOFF);
    }
}
''',
)

# Stable watcher snapshot and self-recovery after notify errors.
write(
    "espanso/src/cli/daemon/watcher.rs",
    r'''/*
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
    let mut watcher: RecommendedWatcher = Watcher::new(tx, WATCHER_NOTIFY_DELAY)
        .context("unable to create file watcher")?;
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
        fs::write(&file, "matches:\n  - trigger: \":a\"\n    replace: a\n")
            .expect("second write");
        assert_ne!(before, config_fingerprint(temp.path()));
    }
}
''',
)

# Locks must never panic during shutdown or Drop.
write(
    "espanso/src/lock.rs",
    r'''/*
 * This file is part of espanso.
 *
 * Copyright (C) 2019-2021 Federico Terzi
 *
 * espanso is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

use anyhow::Result;
use fs2::FileExt;
use log::{error, warn};
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

pub struct Lock {
    lock_file: File,
}

impl Lock {
    #[allow(dead_code)]
    pub fn release(self) -> Result<()> {
        fs2::FileExt::unlock(&self.lock_file)?;
        Ok(())
    }

    fn acquire(runtime_dir: &Path, name: &str) -> Option<Lock> {
        let lock_file_path = runtime_dir.join(format!("{name}.lock"));
        let lock_file = match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_file_path)
        {
            Ok(file) => file,
            Err(error) => {
                error!("unable to open lock file {}: {error}", lock_file_path.display());
                return None;
            }
        };
        if lock_file.try_lock_exclusive().is_ok() {
            Some(Lock { lock_file })
        } else {
            None
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        if let Err(error) = fs2::FileExt::unlock(&self.lock_file) {
            warn!("unable to unlock lock file during shutdown: {error}");
        }
    }
}

pub fn acquire_daemon_lock(runtime_dir: &Path) -> Option<Lock> {
    Lock::acquire(runtime_dir, "espanso-daemon")
}

pub fn acquire_worker_lock(runtime_dir: &Path) -> Option<Lock> {
    Lock::acquire(runtime_dir, "espanso-worker")
}
''',
)

# Explicit launcher failure code, portable-safe wizard behaviour and no panic on daemon exit.
replace(
    "espanso/src/exit_code.rs",
    "pub const LAUNCHER_ALREADY_RUNNING: i32 = 2;",
    "pub const LAUNCHER_ALREADY_RUNNING: i32 = 2;\npub const LAUNCHER_DAEMON_FAILURE: i32 = 3;",
)
replace(
    "espanso/src/cli/launcher/mod.rs",
    "        LAUNCHER_ALREADY_RUNNING, LAUNCHER_CONFIG_DIR_POPULATION_FAILURE, LAUNCHER_SUCCESS,",
    "        LAUNCHER_ALREADY_RUNNING, LAUNCHER_CONFIG_DIR_POPULATION_FAILURE, LAUNCHER_DAEMON_FAILURE,\n        LAUNCHER_SUCCESS,",
)
replace(
    "espanso/src/cli/launcher/mod.rs",
    """    let is_auto_start_page_enabled =
        !preferences.has_selected_auto_start_option() && !cfg!(target_os = \"linux\");
""",
    """    let is_auto_start_page_enabled = !paths.is_portable_mode
        && !preferences.has_selected_auto_start_option()
        && !cfg!(target_os = \"linux\");
""",
)
replace(
    "espanso/src/cli/launcher/mod.rs",
    """        daemon::launch_daemon(&paths_overrides).expect(\"failed to launch daemon\");
""",
    """        if let Err(error) = daemon::launch_daemon(&paths_overrides) {
            error!(\"rEspanso daemon stopped unexpectedly: {error}\");
            return LAUNCHER_DAEMON_FAILURE;
        }
""",
)

# Portable bootstrap log and visible fatal error.
write(
    "espanso/src/bin/respanso-portable.rs",
    r'''#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

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
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    let title = "rEspanso\0".encode_utf16().collect::<Vec<_>>();
    let message = format!("{message}\0").encode_utf16().collect::<Vec<_>>();
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
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
''',
)

# Full rEspanso branding in the first-run wizard.
replace(
    "espanso-modulo/src/sys/wizard/wizard_gui.h",
    'const wxString &title = wxT("Espanso")',
    'const wxString &title = wxT("rEspanso")',
)
replace(
    "espanso-modulo/src/sys/wizard/wizard_gui.cpp",
    'wxT("Welcome to Espanso!")',
    'wxT("Добро пожаловать в rEspanso")',
)
replace(
    "espanso-modulo/src/sys/wizard/wizard_gui.cpp",
    'wxT("This wizard will help you to quickly get started with espanso. "\n            "\\n\\nClick \\\"Start\\\" when you are ready")',
    'wxT("Быстрая подстановка текста, гибкие шаблоны и локальная "\n            "автоматизация. Настройка займёт меньше минуты.\\n\\nНажмите "\n            "\\\"Начать\\\", чтобы продолжить.")',
)
replace(
    "espanso-modulo/src/sys/wizard/wizard_gui.cpp",
    'wxT("Start")',
    'wxT("Начать")',
    1,
)
replace(
    "espanso-modulo/src/sys/wizard/wizard_gui.cpp",
    'wxT("Quit Espanso")',
    'wxT("Закрыть rEspanso")',
)
replace(
    "espanso-modulo/src/sys/wizard/wizard.cpp",
    'wxString::Format("( version %s )", wizard_metadata->version)',
    'wxString::Format("Версия %s", wizard_metadata->version)',
)
replace(
    "espanso-modulo/src/sys/wizard/wizard.cpp",
    '"An error occurred while registering Espanso as a service, "',
    '"Не удалось зарегистрировать rEspanso в автозапуске. "',
)

# Unified storm resources and cache-safe extraction names.
replace(
    "espanso/src/icon.rs",
    'const LOGO_NO_BACKGROUND_BINARY: &[u8] = include_bytes!("res/logo_no_background.png");',
    'const LOGO_NO_BACKGROUND_BINARY: &[u8] = include_bytes!("res/logo_no_background.png");\n#[cfg(target_os = "windows")]\nconst WINDOWS_NOTIFICATION_BINARY: &[u8] = include_bytes!("res/windows/notification.png");',
)
replace(
    "espanso/src/icon.rs",
    "    pub logo: Option<PathBuf>,\n    pub logo_no_background: Option<PathBuf>,",
    "    pub logo: Option<PathBuf>,\n    pub logo_no_background: Option<PathBuf>,\n    pub notification_icon: Option<PathBuf>,",
)
replace(
    "espanso/src/icon.rs",
    '&runtime_dir.join("formv2.ico")',
    '&runtime_dir.join("respanso-form-v4.ico")',
)
replace(
    "espanso/src/icon.rs",
    '&runtime_dir.join("wizardv2.ico")',
    '&runtime_dir.join("respanso-wizard-v4.ico")',
)
replace(
    "espanso/src/icon.rs",
    '&runtime_dir.join("normalv2.ico")',
    '&runtime_dir.join("respanso-tray-active-v4.ico")',
)
replace(
    "espanso/src/icon.rs",
    '&runtime_dir.join("disabledv2.ico")',
    '&runtime_dir.join("respanso-tray-disabled-v4.ico")',
)
replace(
    "espanso/src/icon.rs",
    'logo: Some(extract_icon(ICON_BINARY, &runtime_dir.join("iconv2.png"))?),',
    'logo: Some(extract_icon(ICON_BINARY, &runtime_dir.join("respanso-logo-v4.png"))?),',
    1,
)
replace(
    "espanso/src/icon.rs",
    '&runtime_dir.join("icon_no_backgroundv2.png")',
    '&runtime_dir.join("respanso-welcome-v4.png")',
)
replace(
    "espanso/src/icon.rs",
    """        tray_explain_image: Some(extract_icon(
            WINDOWS_TRAY_EXPLAIN_IMAGE,
            &runtime_dir.join(\"tray_explain_image.png\"),
        )?),
""",
    """        tray_explain_image: Some(extract_icon(
            WINDOWS_TRAY_EXPLAIN_IMAGE,
            &runtime_dir.join(\"tray_explain_image.png\"),
        )?),
        notification_icon: Some(extract_icon(
            WINDOWS_NOTIFICATION_BINARY,
            &runtime_dir.join(\"respanso-notification-v4.png\"),
        )?),
""",
)
replace(
    "espanso/src/cli/worker/mod.rs",
    """        notification_icon_path: icon_paths
            .logo
""",
    """        notification_icon_path: icon_paths
            .notification_icon
            .as_ref()
            .or(icon_paths.logo.as_ref())
""",
)

# Better Windows shortcut branding for non-portable service installs.
replace(
    "espanso/src/cli/service/win.rs",
    'Ok(parent.join("espanso.lnk"))',
    'Ok(parent.join("rEspanso.lnk"))',
)
replace(
    "espanso/src/cli/service/win.rs",
    """$Shortcut.TargetPath = $env:TARGET_PATH; $Shortcut.Arguments = $env:TARGET_ARGS; $Shortcut.Save()""",
    """$Shortcut.TargetPath = $env:TARGET_PATH; $Shortcut.Arguments = $env:TARGET_ARGS; $Shortcut.WorkingDirectory = Split-Path $env:TARGET_PATH; $Shortcut.IconLocation = $env:TARGET_PATH; $Shortcut.Save()""",
)

# Documentation reflects the new semantics and recovery contract.
match_docs = ROOT / "docs/respanso/MATCH_STUDIO.ru.md"
text = match_docs.read_text(encoding="utf-8")
text = text.replace(
    "Одинаковые обычные триггеры допустимы: rEspanso покажет окно выбора.",
    "Одинаковые обычные триггеры и RegExp допустимы: rEspanso покажет окно выбора.",
)
text = text.replace(
    "Повторы RegExp остаются предупреждением.",
    "Повторы RegExp не считаются ошибкой: они используются как варианты окна выбора.",
)
match_docs.write_text(text, encoding="utf-8", newline="\n")

write(
    "docs/respanso/RUNTIME_STABILITY_2026-08-01.ru.md",
    """# Стабилизация runtime rEspanso — 01.08.2026

- одинаковые RegExp разрешены как варианты окна выбора;
- daemon контролирует PID каждого поколения worker;
- неожиданный выход worker автоматически восстанавливается с ограниченным backoff;
- зависший worker после тайм-аута завершается принудительно, затем запускается новое поколение;
- worker с кодом 0 вне ожидаемого shutdown также считается аварийно завершившимся;
- reload выполняется только после двух одинаковых снимков YAML;
- watcher самостоятельно восстанавливается после ошибки notify;
- некорректная конфигурация не останавливает последний исправный worker;
- portable-режим больше не предлагает ошибочный автозапуск core без portable-путей;
- portable launcher пишет `runtime/rEspanso-bootstrap.log` и показывает видимую ошибку;
- мастер первого запуска и системные ресурсы приведены к бренду rEspanso;
- ресурсы трея и уведомлений используют новые имена версии v4, исключая старый runtime-кэш.
""",
)

print("runtime stability and branding patch applied")

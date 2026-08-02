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

#[allow(clippy::too_many_lines, clippy::collapsible_match)]
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
    if let Err(error) =
        keyboard_layout_watcher::initialize_and_spawn(keyboard_layout_watcher_notify.clone())
    {
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
    let mut current_worker_pid =
        match spawn_worker_with_retry(&paths_overrides, exit_notify.clone(), None) {
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
            current_worker_pid, WORKER_STOP_TIMEOUT
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

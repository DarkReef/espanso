/*
 * This file is part of espanso.
 *
 * Copyright (C) 2019-2021 Federico Terzi
 *
 * espanso is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * espanso is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with espanso.  If not, see <https://www.gnu.org/licenses/>.
 */

use std::time::Instant;

use super::{CliModule, CliModuleArgs, PathsOverrides};
use crate::{
    error_eprintln,
    exit_code::{
        SERVICE_ALREADY_RUNNING, SERVICE_FAILURE, SERVICE_NOT_REGISTERED, SERVICE_NOT_RUNNING,
        SERVICE_SUCCESS, SERVICE_TIMED_OUT,
    },
    info_println,
    lock::acquire_worker_lock,
};

#[cfg(target_os = "macos")]
mod macos;
use crate::path::Paths;
use clap::ArgMatches;
#[cfg(target_os = "macos")]
use macos::*;

#[cfg(not(target_os = "windows"))]
mod unix;
#[cfg(not(target_os = "windows"))]
use unix::*;

#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
use win::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::*;

mod stop;

pub fn new() -> CliModule {
    CliModule {
        enable_logs: true,
        disable_logs_terminal_output: true,
        requires_paths: true,
        subcommand: "service".to_string(),
        log_mode: super::LogMode::AppendOnly,
        entry: service_main,
        ..Default::default()
    }
}

fn service_main(args: CliModuleArgs) -> i32 {
    let paths = args.paths.expect("missing paths argument");
    let cli_args = args.cli_args.expect("missing cli_args");
    #[allow(unused_variables)]
    let paths_overrides = args.paths_overrides.expect("missing paths_overrides");

    if cli_args.subcommand_matches("register").is_some() {
        if let Err(err) = register() {
            error_eprintln!("unable to register service: {}", err);
            return SERVICE_FAILURE;
        }
        info_println!("service registered correctly!");
    } else if cli_args.subcommand_matches("unregister").is_some() {
        if let Err(err) = unregister() {
            error_eprintln!("unable to unregister service: {}", err);
            return SERVICE_FAILURE;
        }
        info_println!("service unregistered correctly!");
    } else if cli_args.subcommand_matches("check").is_some() {
        if is_registered() {
            info_println!("registered as a service");
        } else {
            error_eprintln!("not registered as a service");
            return SERVICE_NOT_REGISTERED;
        }
    } else if let Some(sub_args) = cli_args.subcommand_matches("start") {
        return start_main(&paths, &paths_overrides, sub_args);
    } else if cli_args.subcommand_matches("stop").is_some() {
        return stop_main(&paths);
    } else if cli_args.subcommand_matches("status").is_some() {
        return status_main(&paths);
    } else if let Some(sub_args) = cli_args.subcommand_matches("restart") {
        return restart_main(&paths, &paths_overrides, sub_args);
    } else {
        eprintln!("Invalid usage, please run `rEspanso service --help` for more information.");
    }

    SERVICE_SUCCESS
}

fn restart_main(paths: &Paths, paths_overrides: &PathsOverrides, args: &ArgMatches) -> i32 {
    // stop_main already waits until the worker/daemon lock is released. A
    // fixed delay here used to race slow X11/GTK shutdowns and, worse, a stop
    // error was ignored before a second instance was started.
    let stop_result = stop_main(paths);
    if stop_result != SERVICE_SUCCESS && stop_result != SERVICE_NOT_RUNNING {
        error_eprintln!(
            "unable to restart rEspanso because the running instance did not stop cleanly"
        );
        return stop_result;
    }

    start_main(paths, paths_overrides, args)
}

fn start_main(paths: &Paths, _paths_overrides: &PathsOverrides, args: &ArgMatches) -> i32 {
    let lock_file = acquire_worker_lock(&paths.runtime);
    if lock_file.is_none() {
        error_eprintln!("espanso is already running!");
        return SERVICE_ALREADY_RUNNING;
    }
    drop(lock_file);

    if args.is_present("unmanaged") && !cfg!(target_os = "windows") {
        // Unmanaged service
        #[cfg(unix)]
        {
            if let Err(err) = fork_daemon(_paths_overrides, &paths.runtime) {
                error_eprintln!("unable to start service (unmanaged): {}", err);
                return SERVICE_FAILURE;
            }
        }
        #[cfg(windows)]
        {
            unreachable!();
        }
    } else {
        // Managed service
        if let Err(err) = start_service() {
            error_eprintln!("unable to start service: {}", err);
            return SERVICE_FAILURE;
        }
    }

    let now = Instant::now();
    while now.elapsed() < std::time::Duration::from_secs(5) {
        let lock_file = acquire_worker_lock(&paths.runtime);
        if lock_file.is_none() {
            info_println!("espanso started correctly!");
            return SERVICE_SUCCESS;
        }
        drop(lock_file);

        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    error_eprintln!("unable to start service: timed out");

    error_eprintln!("See {} and {} for the startup failure.",
        paths.runtime.join("startup.log").display(),
        paths.runtime.join("espanso.log").display());

    SERVICE_TIMED_OUT
}

fn stop_main(paths: &Paths) -> i32 {
    // In the healthy case the worker owns the lifecycle: ExitAllProcesses lets
    // it shut down the X11/UI engine first and then asks the daemon to exit.
    // Killing the daemon first makes the worker treat it as an unexpected
    // crash and can leave the X11 event loop alive until the service timeout.
    let worker_lock = acquire_worker_lock(&paths.runtime);
    if worker_lock.is_none() {
        if let Err(err) = stop::terminate_worker(&paths.runtime) {
            error_eprintln!("unable to stop rEspanso: {}", err);
            return SERVICE_FAILURE;
        }
        return SERVICE_SUCCESS;
    }
    drop(worker_lock);

    // Recovery path: if the worker has already crashed/released its lock but
    // the owning daemon is still alive, terminate the daemon directly.
    if crate::lock::acquire_daemon_lock(&paths.runtime).is_none() {
        return match stop::terminate_daemon(&paths.runtime) {
            Ok(()) => SERVICE_SUCCESS,
            Err(err) => {
                error_eprintln!("unable to stop rEspanso daemon: {}", err);
                SERVICE_FAILURE
            }
        };
    }

    error_eprintln!("espanso is not running!");
    SERVICE_NOT_RUNNING
}

fn status_main(paths: &Paths) -> i32 {
    let lock_file = acquire_worker_lock(&paths.runtime);
    if lock_file.is_some() {
        error_eprintln!("espanso is not running");
        return SERVICE_NOT_RUNNING;
    }
    drop(lock_file);

    info_println!("espanso is running");
    SERVICE_SUCCESS
}

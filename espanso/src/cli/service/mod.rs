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

use crate::{
    error_eprintln,
    exit_code::{
        SERVICE_ALREADY_RUNNING, SERVICE_FAILURE, SERVICE_NOT_RUNNING, SERVICE_SUCCESS,
        SERVICE_TIMED_OUT,
    },
    info_println,
    lock::acquire_worker_lock,
};

#[cfg(target_os = "macos")]
pub mod macos;
use crate::path::Paths;
#[cfg(target_os = "macos")]
use macos::*;

#[cfg(not(target_os = "windows"))]
pub mod unix;
#[cfg(not(target_os = "windows"))]
use unix::*;

#[cfg(target_os = "windows")]
pub mod win;
#[cfg(target_os = "windows")]
use win::*;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
use linux::*;

mod stop;

// pub fn new() -> CliModule {
//     CliModule {
//         enable_logs: true,
//         disable_logs_terminal_output: true,
//         requires_paths: true,
//         subcommand: "service".to_string(),
//         log_mode: super::LogMode::AppendOnly,
//         entry: service_main,
//         ..Default::default()
//     }
// }

pub fn start_main(paths: &Paths, is_unmanaged: bool) -> i32 {
    let lock_file = acquire_worker_lock(&paths.runtime);
    if lock_file.is_none() {
        error_eprintln!("espanso is already running!");
        return SERVICE_ALREADY_RUNNING;
    }
    drop(lock_file);

    if is_unmanaged && !cfg!(target_os = "windows") {
        // Unmanaged service
        #[cfg(unix)]
        {
            if let Err(err) = fork_daemon((
                Some(paths.config.clone()),
                Some(paths.packages.clone()),
                Some(paths.runtime.clone()),
            )) {
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

    error_eprintln!(
    "Hint: sometimes this happens because another Espanso process is left running for some reason."
  );
    error_eprintln!(
    "      Please try running 'espanso restart' or manually killing all Espanso processes, then try again."
  );

    SERVICE_TIMED_OUT
}

pub fn stop_main(paths: &Paths) -> i32 {
    let lock_file = acquire_worker_lock(&paths.runtime);
    if lock_file.is_some() {
        error_eprintln!("espanso is not running!");
        return SERVICE_NOT_RUNNING;
    }
    drop(lock_file);

    if let Err(err) = stop::terminate_worker(&paths.runtime) {
        error_eprintln!("unable to stop espanso: {}", err);
        return SERVICE_FAILURE;
    }

    SERVICE_SUCCESS
}

pub fn status_main(paths: &Paths) -> i32 {
    let lock_file = acquire_worker_lock(&paths.runtime);
    if lock_file.is_some() {
        error_eprintln!("espanso is not running");
        return SERVICE_NOT_RUNNING;
    }
    drop(lock_file);

    info_println!("espanso is running");
    SERVICE_SUCCESS
}

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

use crossbeam::channel::unbounded;
use espanso_config::{config::ConfigStore, matches::store::MatchStore};
use espanso_engine::event::ExitMode;
use log::{debug, error, info};

use crate::{
    cli::util::prevent_running_as_root_on_macos,
    exit_code::{
        WORKER_ALREADY_RUNNING, WORKER_EXIT_ALL_PROCESSES, WORKER_GENERAL_ERROR, WORKER_RESTART,
        WORKER_SUCCESS,
    },
    lock::acquire_worker_lock,
    path::Paths,
};

use self::ui::util::convert_icon_paths_to_tray_vec;

mod builtin;
mod config;
mod context;
mod daemon_monitor;
mod engine;
mod ipc;
mod match_cache;
mod secure_input;
mod ui;

pub fn worker_main(
    paths: Paths,
    config_store: Box<dyn ConfigStore>,
    match_store: Box<dyn MatchStore>,
    monitor_daemon: bool,
    start_reason: Option<String>,
) -> i32 {
    prevent_running_as_root_on_macos();

    debug!("starting with start-reason = {start_reason:?}");

    // Avoid running multiple worker instances
    let lock_file = acquire_worker_lock(&paths.runtime);
    if lock_file.is_none() {
        error!("worker is already running!");
        return WORKER_ALREADY_RUNNING;
    }

    let use_evdev_backend = if cfg!(feature = "wayland") {
        true
    } else {
        std::env::var("USE_EVDEV").unwrap_or_else(|_| "false".to_string()) == "true"
    };

    let icon_paths =
        crate::icon::load_icon_paths(&paths.runtime).expect("unable to initialize icons");

    let (remote, mut eventloop) = espanso_ui::create_ui(espanso_ui::UIOptions {
        show_icon: config_store.default().show_icon(),
        icon_paths: convert_icon_paths_to_tray_vec(&icon_paths),
        notification_icon_path: icon_paths
            .logo
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
    })
    .expect("unable to create tray icon UI module");

    eventloop
        .initialize()
        .expect("unable to initialize UI module");

    let (engine_exit_notify, engine_exit_receiver) = unbounded();
    let (ipc_event_notify, ipc_event_receiver) = unbounded();
    let (engine_ui_event_sender, engine_ui_event_receiver) = unbounded();
    let (engine_secure_input_sender, engine_secure_input_receiver) = unbounded();

    // Initialize the engine on another thread and start it
    let engine_handle = engine::initialize_and_spawn(
        paths.clone(),
        config_store,
        match_store,
        remote,
        engine_exit_receiver,
        engine_ui_event_receiver,
        engine_secure_input_receiver,
        use_evdev_backend,
        start_reason,
        ipc_event_receiver,
    )
    .expect("unable to initialize engine");

    // Setup the IPC server
    ipc::initialize_and_spawn(&paths.runtime, engine_exit_notify.clone(), ipc_event_notify)
        .expect("unable to initialize IPC server");

    // If specified, automatically monitor the daemon status and
    // terminate the worker if the daemon terminates
    // This is needed to avoid "dangling" worker processes
    // if the daemon crashes or is forcefully terminated.
    if monitor_daemon {
        daemon_monitor::initialize_and_spawn(&paths.runtime, engine_exit_notify)
            .expect("unable to initialize daemon monitor thread");
    }

    secure_input::initialize_and_spawn(engine_secure_input_sender)
        .expect("unable to initialize secure input watcher");

    eventloop
        .run(Box::new(move |event| {
            if let Err(error) = engine_ui_event_sender.send(event) {
                error!("unable to send UIEvent to engine: {error}");
                panic!("broken UI->Engine channel");
            }
        }))
        .expect("unable to run main eventloop");

    info!("waiting for engine exit mode...");
    match engine_handle.join() {
        Ok(mode) => match mode {
            ExitMode::Exit => {
                info!("exiting worker process...");
                WORKER_SUCCESS
            }
            ExitMode::ExitAllProcesses => {
                info!("exiting worker process and daemon...");
                WORKER_EXIT_ALL_PROCESSES
            }
            ExitMode::RestartWorker => {
                info!("exiting worker (to be restarted)");
                WORKER_RESTART
            }
        },
        Err(err) => {
            error!("unable to read engine exit mode: {err:?}");
            WORKER_GENERAL_ERROR
        }
    }
}

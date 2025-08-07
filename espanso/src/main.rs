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

// This is needed to avoid showing a console window when starting espanso on Windows
#![windows_subsystem = "windows"]

use std::{collections::HashMap, process::Command};

mod cli;
mod config;
mod gui;
mod icon;
mod ipc;
mod lock;
#[macro_use]
mod logging;
mod capabilities;
mod common_flags;
mod exit_code;
mod patch;
mod path;
mod preferences;
mod util;

use crate::{
    cli::{cmd, log::log_main, service, workaround, worker},
    path::get_path_override,
};
use clap::Parser;
use cli::LogMode;
use config::load_config;
use log::{info, LevelFilter};
use logging::FileProxy;
use path::{resolve_paths, Paths};
use simplelog::{
    CombinedLogger, ConfigBuilder, SharedLogger, TermLogger, TerminalMode, WriteLogger,
};

const LOG_FILE_NAME: &str = "espanso.log";

fn main() {
    match util::attach_console() {
        Ok(()) => info!("Console attached"),
        Err(e) => panic!("Could not attach console! {e}"),
    }

    let args = cli::Arguments::parse();

    let log_level = match args.verbose {
        0 | 1 => {
            // println!("Debug mode is off");
            LevelFilter::Info
        }
        2 => {
            println!("Debug mode is on");
            LevelFilter::Debug
        }
        3 => {
            // Trace mode is only available in debug mode for security reasons
            #[cfg(debug_assertions)]
            println!("Trace mode is on");
            LevelFilter::Trace
        }
        _ => LevelFilter::Warn,
    };

    let log_proxy = FileProxy::new();

    let config = ConfigBuilder::new()
        .set_time_to_local(true)
        .set_time_format(format!(
            "%H:%M:%S [{:?}({})]",
            args.command,
            std::process::id()
        ))
        .set_location_level(LevelFilter::Off)
        .add_filter_ignore_str("html5ever")
        .build();

    let mut outputs: Vec<Box<dyn SharedLogger>> = vec![WriteLogger::new(
        LevelFilter::Info,
        config.clone(),
        log_proxy.clone(),
    )];

    outputs.insert(0, TermLogger::new(log_level, config, TerminalMode::Mixed));

    CombinedLogger::init(outputs).expect("unable to initialize logs");

    // Activate logging for panics
    log_panics::init();

    // declare the data here
    let mut args_hashmap: HashMap<&str, &str> = HashMap::new();
    // input typed by the user (arg)
    let mut user_input = String::new();

    let (force_config_path, force_package_path, force_runtime_path) = get_path_override(&args);

    let paths: Paths = resolve_paths(
        force_config_path.as_deref(),
        force_package_path.as_deref(),
        force_runtime_path.as_deref(),
    );

    let config_result = load_config(&paths.config).expect("unable to load config");

    let config_store = config_result.config_store;
    let match_store = config_result.match_store;
    // let non_fatal_errors = config_result.non_fatal_errors;

    // just a box to store the exite code
    let mut exit_code = 0;

    match args.command {
        cli::Command::Cmd(subcmd) => cmd::cmd_main(subcmd, paths),
        cli::Command::Edit { target_file } => {
            args_hashmap.insert("command", "edit");
            if let Some(_file) = target_file {
                // args_hashmap.insert("edit-file", file.clone().as_str());
            } else {
                args_hashmap.insert("edit-file", "empty");
                println!("`espanso edit` (empty) was passed");
            }
            1 // TODO
        }
        cli::Command::EnvPath(..) => {
            println!("some dummy output");
            1 // TODO
        }
        cli::Command::Install { .. } => {
            println!("some dummy output");
            1 // TODO
        }
        cli::Command::Launch => {
            println!("some dummy output");

            #[cfg(target_os = "macos")]
            espanso_mac_utils::convert_to_foreground_app();

            1 // TODO
        }
        cli::Command::Log => {
            enable_logs(log_proxy, &paths, LogMode::Read);
            log_main(Some(paths))
        }
        cli::Command::Match(cmd) => {
            args_hashmap.insert("command", "match");
            println!("some dummy output");

            match cmd {
                cli::MatchArgs::Exec { trigger } => {
                    args_hashmap.insert("subcommand", "exec");
                    trigger.clone_into(&mut user_input);
                    args_hashmap.insert("trigger", user_input.as_str());
                }
                cli::MatchArgs::List(flags) => {
                    args_hashmap.insert("subcommand", "list");
                    if let Some(class) = flags.class {
                        class.clone_into(&mut user_input);
                        args_hashmap.insert("class", user_input.as_str());
                    }
                    if let Some(exec) = flags.exec {
                        dbg!(&exec);
                        dbg!(&user_input);
                    }
                    if let Some(title) = flags.title {
                        title.clone_into(&mut user_input.clone());
                        args_hashmap.insert("title", user_input.as_str());
                    }
                    if flags.json {
                        args_hashmap.insert("json", "true");
                    }
                    if flags.only_triggers {
                        args_hashmap.insert("onlytriggers", "true");
                    }
                    if flags.preserve_newlines {
                        args_hashmap.insert("preservenewlines", "true");
                    }
                }
            }

            1 // TODO
        }
        cli::Command::Package { .. } => {
            println!("some dummy output");
            1 // TODO
        }
        cli::Command::Path(path_args) => match path_args {
            cli::PathArgs::Base => todo!(),
            cli::PathArgs::Config => todo!(),
            cli::PathArgs::Default => todo!(),
            cli::PathArgs::Packages => todo!(),
            cli::PathArgs::Runtime => todo!(),
        },
        cli::Command::Restart(start_args) => {
            service::stop_main(&paths);
            std::thread::sleep(std::time::Duration::from_millis(300));
            service::start_main(&paths, start_args.unmanaged)
        }
        cli::Command::Service(service_command) => {
            exit_code = match service_command {
                cli::ServiceCommand::Check => {
                    #[cfg(target_os = "windows")]
                    if service::win::is_registered() {
                        info_println!("registered as a service");
                    } else {
                        error_eprintln!("not registered as a service");
                    }

                    #[cfg(target_os = "linux")]
                    if service::linux::is_registered() {
                        info_println!("registered as a service");
                    } else {
                        error_eprintln!("not registered as a service");
                    }
                    #[cfg(target_os = "macos")]
                    if service::macos::is_registered() {
                        info_println!("registered as a service");
                    } else {
                        error_eprintln!("not registered as a service");
                    }
                    0
                }
                cli::ServiceCommand::Register => {
                    #[cfg(target_os = "windows")]
                    if let Err(err) = service::win::register() {
                        error_eprintln!("unable to register service: {}", err);
                        std::process::exit(1);
                    }

                    #[cfg(target_os = "linux")]
                    if let Err(err) = service::linux::register() {
                        error_eprintln!("unable to register service: {}", err);
                        std::process::exit(1);
                    }

                    #[cfg(target_os = "macos")]
                    if let Err(err) = service::macos::register() {
                        error_eprintln!("unable to register service: {}", err);
                        std::process::exit(1);
                    }

                    info_println!("service registered correctly!");
                    0
                }
                cli::ServiceCommand::Restart(start_args) => {
                    service::stop_main(&paths);
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    service::start_main(&paths, start_args.unmanaged)
                }
                cli::ServiceCommand::Start(start_args) => {
                    service::start_main(&paths, start_args.unmanaged)
                }
                cli::ServiceCommand::Status => service::status_main(&paths),
                cli::ServiceCommand::Stop => service::stop_main(&paths),
                cli::ServiceCommand::Unregister => {
                    #[cfg(target_os = "windows")]
                    if let Err(err) = service::win::unregister() {
                        error_eprintln!("unable to unregister service: {}", err);
                        std::process::exit(1);
                    }

                    #[cfg(target_os = "linux")]
                    if let Err(err) = service::linux::unregister() {
                        error_eprintln!("unable to unregister service: {}", err);
                        std::process::exit(1);
                    }

                    #[cfg(target_os = "macos")]
                    if let Err(err) = service::macos::unregister() {
                        error_eprintln!("unable to unregister service: {}", err);
                        std::process::exit(1);
                    }

                    info_println!("service unregistered correctly!");
                    0
                }
            };
            exit_code
        }
        cli::Command::Start(start_args) => service::start_main(&paths, start_args.unmanaged),
        cli::Command::Status => service::status_main(&paths),
        cli::Command::Stop => service::stop_main(&paths),
        cli::Command::Uninstall(uninstall_args) => {
            println!("installing {}", uninstall_args.package_name);
            1 // TODO
        }
        cli::Command::Workaround(_unused_workaround_args) => {
            workaround::workaround_main(_unused_workaround_args)
        }
        cli::Command::Worker {
            monitor_daemon,
            start_reason,
        } => worker::worker_main(
            paths,
            config_store,
            match_store,
            monitor_daemon,
            start_reason,
        ),
    };

    // to compare the list of handlers to the `cli_args`
    // try to invoke `kdotool` to see if you have it or not.
    if Command::new("kdotool")
        .arg("getactivewindow")
        .arg("getwindowclassname")
        .output()
        .is_ok()
    {
    } else {
        info!("kdotool missing or not available for the current wayland DE.");
    }

    std::process::exit(exit_code);
}

fn enable_logs(log_proxy: FileProxy, paths: &Paths, log_mode: LogMode) {
    log_proxy
        .set_output_file(
            &paths.runtime.join(LOG_FILE_NAME),
            log_mode == LogMode::Read,
            log_mode == LogMode::CleanAndAppend,
        )
        .expect("unable to set up log output file");
}

#[cfg(test)]
mod tests {}

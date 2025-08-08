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
    cli::{
        cmd, daemon, edit::edit_main, env_path, launcher, log::log_main, modulo, package, service,
        workaround, worker,
    },
    path::{
        get_default_config_path, get_default_runtime_dir, get_path_override,
        get_portable_config_dir, get_portable_runtime_dir, is_portable_mode,
    },
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

const VERSION: &str = env!("CARGO_PKG_VERSION");
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
        cli::Command::Daemon => daemon::daemon_main(paths),
        cli::Command::Edit { target_file } => edit_main(target_file, paths),
        cli::Command::EnvPath(env_path_args) => env_path::env_path_main(env_path_args),
        cli::Command::Install(install_args) => {
            package::package_main(cli::PackageArgs::Install(install_args), paths)
        }
        cli::Command::Launcher => {
            #[cfg(target_os = "macos")]
            espanso_mac_utils::convert_to_foreground_app();
            launcher::launcher_main(paths)
        }
        cli::Command::Log => {
            enable_logs(log_proxy, &paths, LogMode::Read);
            log_main(Some(paths))
        }
        cli::Command::Match(cmd) => {
            todo!();
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
        cli::Command::Modulo(modulo_args) => {
            todo!("Need to pass stdin to `FormArgs`");
            modulo::modulo_main(modulo_args, paths)
        }
        cli::Command::Package(package_args) => package::package_main(package_args, paths),
        cli::Command::Path(path_args) => {
            if let Some(default_config_path) = if is_portable_mode() {
                get_portable_config_dir()
            } else {
                Some(get_default_config_path())
            } {
                match path_args {
                    cli::PathArgs::Base => {
                        println!(
                            "{}",
                            &default_config_path.join("match").join("base.yml").display()
                        );
                    }
                    cli::PathArgs::Config => {
                        println!("{}", &paths.config.display());
                    }
                    cli::PathArgs::Default => println!("{}", &default_config_path.display()),
                    cli::PathArgs::Packages => {
                        println!("{}", &paths.packages.display());
                    }
                    cli::PathArgs::Runtime => {
                        if let Some(runtime_dir) = if is_portable_mode() {
                            get_portable_runtime_dir()
                        } else {
                            get_default_runtime_dir()
                        } {
                            println!("{}", runtime_dir.display());
                        }
                    }
                }
            }
            0
        }
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
            package::package_main(cli::PackageArgs::Uninstall(uninstall_args), paths)
        }
        cli::Command::Workaround(_unused_workaround_args) => {
            workaround::workaround_main(_unused_workaround_args)
        }
        cli::Command::Worker(worker_args) => worker::worker_main(
            paths,
            config_store,
            match_store,
            worker_args.monitor_daemon,
            worker_args.start_reason,
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

// TODO: check if the logs for daemon+worker works fine
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

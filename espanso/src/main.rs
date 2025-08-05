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
mod patch;
mod path;
mod preferences;
mod util;

use clap::Parser;
use cli::LogMode;
use config::load_config;
use log::{info, LevelFilter};
use logging::FileProxy;
use path::resolve_paths;
use simplelog::{
    CombinedLogger, ConfigBuilder, SharedLogger, TermLogger, TerminalMode, WriteLogger,
};

use crate::{cli::log::log_main, path::Paths};

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
            "%H:%M:%S [{}({})]",
            // args.command,
            "some command",
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

    let paths: path::Paths;

    // let force_config_path = get_path_override(
    //     &cli_module_args.cli_args.clone().unwrap(),
    //     "config_dir",
    //     "ESPANSO_CONFIG_DIR",
    // );
    // let force_package_path = get_path_override(
    //     &cli_module_args.cli_args.clone().unwrap(),
    //     "package_dir",
    //     "ESPANSO_PACKAGE_DIR",
    // );
    // let force_runtime_path = get_path_override(
    //     &cli_module_args.cli_args.clone().unwrap(),
    //     "runtime_dir",
    //     "ESPANSO_RUNTIME_DIR",
    // );

    // TODO: resolve this
    paths = resolve_paths(None, None, None);

    let config_result = load_config(&paths.config).expect("unable to load config");

    // just a box to store the exite code
    let exit_code: i32;

    exit_code = match args.command {
        cli::Command::Cmd(subcmd) => {
            println!("something anything");
            args_hashmap.insert("command", "cmd");
            match subcmd {
                cli::CmdCommand::Disable => {
                    args_hashmap.insert("subcommand", "disable");
                    1 // TODO
                }
                cli::CmdCommand::Enable => {
                    args_hashmap.insert("subcommand", "enable");
                    1 // TODO
                }
                cli::CmdCommand::Search => {
                    args_hashmap.insert("subcommand", "search");
                    1 // TODO
                }
                cli::CmdCommand::Toggle => {
                    args_hashmap.insert("subcommand", "toggle");
                    1 // TODO
                }
            }
        }
        cli::Command::Edit { target_file } => {
            args_hashmap.insert("command", "edit");
            if let Some(_file) = target_file {
                // args_hashmap.insert("edit-file", file.clone().as_str());
            } else {
                args_hashmap.insert("edit-file", "empty");
                println!("`espanso edit` (empty) was passed");
            };
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
        cli::Command::Launch {} => {
            println!("some dummy output");

            #[cfg(target_os = "macos")]
            espanso_mac_utils::convert_to_foreground_app();

            1 // TODO
        }
        cli::Command::Log {} => {
            println!("some dummy output");
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
            };

            1 // TODO
        }
        cli::Command::Package { .. } => {
            println!("some dummy output");
            1 // TODO
        }
        cli::Command::Path { .. } => {
            // if cli_args.subcommand_matches("config").is_some() {
            //     println!("{}", paths.config.to_string_lossy());
            // } else if cli_args.subcommand_matches("packages").is_some() {
            //     println!("{}", paths.packages.to_string_lossy());
            // } else if cli_args.subcommand_matches("data").is_some()
            //     || cli_args.subcommand_matches("runtime").is_some()
            // {
            //     println!("{}", paths.runtime.to_string_lossy());
            // } else if cli_args.subcommand_matches("default").is_some() {
            //     println!(
            //         "{}",
            //         paths
            //             .config
            //             .join("config")
            //             .join("default.yml")
            //             .to_string_lossy()
            //     );
            // } else if cli_args.subcommand_matches("base").is_some() {
            //     println!(
            //         "{}",
            //         paths
            //             .config
            //             .join("match")
            //             .join("base.yml")
            //             .to_string_lossy()
            //     );
            // } else {
            //     println!("Config: {}", paths.config.to_string_lossy());
            //     println!("Packages: {}", paths.packages.to_string_lossy());
            //     println!("Runtime: {}", paths.runtime.to_string_lossy());
            // }

            // 0
            1 // TODO
        }
        cli::Command::Restart {} => {
            println!("some dummy output");
            1 // TODO
        }
        cli::Command::Service(..) => {
            println!("some dummy output");
            1 // TODO
        }
        cli::Command::Start(..) => {
            println!("some dummy output");
            1 // TODO
        }
        cli::Command::Status {} => {
            println!("some dummy output");
            1 // TODO
        }
        cli::Command::Stop(..) => {
            println!("some dummy output");
            1 // TODO
        }
        cli::Command::Uninstall(..) => {
            println!("some dummy output");
            1 // TODO
        }
        cli::Command::Workaround(..) => {
            println!("some dummy output");
            1 // TODO
        }
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

/// if you pass Config, Package or Runtime returns you the path
// fn get_path_override(matches: &ArgMatches, argument: &str, env_var: &str) -> Option<PathBuf> {
//     if let Some(path) = matches.value_of(argument) {
//         let path = PathBuf::from(path.trim());
//         if path.is_dir() {
//             Some(path)
//         } else {
//             error_eprintln!("{} argument was specified, but it doesn't point to a valid directory. Make sure to create it first.", argument);
//             std::process::exit(1);
//         }
//     } else if let Ok(path) = std::env::var(env_var) {
//         let path = PathBuf::from(path.trim());
//         if path.is_dir() {
//             Some(path)
//         } else {
//             error_eprintln!("{} env variable was specified, but it doesn't point to a valid directory. Make sure to create it first.", env_var);
//             std::process::exit(1);
//         }
//     } else {
//         None
//     }
// }

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

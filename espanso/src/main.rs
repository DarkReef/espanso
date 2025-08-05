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

use std::{collections::HashMap, path::PathBuf, process::Command};

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
use cli::{ArgMatches, CliModule, CliModuleArgs, LogMode};
use config::{load_config, ConfigLoadResult};
use log::LevelFilter;
use logging::FileProxy;
use path::resolve_paths;
use simplelog::{
    CombinedLogger, ConfigBuilder, SharedLogger, TermLogger, TerminalMode, WriteLogger,
};

const LOG_FILE_NAME: &str = "espanso.log";

fn main() {
    util::attach_console();

    let cli_handlers: Vec<CliModule> = vec![
        //cli::cmd::new(),
        //cli::edit::new(),
        //cli::env_path::new(),
        //cli::launcher::new(),
        cli::log::new(),
        //cli::worker::new(),
        //cli::daemon::new(),
        //cli::modulo::new(),
        //cli::path::new(),
        //cli::service::new(),
        //cli::workaround::new(),
        //cli::package::new(),
        cli::match_cli::new(),
    ];

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

    match args.command {
        cli::Command::Cmd(subcmd) => {
            println!("something anything");
            args_hashmap.insert("command", "cmd");
            match subcmd {
                cli::CmdCommand::Disable => {
                    args_hashmap.insert("subcommand", "disable");
                }
                cli::CmdCommand::Enable => {
                    args_hashmap.insert("subcommand", "enable");
                }
                cli::CmdCommand::Search => {
                    args_hashmap.insert("subcommand", "search");
                }
                cli::CmdCommand::Toggle => {
                    args_hashmap.insert("subcommand", "toggle");
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
        }
        cli::Command::EnvPath(..) => println!("some dummy output"),
        cli::Command::Install { .. } => println!("some dummy output"),
        cli::Command::Launch {} => println!("some dummy output"),
        cli::Command::Log {} => {
            println!("some dummy output");
            args_hashmap.insert("command", "log");
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
                        args_hashmap.insert("exec", exec.as_str());
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
        }
        cli::Command::Package { .. } => println!("some dummy output"),
        cli::Command::Path { .. } => println!("some dummy output"),
        cli::Command::Restart {} => println!("some dummy output"),
        cli::Command::Service(..) => println!("some dummy output"),
        cli::Command::Start(..) => println!("some dummy output"),
        cli::Command::Status {} => println!("some dummy output"),
        cli::Command::Stop(..) => println!("some dummy output"),
        cli::Command::Uninstall(..) => println!("some dummy output"),
        cli::Command::Workaround(..) => println!("some dummy output"),
    }

    #[cfg(target_os = "macos")]
    if handler.show_in_dock {
        espanso_mac_utils::convert_to_foreground_app();
    }

    // just a box to store the exite code
    let exit_code: i32;

    // This is the handler (what effectively runs the cmds)
    //
    // inside the handler, it's the `CliModule.entry` field, which
    // is a function that takes `CliModuleArgs` and returns an `i32`
    // (an `exit_code`)
    let handler: CliModule;
    let config_result: ConfigLoadResult;
    let paths: path::Paths;

    // Given our input parsed via clap, we can construct the `CliModuleArgs`
    // in tiny steps
    let mut cli_module_args: CliModuleArgs = CliModuleArgs {
        // insert the hashmap constructed in the match.command
        cli_args: Some(ArgMatches { args: args_hashmap }),
        // and the defaults
        ..Default::default()
    };

    dbg!(&cli_module_args.cli_args);

    // to compare the list of handlers to the `cli_args`
    for bookshelf_handler in cli_handlers {
        dbg!(&bookshelf_handler);
        if bookshelf_handler.subcommand.clone()
            == *cli_module_args
                .cli_args
                .clone()
                .unwrap()
                .args
                .get("command")
                .unwrap()
        {
            println!("found the handler!");
            handler = bookshelf_handler;
            if handler.requires_paths || handler.requires_config {
                let force_config_path = get_path_override(
                    &cli_module_args.cli_args.clone().unwrap(),
                    "config_dir",
                    "ESPANSO_CONFIG_DIR",
                );
                let force_package_path = get_path_override(
                    &cli_module_args.cli_args.clone().unwrap(),
                    "package_dir",
                    "ESPANSO_PACKAGE_DIR",
                );
                let force_runtime_path = get_path_override(
                    &cli_module_args.cli_args.clone().unwrap(),
                    "runtime_dir",
                    "ESPANSO_RUNTIME_DIR",
                );

                paths = resolve_paths(
                    force_config_path.as_deref(),
                    force_package_path.as_deref(),
                    force_runtime_path.as_deref(),
                );
                cli_module_args.paths = Some(paths.clone());

                if handler.requires_config {
                    let config_result = load_config(&paths.config).expect("unable to load config");

                    cli_args.config_store = Some(config_result.config_store);
                    cli_args.match_store = Some(config_result.match_store);
                    cli_args.non_fatal_errors = config_result.non_fatal_errors;
                }

                if handler.enable_logs {
                    log_proxy
                        .set_output_file(
                            &paths.runtime.join(LOG_FILE_NAME),
                            handler.log_mode == LogMode::Read,
                            handler.log_mode == LogMode::CleanAndAppend,
                        )
                        .expect("unable to set up log output file");
                }

                cli_args.paths = Some(paths);
            }

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

            if let Some(args) = matches.subcommand_matches(&handler.subcommand) {
                cli_args.cli_args = Some(args.clone());
            }

            let exit_code = (handler.entry)(cli_args);

            std::process::exit(exit_code);
        }
    }
}

/// if you pass Config, Package or Runtime returns you the path
fn get_path_override(matches: &ArgMatches, argument: &str, env_var: &str) -> Option<PathBuf> {
    if let Some(path) = matches.value_of(argument) {
        let path = PathBuf::from(path.trim());
        if path.is_dir() {
            Some(path)
        } else {
            error_eprintln!("{} argument was specified, but it doesn't point to a valid directory. Make sure to create it first.", argument);
            std::process::exit(1);
        }
    } else if let Ok(path) = std::env::var(env_var) {
        let path = PathBuf::from(path.trim());
        if path.is_dir() {
            Some(path)
        } else {
            error_eprintln!("{} env variable was specified, but it doesn't point to a valid directory. Make sure to create it first.", env_var);
            std::process::exit(1);
        }
    } else {
        None
    }
}

/// # Aliases pre-processing
///
/// Before clap gets to parse the arguments, we want to work with them. This is
/// because clap is unable to alias one subcommand to a different (upper) level
/// of the same command.
/// I found App::visible_alias("alias") but it only works on the same level,
/// like:
/// `espanso service start` for `espanso service st`
fn preprocess_aliases(mut args: Vec<String>) -> Vec<String> {
    // make sure the vec is not empty
    debug_assert!(
        !args.is_empty(),
        "Preprocess aliases got an empty vec! {args:#?}"
    );

    if args.len() >= 2 {
        // Find the first non-flag argument (the command)
        let mut command_index = None;
        for (i, arg) in args.iter().enumerate().skip(1) {
            if !arg.starts_with('-') {
                command_index = Some(i);
                break;
            }
        }

        if let Some(index) = command_index {
            // Clone the command string to avoid borrowing issues
            let command = args[index].clone();

            // Check if this is already a proper subcommand structure
            // (e.g., "espanso service start" should not be transformed)
            let is_already_expanded = if index + 1 < args.len() {
                matches!(command.as_str(), "service" | "package")
            } else {
                false
            };

            if !is_already_expanded {
                match command.as_str() {
                    "start" | "restart" | "stop" | "status" => {
                        args[index] = "service".to_string();
                        args.insert(index + 1, command);
                    }
                    "install" | "uninstall" => {
                        args[index] = "package".to_string();
                        args.insert(index + 1, command);
                    }
                    _ => {
                        // No transformation needed
                    }
                }
            }
        }
    }
    args
}

#[cfg(test)]
mod tests {
    use super::preprocess_aliases;

    #[test]
    fn test_preprocess_aliases_service_start() {
        let args = vec!["espanso".to_string(), "start".to_string()];
        let result = preprocess_aliases(args);
        assert_eq!(result, vec!["espanso", "service", "start"]);
    }

    #[test]
    fn test_preprocess_aliases_service_restart() {
        let args = vec!["espanso".to_string(), "restart".to_string()];
        let result = preprocess_aliases(args);
        assert_eq!(result, vec!["espanso", "service", "restart"]);
    }

    #[test]
    fn test_preprocess_aliases_service_stop() {
        let args = vec!["espanso".to_string(), "stop".to_string()];
        let result = preprocess_aliases(args);
        assert_eq!(result, vec!["espanso", "service", "stop"]);
    }

    #[test]
    fn test_preprocess_aliases_service_status() {
        let args = vec!["espanso".to_string(), "status".to_string()];
        let result = preprocess_aliases(args);
        assert_eq!(result, vec!["espanso", "service", "status"]);
    }

    #[test]
    fn test_preprocess_aliases_package_install() {
        let args = vec!["espanso".to_string(), "install".to_string()];
        let result = preprocess_aliases(args);
        assert_eq!(result, vec!["espanso", "package", "install"]);
    }

    #[test]
    fn test_preprocess_aliases_package_uninstall() {
        let args = vec!["espanso".to_string(), "uninstall".to_string()];
        let result = preprocess_aliases(args);
        assert_eq!(result, vec!["espanso", "package", "uninstall"]);
    }

    #[test]
    fn test_preprocess_aliases_with_additional_args() {
        let args = vec![
            "espanso".to_string(),
            "start".to_string(),
            "--unmanaged".to_string(),
        ];
        let result = preprocess_aliases(args);
        assert_eq!(result, vec!["espanso", "service", "start", "--unmanaged"]);
    }

    #[test]
    fn test_preprocess_aliases_no_alias_needed() {
        let args = vec![
            "espanso".to_string(),
            "service".to_string(),
            "start".to_string(),
        ];
        let result = preprocess_aliases(args);
        assert_eq!(result, vec!["espanso", "service", "start"]);
    }

    #[test]
    fn test_preprocess_aliases_unknown_command() {
        let args = vec!["espanso".to_string(), "unknown".to_string()];
        let result = preprocess_aliases(args);
        assert_eq!(result, vec!["espanso", "unknown"]);
    }

    #[test]
    fn test_preprocess_aliases_only_program_name() {
        let args = vec!["espanso".to_string()];
        let result = preprocess_aliases(args);
        assert_eq!(result, vec!["espanso"]);
    }

    #[test]
    fn test_preprocess_aliases_preserves_case() {
        let args = vec!["espanso".to_string(), "START".to_string()];
        let result = preprocess_aliases(args);
        // Should not match since we're checking exact string match
        assert_eq!(result, vec!["espanso", "START"]);
    }

    #[test]
    fn test_preprocess_aliases_install_with_package_name() {
        let args = vec![
            "espanso".to_string(),
            "install".to_string(),
            "my-package".to_string(),
        ];
        let result = preprocess_aliases(args);
        assert_eq!(result, vec!["espanso", "package", "install", "my-package"]);
    }

    #[test]
    fn test_preprocess_aliases_skips_vebose_argument() {
        let args = vec!["espanso".to_string(), "-v".to_string(), "start".to_string()];
        let result = preprocess_aliases(args);
        assert_eq!(result, vec!["espanso", "-v", "service", "start"]);
    }
}

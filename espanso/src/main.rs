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

use std::{collections::HashMap, path::PathBuf};

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
use espanso_path::Paths;
use log::LevelFilter;
use logging::FileProxy;
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
    //cli::log::new(),
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
        cli::MatchArgs::Exec { .. } => {
          args_hashmap.insert("subcommand", "exec");
        }
        cli::MatchArgs::List(_flags) => {
          args_hashmap.insert("subcommand", "list");
          // you should match multiple times for all the passed flags
          // if (_flags.json) {
          //   args_hashmap.insert("flag", "json");
          //   ...
          // }
        } // args_hashmap.insert("list", _flags);
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
  let paths: Paths;

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

        paths = espanso_path::resolve_paths(
          force_config_path.as_deref(),
          force_package_path.as_deref(),
          force_runtime_path.as_deref(),
        );
        cli_module_args.paths = Some(paths.clone());

        if handler.requires_config {
          config_result = load_config(&paths.config).expect("unable to load config");
          cli_module_args.config_store = Some(config_result.config_store);
          cli_module_args.match_store = Some(config_result.match_store);
          cli_module_args.non_fatal_errors = config_result.non_fatal_errors;
        }

        if handler.enable_logs {
          log_proxy
            .set_output_file(
              &paths.runtime.join(LOG_FILE_NAME),
              handler.log_mode == LogMode::Read,
              handler.log_mode == LogMode::CleanAndAppend,
            )
            .expect("unable to set up output log file");
        }
      }

      exit_code = (handler.entry)(cli_module_args);
      dbg!(exit_code);
      std::process::exit(exit_code);
    }
  }

  println!("something should have been catched...");
  std::process::exit(1);
}

/// if you pass Config, Package or Runtime returns you the path to that file
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

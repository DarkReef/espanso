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

use std::path::PathBuf;

mod cli;
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
use cli::CliModuleArgs;
use log::LevelFilter;
use logging::FileProxy;
use simplelog::{
  CombinedLogger, ConfigBuilder, SharedLogger, TermLogger, TerminalMode, WriteLogger,
};

const LOG_FILE_NAME: &str = "espanso.log";

fn main() {
  util::attach_console();

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
    // Trace mode is only available in debug mode for security reasons
    #[cfg(debug_assertions)]
    3 => LevelFilter::Trace,
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

  match args.command {
    cli::Command::Cmd { .. } => println!("something anything"),
    cli::Command::Edit { target_file } => {
      if target_file.is_some() {
        println!("the file {:#?}", target_file)
      } else {
        println!("`espanso edit` (empty) was passed")
      }
    }
    cli::Command::EnvPath(..) => println!("some dummy output"),
    cli::Command::Install { .. } => println!("some dummy output"),
    cli::Command::Log {} => println!("some dummy output"),
    cli::Command::Match { .. } => println!("some dummy output"),
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

  let _cli_args: CliModuleArgs = CliModuleArgs::default();

  let exit_code = 0;

  std::process::exit(exit_code);
}

/// if you pass `Config`, `Package` or `Runtime` returns you the path
/// # TODO!
fn get_path_override(_: u8, argument: &str, env_var: &str) -> Option<PathBuf> {
  if true {
    let path = PathBuf::from(argument.trim());
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

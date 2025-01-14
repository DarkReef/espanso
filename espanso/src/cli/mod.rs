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

use std::{path::PathBuf, u64};

use clap::{Parser, Subcommand};
use espanso_config::{config::ConfigStore, error::NonFatalErrorSet, matches::store::MatchStore};
use espanso_path::Paths;

//pub mod cmd;
//pub mod daemon;
//pub mod edit;
//pub mod env_path;
//pub mod launcher;
//pub mod log;
//pub mod match_cli;
//pub mod modulo;
//pub mod package;
//pub mod path;
//pub mod service;
//pub mod util;
//pub mod workaround;
//pub mod worker;
//
const VERSION: &str = env!("CARGO_PKG_VERSION");


#[derive(Parser)]
#[command(name = "espanso")]
#[command(about = "A Privacy-first, Cross-platform Text Expander")]
#[command(version = VERSION)]
#[command(long_about=None)]
#[command(arg_required_else_help = true)]
pub struct Arguments {
  #[command(subcommand)]
  pub command: Command,
  //command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Command {
  /// Send a command to the espanso daemon
  #[clap(subcommand)]
  Cmd(CmdCommand),
  /// Shortcut to open the default text editor to edit config files
  Edit {
    /// lists test values
    #[arg(short, long)]
    list: bool,
  },
  /// Add or remove the 'espanso' command from the PATH
  #[clap(subcommand)]
  EnvPath(EnvPathCommand),
  /// Prints this message or the help of the given subcommand(s)
  //Help,
  /// Install a package
  Install { package_name: String },
  /// Print the daemon logs
  Log,
  /// List and execute matches from the CLI
  Match,
  /// Automatically migrate legacy config files to the new v2 format
  Migrate,
  /// Package-management commands
  Package,
  /// Prints all the espanso directory paths to easily locate configuration and matches
  Path,
  /// Restart the espanso service
  Restart,
  /// A collection of commands to manage the Espanso service (for example, enabling auto-start on system boot).
  #[clap(subcommand)]
  Service(ServiceCmd),
  /// Start espanso as a service
  Start,
  /// Check if the espanso daemon is running or not
  Status,
  /// Stop espanso service
  Stop,
  /// Remove a package
  Uninstall,
  /// A collection of workarounds to solve some common problems
  Workaround,
}

#[derive(Subcommand, Debug)]
pub enum ServiceCmd {
  /// Check if espanso is registered as a system service
  Check,
  // Prints this message or the help of the given subcommand(s)
  // Help,
  /// Register espanso as a system service
  Register,
  ///Restart the espanso service
  Restart,
  /// Start espanso as a service
  Start,
  /// Check if the espanso daemon is running or not.
  Status,
  /// Stop espanso service
  Stop,
  /// Unregister espanso from system services
  Unregister,
}

#[derive(Subcommand, Debug)]
pub enum CmdCommand {
  /// Disable expansions
  Disable,
  /// Enable expansions
  Enable,
  // Print this message or the help of the given subcommand(s)
  // Help,
  /// Open the Espanso's search bar
  Search,
  /// Enable/Disable expansions
  Toggle,
}

#[derive(Subcommand, Debug)]
pub enum EnvPathCommand {
  /// Add 'espanso' command to PATH
  Register,
  /// Remove 'espanso' command from PATH
  Unregister,
  // Print this message or the help of the given subcommand(s)
  // Help,
}

#[allow(dead_code)]
pub struct CliModule {
  pub enable_logs: bool,
  pub disable_logs_terminal_output: bool,
  pub log_mode: LogMode,
  pub requires_paths: bool,
  pub requires_config: bool,
  pub subcommand: String,
  pub show_in_dock: bool,
  pub requires_linux_capabilities: bool,
  pub entry: fn(CliModuleArgs) -> i32,
}

impl Default for CliModule {
  fn default() -> Self {
    Self {
      enable_logs: false,
      log_mode: LogMode::Read,
      disable_logs_terminal_output: false,
      requires_paths: false,
      requires_config: false,
      subcommand: String::new(),
      show_in_dock: false,
      requires_linux_capabilities: false,
      entry: |_| 0,
    }
  }
}

#[derive(Debug, PartialEq, Eq)]
pub enum LogMode {
  Read,
  AppendOnly,
  CleanAndAppend,
}

// TODO
enum ArgMatches {
  Something
}

impl ArgMatches {
  pub fn subcommand_matches(&self, _: &str) -> Option<u64> {
    todo!()
  }

  pub fn value_of(&self, _:&str)-> Option<&str>{
    todo!()
  }
  pub fn is_present(&self, _:&str)-> bool{
    todo!();
    true
  }
}

#[derive(Default)]
pub struct CliModuleArgs {
  pub config_store: Option<Box<dyn ConfigStore>>,
  pub match_store: Option<Box<dyn MatchStore>>,
  pub is_legacy_config: bool,
  pub non_fatal_errors: Vec<NonFatalErrorSet>,
  pub paths: Option<Paths>,
  pub paths_overrides: Option<PathsOverrides>,
  pub cli_args: Option<ArgMatches>,
}

pub struct PathsOverrides {
  pub config: Option<PathBuf>,
  pub runtime: Option<PathBuf>,
  pub packages: Option<PathBuf>,
}

pub struct CliAlias {
  pub subcommand: String,
  pub forward_into: String,
}

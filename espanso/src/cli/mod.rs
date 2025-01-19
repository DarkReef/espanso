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

use std::{collections::HashMap, path::PathBuf};

use clap::{Args, Parser, Subcommand};
use espanso_config::{config::ConfigStore, error::NonFatalErrorSet, matches::store::MatchStore};
use espanso_path::Paths;

//pub mod cmd;
//pub mod daemon;
//pub mod edit;
//pub mod env_path;
//pub mod launcher;
pub mod log;
pub mod match_cli;
//pub mod modulo;
//pub mod package;
//pub mod path;
//pub mod service;
//pub mod util;
//pub mod workaround;
//pub mod worker;
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

  #[arg(short, long, action = clap::ArgAction::Count)]
  // Sets the level of verbosity
  pub verbose: u8,
}

#[derive(Subcommand, Debug)]
pub enum Command {
  /// Send a command to the espanso daemon
  #[clap(subcommand)]
  Cmd(CmdCommand),
  /// Shortcut to open the default text editor to edit config files
  Edit {
    /// Defaults to "match/base.yml". It contains the relative path of the file you
    /// want to edit, such as 'config/default.yml' or 'match/base.yml'.
    /// For convenience, you can also specify the name directly and espanso will
    /// figure out the path. For example, specifying 'email' is equivalent to 'match/email.yml'.
    target_file: Option<String>,
  },
  /// Add or remove the 'espanso' command from the PATH
  EnvPath(EnvPathArgs),
  /// Install a package
  Install(InstallArgs),
  /// should not appear in the CLI menu...
  #[clap(subcommand, hide = true)]
  Launch,
  /// Print the daemon logs
  Log,
  /// List and execute matches from the CLI
  #[clap(subcommand)]
  Match(MatchArgs),
  /// Package-management commands
  #[clap(subcommand)]
  Package(PackageArgs),
  /// Prints all the espanso directory paths to easily locate configuration and matches
  #[clap(subcommand)]
  Path(PathArgs),
  /// Restart the espanso service
  Restart,
  /// A collection of commands to manage the Espanso service (for example, enabling auto-start on system boot).
  #[clap(subcommand)]
  Service(ServiceCommand),
  /// Start espanso as a service
  Start(ServiceCommandUnmanagedFlag),
  /// Check if the espanso daemon is running or not
  Status,
  /// Stop espanso service
  Stop(ServiceCommandUnmanagedFlag),
  /// Remove a package
  Uninstall(UninstallArgs),
  /// A collection of workarounds to solve some common problems
  #[clap(subcommand)]
  Workaround(WorkaroundArgs),
}

#[derive(Subcommand, Debug)]
pub enum CmdCommand {
  /// Disable expansions
  Disable,
  /// Enable expansions
  Enable,
  /// Open the Espanso's search bar
  Search,
  /// Enable/Disable expansions
  Toggle,
}

#[derive(Args, Debug)]
#[command(args_conflicts_with_subcommands = true)]
#[command(arg_required_else_help = true)]
pub struct EnvPathArgs {
  #[command(subcommand)]
  pub command: Option<EnvPathSubCommand>,

  #[arg(short, long)]
  pub prompt: bool,
}

#[derive(Debug, Subcommand)]
pub enum EnvPathSubCommand {
  /// Add 'espanso' command to PATH
  Register,
  /// Remove 'espanso' command from PATH
  Unregister,
}

#[allow(clippy::struct_excessive_bools)] // you need this for the flags
#[derive(Args, Debug)]
#[command(args_conflicts_with_subcommands = true)]
#[command(arg_required_else_help = true)]
pub struct InstallArgs {
  /// Package name
  pub package_name: String,

  /// Allow installing packages from non-verified repositories
  #[arg(short, long)]
  pub external: bool,

  /// Overwrite the package if already installed
  #[arg(short, long)]
  pub force: bool,

  /// Git repository from which espanso should install the package
  #[arg(short, long)]
  pub git_repo: Option<String>,

  /// Force espanso to search for the package on a specific git branch
  #[arg(short = 'b', long)]
  pub git_branch: Option<String>,

  /// Request a fresh copy of the Espanso Hub package index instead of using the cached version
  #[arg(short, long)]
  pub refresh_index: bool,

  /// If specified, espanso will use the 'git' command instead of trying direct methods
  #[arg(short, long)]
  pub use_native_git: bool,

  /// Force a particular version to be installed instead of the latest available
  #[arg(short, long)]
  pub version: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum MatchArgs {
  /// Triggers the expansion of a match
  Exec {
    /// Trigger you want to activate
    trigger: String,
  },
  /// Print matches to standard output
  List(MatchListCommand),
}

#[derive(Args, Debug)]
#[command(args_conflicts_with_subcommands = true)]
pub struct MatchListCommand {
  /// Output matches to the JSON format
  #[arg(short, long)]
  pub json: bool,

  /// Print only triggers without replacement. Does nothing when using JSON format
  #[arg(short = 't', long)]
  pub only_triggers: bool,

  /// Preserve newlines when printing replacements. Does nothing when using JSON format
  #[arg(short = 'n', long)]
  pub preserve_newlines: bool,
}

#[derive(Debug, Subcommand)]
#[command(arg_required_else_help = true)]
pub enum PackageArgs {
  /// Install a package
  Install(InstallArgs),
  /// List all installed packages
  List,
  /// Remove a package
  Uninstall(UninstallArgs),
  /// Update a package. If 'all' is passed as package name, attempts to update all packages.
  Update {
    /// Package name or 'all'
    package_name_or_all: String,
  },
}

#[derive(Args, Debug)]
#[command(args_conflicts_with_subcommands = true)]
#[command(arg_required_else_help = true)]
pub struct UninstallArgs {
  /// Package name
  package_name: String,
}

#[derive(Debug, Subcommand)]
pub enum PathArgs {
  /// Print the default match file path.
  Base,
  /// Print the current config folder path.
  Config,
  /// Print the default configuration file path.
  Default,
  /// Print the current packages folder path.
  Packages,
  /// Print the current runtime folder path.
  Runtime,
}

#[derive(Subcommand, Debug)]
pub enum ServiceCommand {
  /// Check if espanso is registered as a system service
  Check,
  /// Register espanso as a system service
  Register,
  ///Restart the espanso service
  Restart(ServiceCommandUnmanagedFlag),
  /// Start espanso as a service
  Start(ServiceCommandUnmanagedFlag),
  /// Check if the espanso daemon is running or not.
  Status,
  /// Stop espanso service
  Stop,
  /// Unregister espanso from system services
  Unregister,
}

#[derive(Args, Debug)]
pub struct ServiceCommandUnmanagedFlag {
  /// Run espanso as an unmanaged service (avoid system manager)
  #[arg(long)]
  pub unmanaged: bool,
}

#[derive(Subcommand, Debug)]
pub enum WorkaroundArgs {
  /// Attempt to disable secure input by automating the common steps.
  SecureInput,
}

#[allow(dead_code)]
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
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

/// Custom struct to wrap the old clap v3 `ArgMatches`
#[derive(Debug, Clone)]
pub struct ArgMatches<'a> {
  pub args: HashMap<&'a str, &'a str>,
}

impl ArgMatches<'_> {
  // pub fn new() -> Self {
  //   ArgMatches {
  //     args: HashMap::new(),
  //   }
  // }

  pub fn subcommand_matches(&self, id: &str) -> Option<&ArgMatches> {
    if std::convert::AsRef::<str>::as_ref(self.args.get("subcommand").unwrap()) == id {
      return Some(self);
    }
    None
  }

  pub fn value_of(&self, lookup_key: &str) -> Option<&str> {
    for (key, value) in &self.args {
      if *key == lookup_key {
        return Some(value);
      }
    }
    None
  }
  pub fn is_present(&self, lookup: &str) -> bool {
    for key in self.args.keys() {
      if *key == lookup {
        return true;
      }
    }
    false
  }
}

#[derive(Default)]
pub struct CliModuleArgs<'a> {
  pub config_store: Option<Box<dyn ConfigStore>>,
  pub match_store: Option<Box<dyn MatchStore>>,
  pub is_legacy_config: bool,
  pub non_fatal_errors: Vec<NonFatalErrorSet>,
  pub paths: Option<Paths>,
  pub paths_overrides: Option<PathsOverrides>,
  pub cli_args: Option<ArgMatches<'a>>,
}

// impl<'a> CliModuleArgs<'a> {
//   pub fn new(
//     config_store: Option<Box<dyn ConfigStore>>,
//     match_store: Option<Box<dyn MatchStore>>,
//     is_legacy_config: bool,
//     non_fatal_errors: Vec<NonFatalErrorSet>,
//     paths: Option<Paths>,
//     paths_overrides: Option<PathsOverrides>,
//     cli_args: Option<ArgMatches<'a>>,
//   ) -> Self {
//     Self {
//       config_store,
//       match_store,
//       is_legacy_config,
//       non_fatal_errors,
//       paths,
//       paths_overrides,
//       cli_args,
//     }
//   }
// }

#[derive(Debug)]
pub struct PathsOverrides {
  pub config: Option<PathBuf>,
  pub runtime: Option<PathBuf>,
  pub packages: Option<PathBuf>,
}

pub struct CliAlias {
  pub subcommand: String,
  pub forward_into: String,
}

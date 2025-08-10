# Testing document

Using [trycmd](https://docs.rs/trycmd/latest/trycmd/) we can test the ouput of
the commands

Each of the code blocks here are used to test the binary. In the `trycmd`
documentation is [the complete reference](https://docs.rs/trycmd/latest/trycmd/#trycmd)
but to have a quick recap:

- commands must be inside triple backticks and have the `console` syntax
- commands sent start with `$` (like user commands in a linux shell)
- after the command, you can specify the exit code with `? <exit_code>`
- if the command ends with a new line (next prompt) it must have a new line
here

## `espanso`

```console
$ espanso
? 2
espanso 2.2.4

A Privacy-first, Cross-platform Text Expander

espanso [OPTIONS] <COMMAND>

Commands:
  cmd         Send a command to the espanso daemon
  edit        Shortcut to open the default text editor to edit config files
  env-path    Add or remove the 'espanso' command from the PATH
  install     Install a package
  log         Print the daemon logs
  match       List and execute matches from the CLI
  package     Package-management commands
  path        Prints all the espanso directory paths to easily locate configuration and matches
  restart     Restart the espanso service
  service     A collection of commands to manage the Espanso service (for example, enabling auto-start on system boot)
  start       Start espanso as a service
  status      Check if the espanso daemon is running or not
  stop        Stop espanso service
  uninstall   Remove a package
  workaround  A collection of workarounds to solve some common problems
  help        Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose...                 
  -c, --config-dir <CONFIG_DIR>    
  -p, --package-dir <PACKAGE_DIR>  
  -r, --runtime-dir <RUNTIME_DIR>  
  -h, --help                       Print help
  -V, --version                    Print version

Federico Terzi and the espanso contributors

```

## `espanso cmd --help`

```console
$ espanso cmd --help
? 0
Send a command to the espanso daemon

Usage: espanso cmd <COMMAND>

Commands:
  disable  Disable expansions
  enable   Enable expansions
  search   Open the Espanso's search bar
  toggle   Enable/Disable expansions
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

```

### `espanso cmd disable --help`

```console
$ espanso cmd disable --help
? 0
Disable expansions

Usage: espanso cmd disable

Options:
  -h, --help  Print help

```

### `espanso cmd enable --help`

It doesn't catch the help menu. It just enables espanso

### `espanso cmd search --help`

```console
$ espanso cmd search --help
? 0
Open the Espanso's search bar

Usage: espanso cmd search

Options:
  -h, --help  Print help

```

### `espanso cmd toggle --help`

```console
$ espanso cmd toggle --help
? 0
Enable/Disable expansions

Usage: espanso cmd toggle

Options:
  -h, --help  Print help

```

## `espanso edit --help`

```console
$ espanso edit --help
? 0
Shortcut to open the default text editor to edit config files

Usage: espanso edit [TARGET_FILE]

Arguments:
  [TARGET_FILE]  Defaults to "match/base.yml". It contains the relative path of the file you want to edit, such as 'config/default.yml' or 'match/base.yml'. For convenience, you can also specify the name directly and espanso will figure out the path. For example, specifying 'email' is equivalent to 'match/email.yml'

Options:
  -h, --help  Print help

```

## `espanso env-path --help`

```console
$ espanso env-path --help
? 0
Add or remove the 'espanso' command from the PATH

Usage: espanso env-path [OPTIONS]
       espanso env-path <COMMAND>

Commands:
  register    Add 'espanso' command to PATH
  unregister  Remove 'espanso' command from PATH
  help        Print this message or the help of the given subcommand(s)

Options:
  -p, --prompt  
  -h, --help    Print help

```

### `espanso env-path register --help`

```console
$ espanso env-path register --help
? 0
Add 'espanso' command to PATH

Usage: espanso env-path register

Options:
  -h, --help  Print help

```

### `espanso env-path unregister --help`

```console
$ espanso env-path unregister --help
? 0
Remove 'espanso' command from PATH

Usage: espanso env-path unregister

Options:
  -h, --help  Print help

```

### `espanso env-path --prompt --help`

```console
$ espanso env-path --prompt --help
? 0
Add or remove the 'espanso' command from the PATH

Usage: espanso env-path [OPTIONS]
       espanso env-path <COMMAND>

Commands:
  register    Add 'espanso' command to PATH
  unregister  Remove 'espanso' command from PATH
  help        Print this message or the help of the given subcommand(s)

Options:
  -p, --prompt  
  -h, --help    Print help

```

## `espanso install --help`

```console
$ espanso install --help
? 0
Install a package

Usage: espanso install [OPTIONS] <PACKAGE_NAME>

Arguments:
  <PACKAGE_NAME>  Package name

Options:
  -e, --external                 Allow installing packages from non-verified repositories
  -f, --force                    Overwrite the package if already installed
  -g, --git-repo <GIT_REPO>      Git repository from which espanso should install the package
  -b, --git-branch <GIT_BRANCH>  Force espanso to search for the package on a specific git branch
  -r, --refresh-index            Request a fresh copy of the Espanso Hub package index instead of using the cached version
  -u, --use-native-git           If specified, espanso will use the 'git' command instead of trying direct methods
  -v, --version <VERSION>        Force a particular version to be installed instead of the latest available
  -h, --help                     Print help

```

## `espanso log --help`

```console
$ espanso log --help
? 0
Print the daemon logs

Usage: espanso log

Options:
  -h, --help  Print help

```

## `espanso match --help`

```console
$ espanso match --help
? 0
List and execute matches from the CLI

Usage: espanso match <COMMAND>

Commands:
  exec  Triggers the expansion of a match
  list  Print matches to standard output
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

```

## `espanso match exec --help`

```console
$ espanso match exec --help
? 0
Triggers the expansion of a match

Usage: espanso match exec [OPTIONS] --trigger <TRIGGER>

Options:
      --trigger <TRIGGER>  Trigger you want to activate
      --args <ARGS>        Specify also an argument for the expansion, following the 'name=value' format. You can specify multiple ones
  -h, --help               Print help

```

## `espanso match list --help`

```console
$ espanso match list --help
? 0
Print matches to standard output

Usage: espanso match list [OPTIONS]

Options:
      --class <CLASS>      Only return matches that would be active with the given class. This is relevant if you want to list matches only active inside an app-specific config
      --exec <EXEC>        Only return matches that would be active with the given exec. This is relevant if you want to list matches only active inside an app-specific config
      --title <TITLE>      Only return matches that would be active with the given title. This is relevant if you want to list matches only active inside an app-specific config
  -j, --json               Output matches to the JSON format
  -t, --only-triggers      Print only triggers without replacement. Does nothing when using JSON format
  -n, --preserve-newlines  Preserve newlines when printing replacements. Does nothing when using JSON format
  -h, --help               Print help

```

## `espanso package --help`

```console
$ espanso package --help
? 0
Package-management commands

Usage: espanso package <COMMAND>

Commands:
  install    Install a package
  list       List all installed packages
  uninstall  Remove a package
  update     Update a package. If 'all' is passed as package name, attempts to update all packages
  help       Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

```

### `espanso package install`

```console
$ espanso package install
? 2
Install a package

Usage: espanso package install [OPTIONS] <PACKAGE_NAME>

Arguments:
  <PACKAGE_NAME>  Package name

Options:
  -e, --external                 Allow installing packages from non-verified repositories
  -f, --force                    Overwrite the package if already installed
  -g, --git-repo <GIT_REPO>      Git repository from which espanso should install the package
  -b, --git-branch <GIT_BRANCH>  Force espanso to search for the package on a specific git branch
  -r, --refresh-index            Request a fresh copy of the Espanso Hub package index instead of using the cached version
  -u, --use-native-git           If specified, espanso will use the 'git' command instead of trying direct methods
  -v, --version <VERSION>        Force a particular version to be installed instead of the latest available
  -h, --help                     Print help

```

### `espanso package list --help`

```console
$ espanso package list --help
? 0
List all installed packages

Usage: espanso package list

Options:
  -h, --help  Print help

```

### `espanso package uninstall --help`

```console
$ espanso package uninstall --help
? 0
Remove a package

Usage: espanso package uninstall <PACKAGE_NAME>

Arguments:
  <PACKAGE_NAME>  Package name

Options:
  -h, --help  Print help

```

### `espanso package update`

```console
$ espanso package update
? 2
error: the following required arguments were not provided:
  <PACKAGE_NAME_OR_ALL>

Usage: espanso package update <PACKAGE_NAME_OR_ALL>

For more information, try '--help'.

```

### `espanso package update --help`

```console
$ espanso package update --help
? 0
Update a package. If 'all' is passed as package name, attempts to update all packages

Usage: espanso package update <PACKAGE_NAME_OR_ALL>

Arguments:
  <PACKAGE_NAME_OR_ALL>  Package name or 'all'

Options:
  -h, --help  Print help

```

## `espanso path`

```console
$ espanso path
? 2
Prints all the espanso directory paths to easily locate configuration and matches

Usage: espanso path <COMMAND>

Commands:
  base      Print the default match file path
  config    Print the current config folder path
  default   Print the default configuration file path
  packages  Print the current packages folder path
  runtime   Print the current runtime folder path
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

```

## `espanso restart --help`

```console
$ espanso restart --help
? 0
Restart the espanso service

Usage: espanso restart [OPTIONS]

Options:
      --unmanaged  Run espanso as an unmanaged service (avoid system manager)
  -h, --help       Print help

```

## `espanso service --help`

```console
$ espanso service --help
? 0
A collection of commands to manage the Espanso service (for example, enabling auto-start on system boot)

Usage: espanso service <COMMAND>

Commands:
  check       Check if espanso is registered as a system service
  register    Register espanso as a system service
  restart     Restart the espanso service
  start       Start espanso as a service
  status      Check if the espanso daemon is running or not
  stop        Stop espanso service
  unregister  Unregister espanso from system services
  help        Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

```

### `espanso service check --help`

```console
$ espanso service check --help
? 0
Check if espanso is registered as a system service

Usage: espanso service check

Options:
  -h, --help  Print help

```

### `espanso service register --help`

```console
$ espanso service register --help
? 0
Register espanso as a system service

Usage: espanso service register

Options:
  -h, --help  Print help

```

### `espanso service restart --help`

```console
$ espanso service restart --help
? 0
Restart the espanso service

Usage: espanso service restart [OPTIONS]

Options:
      --unmanaged  Run espanso as an unmanaged service (avoid system manager)
  -h, --help       Print help

```

### `espanso service start --help`

```console
$ espanso service start --help
? 0
Start espanso as a service

Usage: espanso service start [OPTIONS]

Options:
      --unmanaged  Run espanso as an unmanaged service (avoid system manager)
  -h, --help       Print help

```

### `espanso service status --help`

```console
$ espanso service status --help
? 0
Check if the espanso daemon is running or not

Usage: espanso service status

Options:
  -h, --help  Print help

```

### `espanso service stop --help`

```console
$ espanso service stop --help
? 0
Stop espanso service

Usage: espanso service stop

Options:
  -h, --help  Print help

```

### `espanso service unregister --help`

```console
$ espanso service unregister --help
? 0
Unregister espanso from system services

Usage: espanso service unregister

Options:
  -h, --help  Print help

```

## `espanso start --help`

```console
$ espanso start --help
? 0
Start espanso as a service

Usage: espanso start [OPTIONS]

Options:
      --unmanaged  Run espanso as an unmanaged service (avoid system manager)
  -h, --help       Print help

```

## `espanso status --help`

```console
$ espanso status --help
? 0
Check if the espanso daemon is running or not

Usage: espanso status

Options:
  -h, --help  Print help

```

## `espanso stop --help`

```console
$ espanso stop --help
? 0
Stop espanso service

Usage: espanso stop

Options:
  -h, --help  Print help

```

### `espanso uninstall --help`

```console
$ espanso uninstall --help
? 0
Remove a package

Usage: espanso uninstall <PACKAGE_NAME>

Arguments:
  <PACKAGE_NAME>  Package name

Options:
  -h, --help  Print help

```

## `espanso workaround --help`

```console
$ espanso workaround --help
? 0
A collection of workarounds to solve some common problems

Usage: espanso workaround <COMMAND>

Commands:
  secure-input  Attempt to disable secure input by automating the common steps
  help          Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

```

## `espanso workaround secure-input --help`

```console
$ espanso workaround secure-input --help
? 0
Attempt to disable secure input by automating the common steps

Usage: espanso workaround secure-input

Options:
  -h, --help  Print help

```

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
A Privacy-first, Cross-platform Text Expander

Usage: espanso [OPTIONS] <COMMAND>

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
  -h, --help        Print help
  -V, --version     Print version

```

## `espanso cmd`

```console
$ espanso cmd
? 2
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

### `espanso cmd disable`

```console
$ espanso cmd disable
something anything

```

### `espanso cmd enable`

```console
$ espanso cmd enable
something anything

```

### `espanso cmd search`

```console
$ espanso cmd search
something anything

```

### `espanso cmd toggle`

```console
$ espanso cmd toggle
something anything

```

### `espanso cmd help`

```console
$ espanso cmd help
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

## `espanso edit`

```console
$ espanso edit
`espanso edit` (empty) was passed

```

## `espanso edit some_file`

```console
$ espanso edit some_file
the file Some(
    "some_file",
)

```

## `espanso env-path`

```console
$ espanso env-path
? 2
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

### `espanso env-path register`

```console
$ espanso env-path register
some dummy output

```

### `espanso env-path unregister`

```console
$ espanso env-path unregister
some dummy output

```

### `espanso env-path help`

```console
$ espanso env-path help
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

### `espanso env-path --prompt`

```console
$ espanso env-path --prompt
some dummy output

```

## `espanso help`

```console
$ espanso help
A Privacy-first, Cross-platform Text Expander

Usage: espanso [OPTIONS] <COMMAND>

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
  -h, --help        Print help
  -V, --version     Print version

```

## `espanso install`

```console
$ espanso install
? 2
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

### `espanso install --help`

```console
$ espanso install --help
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

### `espanso install --external a`

```console
$ espanso install --external a
some dummy output

```

### `espanso install -e a`

```console
$ espanso install -e a
some dummy output

```

### `espanso install --force a`

```console
$ espanso install --force a
some dummy output

```

### `espanso install -f a`

```console
$ espanso install -f a
some dummy output

```

### `espanso install --git-repo something a`

```console
$ espanso install --git-repo something a
some dummy output

```

### `espanso install -g something a`

```console
$ espanso install -g something a
some dummy output

```

### `espanso install --git-branch something a`

```console
$ espanso install --git-branch something a
some dummy output

```

### `espanso install -b something a`

```console
$ espanso install -b something a
some dummy output

```

### `espanso install --refresh-index a`

```console
$ espanso install --refresh-index a
some dummy output

```

### `espanso install -r a`

```console
$ espanso install -r a
some dummy output

```

### `espanso install --use-native-git a`

```console
$ espanso install --use-native-git a
some dummy output

```

### `espanso install -u a`

```console
$ espanso install -u a
some dummy output

```

### `espanso install --version 1 a`

```console
$ espanso install --version 1 a
some dummy output

```

### `espanso install -v 1 a`

```console
$ espanso install -v 1 a
some dummy output

```

### `espanso install --version 1`

```console
$ espanso install --version 1
? 2
error: the following required arguments were not provided:
  <PACKAGE_NAME>

Usage: espanso install --version <VERSION> <PACKAGE_NAME>

For more information, try '--help'.

```

### `espanso install -v 1`

```console
$ espanso install -v 1
? 2
error: the following required arguments were not provided:
  <PACKAGE_NAME>

Usage: espanso install --version <VERSION> <PACKAGE_NAME>

For more information, try '--help'.

```

## `espanso install some_package`

```console
$ espanso install some_package
some dummy output

```

### `espanso install dummy_package`

```console
$ espanso install dummy_package
some dummy output

```

## `espanso log`

```console
$ espanso log
some dummy output

```

## `espanso match`

```console
$ espanso match
? 2
List and execute matches from the CLI

Usage: espanso match <COMMAND>

Commands:
  exec  Triggers the expansion of a match
  list  Print matches to standard output
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

```

## `espanso match exec`

```console
$ espanso match exec
? 2
error: the following required arguments were not provided:
  --arg <ARG>

Usage: espanso match exec --arg <ARG>

For more information, try '--help'.

```

### `espanso match exec --arg a`

```console
$ espanso match exec --arg a
some dummy output

```

## `espanso match list`

```console
$ espanso match list
some dummy output

```

### `espanso match list --json`

```console
$ espanso match list --json
some dummy output

```

### `espanso match list --only-triggers`

```console
$ espanso match list --only-triggers
some dummy output

```

### `espanso match list --preserve-newlines`

```console
$ espanso match list --preserve-newlines
some dummy output

```

### `espanso match list --help`

```console
$ espanso match list --help
Print matches to standard output

Usage: espanso match list [OPTIONS]

Options:
  -j, --json               Output matches to the JSON format
  -t, --only-triggers      Print only triggers without replacement
  -n, --preserve-newlines  Preserve newlines when printing replacements. Does nothing when using JSON format
  -h, --help               Print help

```

## `espanso package`

```console
$ espanso package
? 2
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

### `espanso package list`

```console
$ espanso package list
some dummy output

```

### `espanso package list --help`

```console
$ espanso package list --help
List all installed packages

Usage: espanso package list

Options:
  -h, --help  Print help

```

### `espanso package uninstall`

```console
$ espanso package uninstall
? 2
Remove a package

Usage: espanso package uninstall <PACKAGE_NAME>

Arguments:
  <PACKAGE_NAME>  Package name

Options:
  -h, --help  Print help

```

### `espanso package uninstall --help`

```console
$ espanso package uninstall --help
Remove a package

Usage: espanso package uninstall <PACKAGE_NAME>

Arguments:
  <PACKAGE_NAME>  Package name

Options:
  -h, --help  Print help

```

### `espanso package uninstall a`

```console
$ espanso package uninstall a
some dummy output

```

### `espanso package update a`

```console
$ espanso package update a
some dummy output

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
Update a package. If 'all' is passed as package name, attempts to update all packages

Usage: espanso package update <PACKAGE_NAME_OR_ALL>

Arguments:
  <PACKAGE_NAME_OR_ALL>  Package name or 'all'

Options:
  -h, --help  Print help

```

### `espanso package update all`

```console
$ espanso package update all
some dummy output

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

## `espanso path base`

```console
$ espanso path base
some dummy output

```

## `espanso path config`

```console
$ espanso path config
some dummy output

```

## `espanso path default`

```console
$ espanso path default
some dummy output

```

## `espanso path packages`

```console
$ espanso path packages
some dummy output

```

## `espanso path runtime`

```console
$ espanso path runtime
some dummy output

```

## `espanso restart`

```console
$ espanso restart
some dummy output

```

## `espanso service`

```console
$ espanso service
? 2
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

### `espanso service check`

```console
$ espanso service check
some dummy output

```

### `espanso service register`

```console
$ espanso service register
some dummy output

```

### `espanso service restart`

```console
$ espanso service restart
some dummy output

```

### `espanso service restart --unmanaged`

```console
$ espanso service restart --unmanaged
some dummy output

```

### `espanso service start`

```console
$ espanso service start
some dummy output

```

### `espanso service start --unmanaged`

```console
$ espanso service start --unmanaged
some dummy output

```

### `espanso service status`

```console
$ espanso service status
some dummy output

```

### `espanso service stop`

```console
$ espanso service stop
some dummy output

```

### `espanso service unregister`

```console
$ espanso service unregister
some dummy output

```

## `espanso start`

```console
$ espanso start
some dummy output

```

## `espanso status`

```console
$ espanso status
some dummy output

```

## `espanso stop`

```console
$ espanso stop
some dummy output

```

## `espanso uninstall`

```console
$ espanso uninstall
? 2
Remove a package

Usage: espanso uninstall <PACKAGE_NAME>

Arguments:
  <PACKAGE_NAME>  Package name

Options:
  -h, --help  Print help

```

### `espanso uninstall --help`

```console
$ espanso uninstall --help
Remove a package

Usage: espanso uninstall <PACKAGE_NAME>

Arguments:
  <PACKAGE_NAME>  Package name

Options:
  -h, --help  Print help

```

## `espanso workaround`

```console
$ espanso workaround
? 2
A collection of workarounds to solve some common problems

Usage: espanso workaround <COMMAND>

Commands:
  secure-input  Attempt to disable secure input by automating the common steps
  help          Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

```

## `espanso workaround secure-input`

```console
$ espanso workaround secure-input
some dummy output

```

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

`espanso env-path` is tested on files `./env-path.md` and `./env-path-macos.md`

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
error: the following required arguments were not provided:
  <PACKAGE_NAME>

Usage: espanso install <PACKAGE_NAME>

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
some dummy output

```

## `espanso package`

```console
$ espanso package
some dummy output

```

## `espanso path`

```console
$ espanso path
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
some dummy output

```

## `espanso workaround`

```console
$ espanso workaround
some dummy output

```

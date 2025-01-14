# env-path for non-macos systems

because some commands are macos dependant, we test any non-macos here:
checkout `./README.md` for clarifications on the `trycmd` crate


## `espanso env-path`

```console
$ espanso env-path
? 2
Add or remove the 'espanso' command from the PATH

Usage: espanso env-path
       espanso env-path <COMMAND>

Commands:
  register    Add 'espanso' command to PATH
  unregister  Remove 'espanso' command from PATH
  help        Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

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

Usage: espanso env-path
       espanso env-path <COMMAND>

Commands:
  register    Add 'espanso' command to PATH
  unregister  Remove 'espanso' command from PATH
  help        Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

```

### `espanso env-path --prompt`

```console
$ espanso env-path --prompt
error: unexpected argument '--prompt' found

Usage: espanso env-path
       espanso env-path <COMMAND>

For more information, try '--help'.

```

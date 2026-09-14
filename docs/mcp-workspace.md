# MCP workspace editing

rEspanso exposes a sandboxed MCP workspace for agents that need to inspect or change text-expansion rules and Rhai scripts.

## Scope

The MCP process can only access:

- `match/**/*.yml`
- `match/**/*.yaml`
- `scripts/**/*.rhai`

Absolute paths, `..`, symbolic links and other extensions are rejected. Files are UTF-8 and limited to 512 KiB.

## Tools

- `workspace_list` — list editable files and their hashes.
- `workspace_read` — read one file and obtain its current hash.
- `workspace_validate` — validate proposed YAML or Rhai source without writing.
- `workspace_write` — create or replace a validated file.
- `workspace_delete` — remove an active file by moving it to `.respanso-mcp-trash/`.

Read and validation operations are always available. Write and delete operations are disabled by default and require the local Studio setting **«Разрешить MCP-агенту изменять триггеры и Rhai-скрипты»**.

## Safe modification flow

For an existing file the agent should:

1. Call `workspace_read`.
2. Prepare the complete proposed UTF-8 file content.
3. Call `workspace_validate`.
4. Show the planned change to the user / host UI.
5. Call `workspace_write` with the `expected_hash` returned by `workspace_read` and `confirmed: true`.

If the file changed between steps 1 and 5, rEspanso rejects the write and requires the agent to read it again. Repeating a successful write with exactly the same content is treated as unchanged.

For a new file, omit `expected_hash`.

## Deletion

`workspace_delete` requires both `confirmed: true` and the current `expected_hash`. The file is moved out of the active workspace into `.respanso-mcp-trash/<timestamp>/...` instead of being permanently deleted.

## Validation

Match files must parse as YAML with a mapping at the root; if `matches` is present it must be a sequence. Rhai files are syntax-compiled with the Rhai engine before they can be written.

The MCP transport itself does not execute Rhai scripts or trigger expansions as part of validation.

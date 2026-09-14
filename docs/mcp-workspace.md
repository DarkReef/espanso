# MCP workspace editing

rEspanso exposes a sandboxed MCP workspace for registered agents that need to inspect or change text-expansion rules and Rhai scripts.

The stdio server supports both the legacy `2025-06-18` initialize flow and the stateless `2026-07-28` flow with `server/discover` and per-request `_meta`.

## Agent registration

Workspace tools are not exposed to an arbitrary MCP process. Create an agent in **ИИ / MCP → MCP-агенты · регистрация и права**.

Each registration has:

- a generated Agent ID;
- a high-entropy token shown only when created or rotated;
- enabled/disabled state;
- independent permissions for read, write, delete and `rewrite_text`.

The local registry is stored as `mcp-agents.json`. It contains only a SHA-256 token verifier, never the plaintext token.

Pass the credentials to the MCP process as environment variables:

```text
RESPANSO_MCP_AGENT_ID=<agent id>
RESPANSO_MCP_TOKEN=<one-time displayed token>
```

An unknown, disabled or incorrectly authenticated process does not receive the `workspace_*` tools in discovery. A registered agent is still constrained by its per-agent permissions and the global Studio write switch.

Workspace authorization is revalidated on every workspace tool call. Disabling or deleting an agent, changing its permissions, or rotating its token therefore takes effect without trusting a long-lived authorization cached at process startup. After token rotation, an already-running client must be restarted or otherwise relaunched with the new environment token before its next workspace operation can succeed.

## Scope

The authenticated MCP process can only access:

- `match/**/*.yml`
- `match/**/*.yaml`
- `scripts/**/*.rhai`

Absolute paths, `..`, symbolic links and other extensions are rejected. Files are UTF-8 and limited to 512 KiB. `mcp-agents.json`, AI settings, credentials and other rEspanso configuration are outside the agent workspace.

## Tools

- `workspace_list` — list editable files and their hashes.
- `workspace_read` — read one file and obtain its current hash.
- `workspace_validate` — validate proposed YAML or Rhai source without writing.
- `workspace_write` — create or replace a validated file.
- `workspace_delete` — remove an active file by moving it to `.respanso-mcp-trash/`.

Read and validation require the registered agent's read permission. Write additionally requires both the agent's write permission and the local Studio master switch **«Главный выключатель: разрешить зарегистрированным MCP-агентам изменять триггеры и Rhai-скрипты»**. Delete requires its own per-agent permission.

## Safe modification flow

For an existing file the agent should:

1. Call `workspace_read`.
2. Prepare the complete proposed UTF-8 file content.
3. Call `workspace_validate`.
4. Show the planned change to the user / host UI.
5. Call `workspace_write` with the `expected_hash` returned by `workspace_read` and `confirmed: true`.

If the file changed between steps 1 and 5, rEspanso rejects the write and requires the agent to read it again. Repeating a successful write with exactly the same content is treated as unchanged.

For a new file, omit `expected_hash`.

## Runtime and Studio refresh

The rEspanso daemon watches the configuration tree recursively. Valid changes to `.yml`, `.yaml` and `.rhai` trigger the existing stable-change debounce and, when `auto_restart` is enabled, the worker is restarted against the reloaded configuration. Invalid YAML is not silently promoted over the last healthy worker.

Match Studio uses its existing external-file conflict handling but caps the next check to roughly 500 ms in the shell. The file monitor still waits for stable content before reloading. Therefore agent edits normally become visible in Studio in about one second, while unsaved local Studio edits continue to produce the normal external-change conflict instead of being overwritten.

## Deletion

`workspace_delete` requires `confirmed: true`, delete permission and the current `expected_hash`. The file is moved out of the active workspace into `.respanso-mcp-trash/<timestamp>/...` instead of being permanently deleted.

## Validation

Match files must parse as YAML with a mapping at the root; if `matches` is present it must be a sequence. Rhai files are syntax-compiled with the Rhai engine before they can be written.

The MCP transport itself does not execute Rhai scripts or trigger expansions as part of validation.

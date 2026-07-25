# rEspanso: execute a match from selected text

The `dev-in` branch adds a Windows-first workflow for executing an existing match from text selected in any application.

## Usage

1. Select text in the current application.
2. Press `Ctrl+Alt+M`.
3. rEspanso copies the selection, searches user-defined trigger and regexp matches, and executes the matching rule.
4. An `@dialog` result leaves the selected source text unchanged. A normal replacement is inserted into the active application and can replace the current selection.

The clipboard text is restored when `preserve_clipboard: true` is enabled.

## Important distinction from ordinary typing

A trigger remains an ordinary Espanso trigger as well as a selected-text lookup key.

For example, if a rule uses `trigger: "I10"`, typing `I10` while rEspanso is active invokes the ordinary trigger engine immediately. It does not wait for text selection or `Ctrl+Alt+M`.

To test the selected-text path, use text that already exists in a document, web page or MIS. Alternatively, paste the text into an editor, select it, and then press `Ctrl+Alt+M`.

The `selection` variable is available only when the match is invoked through the selected-text hotkey. A match that may also be invoked by ordinary typing should not require `{{selection}}` unless it provides its own fallback value.

## Exact trigger example

```yaml
matches:
  - trigger: "I10"
    replace: |
      @dialog: Диагноз
      Артериальная гипертензия.
```

Select an already existing or pasted `I10`, press `Ctrl+Alt+M`, and the match is rendered as an informational dialog.

When the result explicitly needs the selected source text, it can use `{{selection}}`, but that version must be invoked through the selected-text workflow rather than ordinary typing:

```yaml
matches:
  - trigger: "I10"
    replace: |
      @dialog: Диагноз
      Выделено: {{selection}}
```

## Regexp and API script example

```yaml
matches:
  - regex: ':пац_(?P<surname>[А-ЯЁа-яё-]+)(?P<initials>[А-ЯЁ]{2})(?P<year>\d{4})'
    replace: |
      @dialog: Контекст пациента
      {{patient_context}}

    vars:
      - name: patient_context
        type: script
        params:
          args:
            - python
            - '%CONFIG%/scripts/patient_context.py'
          trim: true
```

Select:

```text
:пац_СидоровИЮ1985
```

The script receives:

```text
ESPANSO_SELECTION=:пац_СидоровИЮ1985
ESPANSO_SURNAME=Сидоров
ESPANSO_INITIALS=ИЮ
ESPANSO_YEAR=1985
```

The script can call KSAPD or another API and print dynamic text to stdout. The rendered `@dialog` directive displays that output without replacing the selected source text.

## Matching rules

- Trigger matches require the whole selected text to equal the trigger.
- Regexp matches are checked against the whole selected text.
- Named regexp groups are passed to the renderer and scripts.
- If several matches apply, the existing match-selection UI is used.
- Built-in matches are not searched.

## Current limitations

- The MVP uses simulated Copy and the clipboard. UI Automation support can be added as a preferred Windows path later.
- Only text clipboard content can currently be restored; other clipboard formats are outside the existing clipboard read contract.
- The shortcut is currently fixed to `Ctrl+Alt+M` in the first MVP.
- Trigger and regexp definitions currently remain active for ordinary typed expansion. A future `selection_only: true` mode should separate selected-text actions from regular typing.

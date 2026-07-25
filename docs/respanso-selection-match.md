# rEspanso: execute a match from selected text

The `dev-in` branch adds a Windows-first workflow for executing an existing match from text selected in any application.

## Usage

1. Select text in the current application.
2. Press `Ctrl+Alt+M`.
3. rEspanso copies the selection, searches user-defined trigger and regexp matches, and executes the matching rule.
4. The selected text remains unchanged.

The clipboard text is restored when `preserve_clipboard: true` is enabled.

## Exact trigger example

```yaml
matches:
  - trigger: "I10"
    replace: |
      @dialog: Диагноз
      Артериальная гипертензия.
      Выделено: {{selection}}
```

Select `I10`, press `Ctrl+Alt+M`, and the match is rendered as an informational dialog.

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

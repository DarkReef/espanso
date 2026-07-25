# rEspanso: recover a missed match from selected text

The `dev-in` branch adds a Windows-first recovery path for ordinary Espanso matches.

The feature is intended for cases where Espanso misses a trigger during typing, especially in slow or unusual input fields such as desktop medical information systems. If the trigger text remains in the field, select it and press `Ctrl+Alt+M`. rEspanso copies the selection, searches the existing user matches, and executes the same rule.

No separate `selection_only` match and no special trigger syntax are required.

## Usage

1. Type an ordinary Espanso trigger.
2. If Espanso expands it normally, nothing else is needed.
3. If the trigger remains as plain text, select the complete trigger.
4. Press `Ctrl+Alt+M`.
5. rEspanso searches exact trigger matches and full regexp matches, then executes the matching rule.

An `@dialog` result leaves the selected source text unchanged. A normal replacement is inserted into the active application and can replace the current selection.

The clipboard text is restored when `preserve_clipboard: true` is enabled.

## Ordinary trigger with recovery

```yaml
matches:
  - trigger: ":i10_9"
    replace: "Артериальная гипертензия с преимущественным поражением сердца без сердечной недостаточности"
```

Normal path:

```text
Type :i10_9
→ Espanso expands it automatically
```

Recovery path:

```text
:i10_9 remains in the MIS field
→ select :i10_9
→ press Ctrl+Alt+M
→ rEspanso executes the same match
```

## Informational dialog

```yaml
matches:
  - trigger: ":i10_9"
    replace: |
      @dialog: Диагноз
      Артериальная гипертензия.
```

The rule still works as an ordinary trigger. The selected-text hotkey only retries it when normal trigger detection failed.

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

If the text remains after a missed expansion, select:

```text
:пац_СидоровИЮ1985
```

and press `Ctrl+Alt+M`.

The script receives:

```text
ESPANSO_SELECTION=:пац_СидоровИЮ1985
ESPANSO_SURNAME=Сидоров
ESPANSO_INITIALS=ИЮ
ESPANSO_YEAR=1985
```

The script can call KSAPD or another API and print dynamic text to stdout. The rendered `@dialog` directive displays that output without replacing the selected source text.

## Matching rules

- Trigger matches require the whole selected text to equal an existing trigger.
- Regexp matches are checked against the whole selected text.
- Named regexp groups are passed to the renderer and scripts.
- The original ordinary match is executed; no duplicate recovery rule is created.
- If several matches apply, the existing match-selection UI is used.
- Built-in matches are not searched.

## Current limitations

- The MVP uses simulated Copy and the clipboard. UI Automation support can be added as a preferred Windows path later.
- Only text clipboard content can currently be restored; other clipboard formats are outside the existing clipboard read contract.
- The shortcut is currently fixed to `Ctrl+Alt+M` in the first MVP.
- The selected text must contain the complete trigger or satisfy the complete regexp.

# rEspanso dynamic dialogs

Dynamic dialogs combine a regular-expression trigger, named capture groups, an Espanso script variable, and the `@dialog` output directive.

## Patient lookup example

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
          debug: true
```

The named regex groups are exposed to the script as environment variables:

- `ESPANSO_SURNAME`
- `ESPANSO_INITIALS`
- `ESPANSO_YEAR`

Example script:

```python
import json
import os
import sys
import urllib.error
import urllib.request

API_URL = "http://127.0.0.1:8000/ksapd/api/integrations/patient-context"


def main() -> int:
    payload = {
        "surname": os.environ.get("ESPANSO_SURNAME", ""),
        "initials": os.environ.get("ESPANSO_INITIALS", ""),
        "birth_year": os.environ.get("ESPANSO_YEAR", ""),
    }

    request = urllib.request.Request(
        API_URL,
        data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        headers={"Content-Type": "application/json; charset=utf-8"},
        method="POST",
    )

    try:
        with urllib.request.urlopen(request, timeout=5) as response:
            result = json.loads(response.read().decode("utf-8"))
    except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as error:
        print(f"Не удалось получить данные пациента.\n\n{error}")
        return 0

    print(result.get("text", "Данные пациента не найдены."))
    return 0


if __name__ == "__main__":
    sys.exit(main())
```

Typing `:пац_СидоровИЮ1985` runs the script after the regex groups have been captured. The fully rendered result starts with `@dialog`, so rEspanso opens the text viewer instead of inserting the output into the active application.

The API should return a short, human-readable `text` field for this first implementation. A structured card schema can be added later without changing the regex or script execution flow.

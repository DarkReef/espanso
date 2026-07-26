from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

README = r'''# rEspanso

> **rEspanso — форк проекта Espanso, созданный и развиваемый Куциным Иваном Юрьевичем.**
>
> Проект основан на оригинальном Espanso, созданном Federico Terzi и развиваемом сообществом. Исходные авторские права и лицензия GPL-3.0 сохраняются.

**rEspanso** — локальный кроссплатформенный текстовый экспандер и инструмент автоматизации на Rust, ориентированный на надёжную работу с длинными медицинскими шаблонами, визуальными формами и внешними модулями.

## Основные возможности rEspanso

- обычные trigger- и RegExp-матчи Espanso;
- восстановление пропущенного матча: выделить оставшийся триггер и нажать `Ctrl+Alt+M`;
- информационные окна после динамического рендеринга через `@dialog`;
- визуальные формы для калькуляторов, тестов и опросников;
- встроенный Rhai-runtime: редактируемые `.rhai`-модули без Rust, Node.js, Python и PowerShell;
- portable-запуск Windows двойным кликом по `rEspanso.exe`;
- изолированные каталоги `portable/config`, `portable/runtime` и `portable/packages`;
- локальная работа без обязательного облака и телеметрии.

## Быстрый старт Windows portable

1. Распакуйте архив полностью.
2. Запустите `rEspanso.exe` двойным кликом.
3. Для проверки информационного окна введите `:rinfo`.
4. Для проверки восстановления пропущенного матча вставьте `:i10_9`, выделите его и нажмите `Ctrl+Alt+M`.
5. Для проверки визуальной формы и Rhai-модуля введите `:calc_demo`.

Остановка выполняется через меню в системном трее. Резервная CLI-команда:

```powershell
.\rEspanso.exe service stop
```

## Медицинские формы и калькуляторы

Визуальная часть описывается YAML, а расчётная логика хранится во внешних `.rhai`-файлах:

```text
YAML-форма → встроенный Rhai → результат → окно rEspanso
```

Это позволяет добавлять и исправлять FINDRISC, HAS-BLED, SCORE2/SCORE2-OP, CHA₂DS₂-VASc и другие модули без пересборки приложения.

## Совместимость с исходным проектом

Внутренние имена Rust-crate, переменные `ESPANSO_*`, форматы конфигурации, технические пути и часть CLI-идентификаторов намеренно сохраняют имя `espanso`. Это необходимо для совместимости с исходной экосистемой и **не является пользовательским брендом приложения**.

Оригинальный проект и документация:

- [Espanso](https://github.com/espanso/espanso)
- [Официальная документация Espanso](https://espanso.org/docs/)
- [Espanso Hub](https://hub.espanso.org/)

## Авторство

**Форк rEspanso:** Куцин Иван Юрьевич.

**Оригинальный проект Espanso:** Federico Terzi и участники проекта Espanso.

## Лицензия

rEspanso распространяется по лицензии [GPL-3.0](LICENSE), как и исходный проект. Сведения об исходных авторах и авторские уведомления в коде сохранены.
'''

TECHNICAL_TOKENS = {
    "espanso.exe": "__TECH_ESPANSO_EXE__",
    "espansod.exe": "__TECH_ESPANSOD_EXE__",
    "espanso.cmd": "__TECH_ESPANSO_CMD__",
    "espanso.org": "__TECH_ESPANSO_ORG__",
    "espanso/espanso": "__TECH_ESPANSO_REPO__",
    "espanso-": "__TECH_ESPANSO_CRATE__",
    "ESPANSO_": "__TECH_ESPANSO_ENV__",
}

PHRASE_REPLACEMENTS = {
    "espanso daemon": "rEspanso daemon",
    "espanso service": "rEspanso service",
    "espanso directory": "rEspanso directory",
    "espanso package directory": "rEspanso package directory",
    "espanso runtime directory": "rEspanso runtime directory",
    "espanso should": "rEspanso should",
    "Start espanso": "Start rEspanso",
    "start espanso": "start rEspanso",
    "Restart espanso": "Restart rEspanso",
    "restart espanso": "restart rEspanso",
    "Stop espanso": "Stop rEspanso",
    "stop espanso": "stop rEspanso",
    "Check if espanso": "Check if rEspanso",
    "Register espanso": "Register rEspanso",
    "Unregister espanso": "Unregister rEspanso",
    "Send a command to the espanso": "Send a command to the rEspanso",
}


def brand_string(value: str) -> str:
    protected = value
    for token, placeholder in TECHNICAL_TOKENS.items():
        protected = protected.replace(token, placeholder)
    protected = protected.replace("Espanso", "rEspanso")
    for old, new in PHRASE_REPLACEMENTS.items():
        protected = protected.replace(old, new)
    for token, placeholder in TECHNICAL_TOKENS.items():
        protected = protected.replace(placeholder, token)
    return protected


def raw_string_start(source: str, index: int) -> tuple[int, int] | None:
    prefix_len = 0
    if source.startswith("br", index):
        prefix_len = 2
    elif source.startswith("r", index):
        prefix_len = 1
    else:
        return None
    cursor = index + prefix_len
    hashes = 0
    while cursor < len(source) and source[cursor] == "#":
        hashes += 1
        cursor += 1
    if cursor < len(source) and source[cursor] == '"':
        return cursor + 1, hashes
    return None


def transform_rust_strings(source: str) -> str:
    out: list[str] = []
    index = 0
    size = len(source)
    while index < size:
        if source.startswith("//", index):
            end = source.find("\n", index)
            if end == -1:
                out.append(source[index:])
                break
            out.append(source[index:end + 1])
            index = end + 1
            continue
        if source.startswith("/*", index):
            end = source.find("*/", index + 2)
            if end == -1:
                out.append(source[index:])
                break
            out.append(source[index:end + 2])
            index = end + 2
            continue

        raw = raw_string_start(source, index)
        if raw is not None:
            content_start, hashes = raw
            closing = '"' + ('#' * hashes)
            end = source.find(closing, content_start)
            if end == -1:
                out.append(source[index:])
                break
            out.append(source[index:content_start])
            out.append(brand_string(source[content_start:end]))
            out.append(closing)
            index = end + len(closing)
            continue

        prefix_len = 0
        if source.startswith('b"', index):
            prefix_len = 1
        if source[index + prefix_len:index + prefix_len + 1] == '"':
            quote = index + prefix_len
            cursor = quote + 1
            escaped = False
            while cursor < size:
                char = source[cursor]
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == '"':
                    break
                cursor += 1
            if cursor >= size:
                out.append(source[index:])
                break
            out.append(source[index:quote + 1])
            out.append(brand_string(source[quote + 1:cursor]))
            out.append('"')
            index = cursor + 1
            continue

        if source[index] == "'":
            cursor = index + 1
            escaped = False
            while cursor < size:
                char = source[cursor]
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == "'":
                    cursor += 1
                    break
                cursor += 1
            out.append(source[index:cursor])
            index = cursor
            continue

        out.append(source[index])
        index += 1
    return "".join(out)


def update_package_metadata() -> None:
    path = ROOT / "espanso/Cargo.toml"
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        'authors = ["Federico Terzi <federicoterzi96@gmail.com>"]',
        'authors = ["Куцин Иван Юрьевич", "Federico Terzi <federicoterzi96@gmail.com>"]',
    )
    text = text.replace(
        'description = "Cross-platform Text Expander written in Rust"',
        'description = "rEspanso — privacy-first cross-platform text expander and automation fork"',
    )
    text = text.replace(
        'homepage = "https://espanso.org/"',
        'homepage = "https://github.com/DarkReef/espanso"',
    )
    text = text.replace(
        'repository = "https://github.com/espanso/espanso"',
        'repository = "https://github.com/DarkReef/espanso"',
    )
    text = text.replace(
        'maintainer = "Auca Coyan <aucacoyan@gmail.com>"',
        'maintainer = "Куцин Иван Юрьевич"',
    )
    path.write_text(text, encoding="utf-8")


def update_rust_sources() -> None:
    for path in ROOT.rglob("*.rs"):
        source = path.read_text(encoding="utf-8")
        changed = transform_rust_strings(source)
        if path.relative_to(ROOT).as_posix() == "espanso/src/main.rs":
            changed = changed.replace('App::new("espanso")', 'App::new("rEspanso")')
            changed = changed.replace(
                '.author("Federico Terzi and the espanso contributors")',
                '.author("Куцин Иван Юрьевич; based on Espanso by Federico Terzi and contributors")',
            )
            changed = changed.replace(
                '.about("A Privacy-first, Cross-platform Text Expander")',
                '.about("rEspanso — a privacy-first cross-platform text expander and automation fork")',
            )
        if changed != source:
            path.write_text(changed, encoding="utf-8")


def update_public_texts() -> None:
    paths: set[Path] = set()
    paths.update((ROOT / "docs").glob("respanso-*.md"))
    paths.update((ROOT / "examples/medical-calculators").rglob("*.yml"))
    paths.update((ROOT / "examples/medical-calculators").rglob("*.yaml"))
    paths.update((ROOT / "examples/medical-calculators").rglob("*.rhai"))
    paths.update((ROOT / ".github/workflows").glob("*respanso*.yml"))
    paths.update((ROOT / ".github/workflows").glob("rhai-diagnostics.yml"))
    paths.update(
        ROOT / relative
        for relative in (
            "scripts/build_windows_installer.ps1",
            "scripts/build_windows_portable.ps1",
            "scripts/create_app_image.sh",
            "scripts/create_bundle.sh",
            "scripts/resources/windows/setupscript.iss",
        )
    )
    for path in paths:
        if not path.exists() or path.name.startswith("apply-respanso-branding"):
            continue
        source = path.read_text(encoding="utf-8")
        changed = source.replace("Espanso", "rEspanso")
        changed = changed.replace("rEspanso.org", "espanso.org")
        changed = changed.replace(
            "github.com/rEspanso/rEspanso", "github.com/espanso/espanso"
        )
        changed = changed.replace("hub.rEspanso.org", "hub.espanso.org")
        if changed != source:
            path.write_text(changed, encoding="utf-8")


def update_workflow_branches() -> None:
    for path in (ROOT / ".github/workflows").glob("*.yml"):
        if path.name.startswith("apply-respanso-branding"):
            continue
        source = path.read_text(encoding="utf-8")
        changed = source.replace("      - dev-in\n", "      - rEspanso-feature\n")
        if changed != source:
            path.write_text(changed, encoding="utf-8")


def update_nix_description() -> None:
    path = ROOT / "nix/espanso.nix"
    if not path.exists():
        return
    source = path.read_text(encoding="utf-8")
    path.write_text(source.replace("Espanso", "rEspanso"), encoding="utf-8")


def main() -> None:
    (ROOT / "README.md").write_text(README, encoding="utf-8")
    update_package_metadata()
    update_rust_sources()
    update_public_texts()
    update_workflow_branches()
    update_nix_description()


if __name__ == "__main__":
    main()

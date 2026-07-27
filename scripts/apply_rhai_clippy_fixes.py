from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_exact(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


def main() -> None:
    app = ROOT / "espanso-editor/src/app.rs"
    rhai = ROOT / "espanso-editor/src/rhai_lab.rs"
    imports = ROOT / "espanso-editor/src/yaml_imports.rs"

    replace_exact(
        app,
        '        self.status =\n            "Обнаружены изменения YAML-файлов; Studio обновила правила и настройки".to_owned();',
        '        "Обнаружены изменения YAML-файлов; Studio обновила правила и настройки"\n            .clone_into(&mut self.status);',
        "app reload success status",
    )
    replace_exact(
        app,
        '            self.status = "Файлы конфигурации изменились снаружи. Сохраните или отмените локальные изменения, затем обновите с диска".to_owned();',
        '            "Файлы конфигурации изменились снаружи. Сохраните или отмените локальные изменения, затем обновите с диска"\n                .clone_into(&mut self.status);',
        "app external change status",
    )

    replacements = [
        (
            '        self.file_status = "Новый Rhai-скрипт: задайте имя и сохраните файл".to_owned();',
            '        "Новый Rhai-скрипт: задайте имя и сохраните файл"\n            .clone_into(&mut self.file_status);',
            "new Rhai status",
        ),
        (
            '            self.file_status =\n                "Текущий Rhai-файл изменился снаружи; локальный текст оставлен без изменений"\n                    .to_owned();',
            '            "Текущий Rhai-файл изменился снаружи; локальный текст оставлен без изменений"\n                .clone_into(&mut self.file_status);',
            "dirty external Rhai status",
        ),
        (
            '                self.file_status = "Rhai-файл автоматически обновлён с диска".to_owned();',
            '                "Rhai-файл автоматически обновлён с диска"\n                    .clone_into(&mut self.file_status);',
            "automatic Rhai reload status",
        ),
        (
            '            self.file_status = "Открытый Rhai-файл был удалён с диска".to_owned();',
            '            "Открытый Rhai-файл был удалён с диска"\n                .clone_into(&mut self.file_status);',
            "removed Rhai file status",
        ),
    ]
    for old, new, label in replacements:
        replace_exact(rhai, old, new, label)

    replace_exact(
        imports,
        '            let insertion = top_level_section(content, "matches")\n                .map(|(start, _)| start)\n                .unwrap_or(content.len());',
        '            let insertion = top_level_section(content, "matches")\n                .map_or(content.len(), |(start, _)| start);',
        "yaml imports map_or",
    )


if __name__ == "__main__":
    main()

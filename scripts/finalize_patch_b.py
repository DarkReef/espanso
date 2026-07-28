from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content, encoding="utf-8", newline="\n")


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    if old not in text:
        raise RuntimeError(f"Pattern was not found in {path}: {old[:120]!r}")
    write(path, text.replace(old, new, 1))


def patch_lib_and_manifests() -> None:
    replace_once(
        "espanso-editor/src/lib.rs",
        "pub mod config_transfer;\npub mod file_monitor;",
        "pub mod config_transfer;\npub mod diagnostics;\npub mod file_monitor;",
    )
    replace_once(
        "espanso-editor/src/lib.rs",
        "pub mod settings;\npub mod workspace;",
        "pub mod settings;\npub mod storm_logo;\npub mod workspace;",
    )
    replace_once(
        "espanso-editor/Cargo.toml",
        '''[dev-dependencies]
tempdir.workspace = true

[lints]''',
        '''[dev-dependencies]
tempdir.workspace = true

[build-dependencies]
winres = "0.1.12"

[lints]''',
    )
    replace_once(
        "espanso/Cargo.toml",
        '''[target.'cfg(target_os="linux")'.dependencies]
caps = "0.5.2"
const_format = "0.2.14"

[package.metadata.deb]''',
        '''[target.'cfg(target_os="linux")'.dependencies]
caps = "0.5.2"
const_format = "0.2.14"

[build-dependencies]
winres = "0.1.12"

[package.metadata.deb]''',
    )


def patch_packaging_and_readme() -> None:
    path = "scripts/build_windows_native_portable_with_studio.ps1"
    replace_once(
        path,
        '''$scriptsDir = Join-Path $packageDir "scripts"
$forbiddenNestedConfig''',
        '''$scriptsDir = Join-Path $packageDir "scripts"
$docsDir = Join-Path $packageDir "docs"
$forbiddenNestedConfig''',
    )
    replace_once(
        path,
        '''New-Item -Path $configDir, $matchDir, $runtimeDir, $packagesDir, $scriptsDir -ItemType Directory -Force | Out-Null''',
        '''New-Item -Path $configDir, $matchDir, $runtimeDir, $packagesDir, $scriptsDir, $docsDir -ItemType Directory -Force | Out-Null''',
    )
    replace_once(
        path,
        '''if (Test-Path "LICENSE") {
    Copy-Item "LICENSE" (Join-Path $packageDir "LICENSE.txt")
}

Copy-Item "espanso/src/res/config/default.yml"''',
        '''if (Test-Path "LICENSE") {
    Copy-Item "LICENSE" (Join-Path $packageDir "LICENSE.txt")
}
Copy-Item "docs/respanso/*" $docsDir -Recurse -Force

Copy-Item "espanso/src/res/config/default.yml"''',
    )
    replace_once(
        path,
        '''  scripts\example.rhai

Все каталоги''',
        '''  scripts\example.rhai
  docs\README.ru.md

Все каталоги''',
    )
    replace_once(
        path,
        '''    $packagesDir,
    (Join-Path $scriptsDir "example.rhai")
)''',
        '''    $packagesDir,
    (Join-Path $scriptsDir "example.rhai"),
    (Join-Path $docsDir "README.ru.md"),
    (Join-Path $docsDir "RHAI_PROMPT.ru.md")
)''',
    )

    workflow = ".github/workflows/build-respanso-windows.yml"
    replace_once(
        workflow,
        '''            "$package/packages",
            "$package/scripts/example.rhai"
          )''',
        '''            "$package/packages",
            "$package/scripts/example.rhai",
            "$package/docs/README.ru.md",
            "$package/docs/RHAI_PROMPT.ru.md"
          )''',
    )

    readme = read("README.md")
    readme = readme.replace(
        "- изолированные каталоги `portable/config`, `portable/runtime` и `portable/packages`;",
        "- единая portable-папка с локальными `config/`, `match/`, `scripts/`, `runtime/` и `packages/`;",
    )
    if "## Документация rEspanso" not in readme:
        marker = "## Совместимость с исходным проектом"
        section = '''## Документация rEspanso

Полное русское руководство находится в [`docs/respanso/`](docs/respanso/README.ru.md). В portable-архив оно включается в папку `docs/`.

'''
        if marker not in readme:
            raise RuntimeError("README compatibility marker not found")
        readme = readme.replace(marker, section + marker, 1)
    write("README.md", readme)


if __name__ == "__main__":
    patch_lib_and_manifests()
    patch_packaging_and_readme()

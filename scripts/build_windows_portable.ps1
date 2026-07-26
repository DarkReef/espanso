#!/usr/bin/env pwsh

$ErrorActionPreference = "Stop"
$TARGET_DIR = "target/windows/portable"
$RESOURCE_DIR = "target/windows/resources"
$PACKAGE_DIR = "target/windows/respanso-portable-with-studio"
$ARCHIVE_PATH = "target/windows/rEspanso-Win-Portable-with-Studio-x86_64.zip"

function Main {
    if (-not (Test-Path $RESOURCE_DIR)) {
        Write-Error "Build Windows resources first: scripts/build_windows_resources.ps1"
    }
    foreach ($path in @($TARGET_DIR, $PACKAGE_DIR)) {
        if (Test-Path $path) {
            Remove-Item $path -Recurse -Force
        }
    }

    Copy-Item -Path $RESOURCE_DIR -Destination $TARGET_DIR -Recurse -Force
    'start "" espansod.exe launcher' | Out-File "$TARGET_DIR/START_ESPANSO.bat" -Encoding ASCII

    New-Item -Path "$TARGET_DIR/.espanso/match" -ItemType Directory -Force | Out-Null
    New-Item -Path "$TARGET_DIR/.espanso-runtime" -ItemType Directory -Force | Out-Null

    $baseMatch = "$TARGET_DIR/.espanso/match/base.yml"
    if (-not (Test-Path $baseMatch)) {
        @(
            'matches:',
            '  - label: "rEspanso Portable готов"',
            '    trigger: ":respanso_example"',
            '    replace: "rEspanso и Match Studio работают из единого portable-комплекта"',
            '    disabled: true'
        ) | Set-Content -Path $baseMatch -Encoding UTF8
    }

    @"
rEspanso Portable + Match Studio

1. Запустите START_ESPANSO.bat.
2. Откройте меню значка rEspanso в трее и выберите «Открыть rEspanso Studio».
3. Studio также можно открыть напрямую файлом «rEspanso Match Studio.exe».

rEspanso и Studio используют одну конфигурацию:
  .espanso

Правила находятся в:
  .espanso\match

Рабочие файлы процесса находятся в:
  .espanso-runtime

Командная строка:
  espanso.cmd --help
  espanso.cmd edit --gui

Не разделяйте файлы комплекта: daemon и Studio рассчитаны на совместное portable-размещение.
"@ | Out-File "$TARGET_DIR/README.txt" -Encoding UTF8

    Move-Item -Path $TARGET_DIR -Destination $PACKAGE_DIR
    Compress-Archive -Path $PACKAGE_DIR -DestinationPath $ARCHIVE_PATH -Force
    Write-Output "Unified rEspanso Portable + Match Studio created: $ARCHIVE_PATH"
}

Main @PSBoundParameters

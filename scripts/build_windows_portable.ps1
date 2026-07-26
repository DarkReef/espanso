#!/usr/bin/env pwsh

$ErrorActionPreference = "Stop"

$TARGET_DIR = "target/windows/portable"
$RESOURCE_DIR = "target/windows/resources"

if (-not (Test-Path $RESOURCE_DIR)) {
    Write-Error "Сначала соберите ресурсы Windows: scripts/build_windows_resources.ps1"
    exit 1
}

function Main {
    if (Test-Path $TARGET_DIR) {
        Remove-Item $TARGET_DIR -Recurse -Force
    }

    if (Test-Path "target/windows/espanso-portable") {
        Remove-Item "target/windows/espanso-portable" -Recurse -Force
    }

    Copy-Item -Path $RESOURCE_DIR -Destination $TARGET_DIR -Recurse -Force

    $launcherContent = 'start espansod.exe launcher'
    $launcherContent | Out-File "$TARGET_DIR/START_ESPANSO.bat" -Encoding ASCII

    $portableConfig = "$TARGET_DIR/portable/config"
    New-Item -Path "$portableConfig/match" -ItemType Directory -Force | Out-Null
    New-Item -Path "$TARGET_DIR/.espanso-runtime" -ItemType Directory -Force | Out-Null

    $baseMatchFile = "$portableConfig/match/base.yml"
    if (-not (Test-Path $baseMatchFile)) {
        @(
            'matches:',
            '  - label: "Portable-конфигурация готова"',
            '    trigger: ":respanso_example"',
            '    replace: "rEspanso Match Studio готов к работе"',
            '    disabled: true'
        ) | Set-Content -Path $baseMatchFile -Encoding UTF8
    }

    $readmeContent = @"
rEspanso Portable

Для запуска rEspanso дважды щёлкните START_ESPANSO.bat.
Для открытия редактора правил дважды щёлкните espanso-editor.exe.

Редактор запускается напрямую: отдельный CMD-файл ему не требуется.
Он автоматически использует конфигурацию:
  portable\config

YAML-файлы правил находятся в:
  portable\config\match

Начальный файл portable\config\match\base.yml уже включён, поэтому новое правило
можно создать сразу после первого запуска редактора.

Не удаляйте файлы и папки из комплекта: это может нарушить работу rEspanso.

Для работы через терминал используйте espanso.cmd. Он необходим только как
совместимая CLI-обёртка и не участвует в запуске Match Studio.
"@
    $readmeContent | Out-File "$TARGET_DIR/README.txt" -Encoding UTF8

    Rename-Item -Path $TARGET_DIR -NewName espanso-portable
    Compress-Archive target/windows/espanso-portable target/windows/Espanso-Win-Portable-x86_64.zip -Force

    Write-Output "Espanso Portable создан"
}

Main @PSBoundParameters

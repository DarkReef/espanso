#!/usr/bin/env pwsh

$ErrorActionPreference = "Stop"

$PACKAGE_DIR = "target/windows/respanso-portable-with-studio"
$ARCHIVE_PATH = "target/windows/rEspanso-Win-Portable-with-Studio-x86_64.zip"

function Resolve-RequiredBinary {
    param(
        [Parameter(Mandatory = $true)][string]$EnvironmentName,
        [Parameter(Mandatory = $true)][string]$FallbackPath
    )

    $configured = [Environment]::GetEnvironmentVariable($EnvironmentName)
    $candidate = if ([string]::IsNullOrWhiteSpace($configured)) { $FallbackPath } else { $configured }
    if (-not (Test-Path -Path $candidate -PathType Leaf)) {
        throw "Required binary was not found ($EnvironmentName): $candidate"
    }
    return (Resolve-Path $candidate).Path
}

function Resolve-VcRuntimeDirectory {
    if (-not [string]::IsNullOrWhiteSpace($env:VCToolsRedistDir)) {
        $crtRoot = Join-Path $env:VCToolsRedistDir "x64"
        if (Test-Path $crtRoot) {
            $crtDir = Get-ChildItem -Path $crtRoot -Directory |
                Where-Object { $_.Name -like "Microsoft.VC*.CRT" } |
                Sort-Object Name -Descending |
                Select-Object -First 1
            if ($crtDir) {
                return $crtDir.FullName
            }
        }
    }

    $runtimeDll = Get-ChildItem -Path "C:\Program Files\Microsoft Visual Studio" -Recurse -Filter "vcruntime140_1.dll" -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -like "*\VC\Redist\MSVC\*" -and $_.FullName -like "*\x64\*" } |
        Select-Object -First 1
    if ($runtimeDll) {
        return $runtimeDll.Directory.FullName
    }

    throw "Unable to locate the Microsoft Visual C++ x64 redistributable runtime"
}

function Main {
    $corePath = Resolve-RequiredBinary -EnvironmentName "EXEC_PATH" -FallbackPath "target/release/espanso.exe"
    $launcherPath = Resolve-RequiredBinary -EnvironmentName "LAUNCHER_PATH" -FallbackPath "target/release/respanso-portable.exe"
    $editorPath = Resolve-RequiredBinary -EnvironmentName "EDITOR_PATH" -FallbackPath "target/release/espanso-editor.exe"

    foreach ($path in @($PACKAGE_DIR, $ARCHIVE_PATH)) {
        if (Test-Path $path) {
            Remove-Item $path -Recurse -Force
        }
    }

    $matchDir = Join-Path $PACKAGE_DIR "portable/config/match"
    $calculatorDir = Join-Path $PACKAGE_DIR "portable/config/medical-calculators"
    $runtimeDir = Join-Path $PACKAGE_DIR "portable/runtime"
    $packagesDir = Join-Path $PACKAGE_DIR "portable/packages"
    New-Item -Path $PACKAGE_DIR, $matchDir, $calculatorDir, $runtimeDir, $packagesDir -ItemType Directory -Force | Out-Null

    Copy-Item $launcherPath (Join-Path $PACKAGE_DIR "rEspanso.exe")
    Copy-Item $corePath (Join-Path $PACKAGE_DIR "rEspanso-core.exe")
    Copy-Item $editorPath (Join-Path $PACKAGE_DIR "rEspanso Match Studio.exe")

    $vcRuntimeDir = Resolve-VcRuntimeDirectory
    Get-ChildItem -Path $vcRuntimeDir -Filter "*.dll" | Copy-Item -Destination $PACKAGE_DIR

    if (Test-Path "LICENSE") {
        Copy-Item "LICENSE" (Join-Path $PACKAGE_DIR "LICENSE.txt")
    }
    foreach ($doc in @(
        @{ Source = "docs/respanso-dynamic-dialog.md"; Destination = "DYNAMIC-DIALOG.md" },
        @{ Source = "docs/respanso-selection-match.md"; Destination = "SELECTION-MATCH.md" },
        @{ Source = "docs/respanso-medical-calculators.md"; Destination = "MEDICAL-CALCULATORS.md" }
    )) {
        if (Test-Path $doc.Source) {
            Copy-Item $doc.Source (Join-Path $PACKAGE_DIR $doc.Destination)
        }
    }

    $baseMatch = Join-Path $matchDir "respanso-test.yml"
    @'
matches:
  - label: "rEspanso Portable готов"
    trigger: ":respanso_example"
    replace: "rEspanso и Match Studio работают из единого portable-комплекта"
    disabled: true
'@ | Set-Content -Path $baseMatch -Encoding UTF8

    if (Test-Path "examples/medical-calculators/medical-calculators.yml") {
        Copy-Item "examples/medical-calculators/medical-calculators.yml" (Join-Path $matchDir "medical-calculators.yml")
    }
    if (Test-Path "examples/medical-calculators/modules") {
        Copy-Item "examples/medical-calculators/modules" (Join-Path $calculatorDir "modules") -Recurse
    }

    @"
rEspanso Portable + Match Studio

ЗАПУСК
1. Полностью распакуйте ZIP-архив.
2. Запустите rEspanso.exe — это нативный portable-launcher без BAT/CMD.
3. Откройте меню значка rEspanso в трее и выберите «Открыть rEspanso Studio».
4. Studio также можно открыть напрямую: «rEspanso Match Studio.exe».

Все компоненты используют одни каталоги:
  portable\config
  portable\runtime
  portable\packages

Правила находятся в:
  portable\config\match

Командная строка через тот же нативный launcher:
  rEspanso.exe --help
  rEspanso.exe edit --gui
  rEspanso.exe service stop

rEspanso-core.exe — внутренний движок. Он должен оставаться рядом с rEspanso.exe.
Файлы .bat и .cmd для запуска portable-комплекта не используются и в архив не входят.
"@ | Set-Content (Join-Path $PACKAGE_DIR "README-FIRST.txt") -Encoding UTF8

    $forbiddenLaunchers = Get-ChildItem -Path $PACKAGE_DIR -Recurse -File |
        Where-Object { $_.Extension -in @(".bat", ".cmd") }
    if ($forbiddenLaunchers) {
        $names = ($forbiddenLaunchers | ForEach-Object FullName) -join ", "
        throw "Portable package unexpectedly contains BAT/CMD launchers: $names"
    }

    $verification = Start-Process `
        -FilePath (Join-Path $PACKAGE_DIR "rEspanso.exe") `
        -ArgumentList "--version" `
        -WorkingDirectory $PACKAGE_DIR `
        -Wait `
        -PassThru
    if ($verification.ExitCode -ne 0) {
        throw "Native portable launcher verification failed with exit code $($verification.ExitCode)"
    }

    Compress-Archive -Path $PACKAGE_DIR -DestinationPath $ARCHIVE_PATH -Force
    Write-Output "Unified native rEspanso Portable + Match Studio created: $ARCHIVE_PATH"
}

Main @PSBoundParameters

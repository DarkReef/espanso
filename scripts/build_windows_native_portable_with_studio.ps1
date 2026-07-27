#!/usr/bin/env pwsh

$ErrorActionPreference = "Stop"

$packageDir = "target/windows/respanso-native-portable-with-studio"
$archivePath = "target/windows/rEspanso-Native-Portable-with-Studio-Windows-x86_64.zip"

function Resolve-Binary([string]$environmentName, [string]$fallbackPath) {
    $configured = [Environment]::GetEnvironmentVariable($environmentName)
    $candidate = if ([string]::IsNullOrWhiteSpace($configured)) { $fallbackPath } else { $configured }
    if (-not (Test-Path -Path $candidate -PathType Leaf)) {
        throw "Required binary is missing ($environmentName): $candidate"
    }
    return (Resolve-Path $candidate).Path
}

function Resolve-VcRuntimeDirectory {
    if (-not [string]::IsNullOrWhiteSpace($env:VCToolsRedistDir)) {
        $root = Join-Path $env:VCToolsRedistDir "x64"
        $directory = Get-ChildItem -Path $root -Directory -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -like "Microsoft.VC*.CRT" } |
            Sort-Object Name -Descending |
            Select-Object -First 1
        if ($directory) {
            return $directory.FullName
        }
    }

    $runtime = Get-ChildItem -Path "C:\Program Files\Microsoft Visual Studio" -Recurse -Filter "vcruntime140_1.dll" -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -like "*\VC\Redist\MSVC\*" -and $_.FullName -like "*\x64\*" } |
        Select-Object -First 1
    if (-not $runtime) {
        throw "Microsoft Visual C++ x64 runtime was not found"
    }
    return $runtime.Directory.FullName
}

$core = Resolve-Binary "EXEC_PATH" "target/release/espanso.exe"
$launcher = Resolve-Binary "LAUNCHER_PATH" "target/release/respanso-portable.exe"
$studio = Resolve-Binary "EDITOR_PATH" "target/release/espanso-editor.exe"

foreach ($path in @($packageDir, $archivePath)) {
    if (Test-Path $path) {
        Remove-Item $path -Recurse -Force
    }
}

$portableDir = Join-Path $packageDir "portable"
$configDir = Join-Path $portableDir "config"
$matchDir = Join-Path $configDir "match"
$runtimeDir = Join-Path $portableDir "runtime"
$packagesDir = Join-Path $portableDir "packages"
$forbiddenNestedConfig = Join-Path $configDir "config"

New-Item -Path $matchDir, $runtimeDir, $packagesDir -ItemType Directory -Force | Out-Null

Copy-Item $launcher (Join-Path $packageDir "rEspanso.exe")
Copy-Item $core (Join-Path $packageDir "rEspanso-core.exe")
Copy-Item $studio (Join-Path $packageDir "rEspanso Match Studio.exe")

$vcRuntime = Resolve-VcRuntimeDirectory
Get-ChildItem -Path $vcRuntime -Filter "*.dll" | Copy-Item -Destination $packageDir

if (Test-Path "LICENSE") {
    Copy-Item "LICENSE" (Join-Path $packageDir "LICENSE.txt")
}

@'
matches:
  - label: "rEspanso Portable готов"
    trigger: ":respanso_example"
    replace: "rEspanso и Match Studio работают из единого portable-комплекта"
    disabled: true
'@ | Set-Content -Path (Join-Path $matchDir "respanso-test.yml") -Encoding UTF8

@'
rEspanso Portable + Match Studio

Автор форка: Куцин Иван Юрьевич
Почта: imaganate.dark@gmail.com

Запуск: rEspanso.exe
Студия: меню rEspanso в трее -> «Открыть студию rEspanso»
Также студию можно открыть напрямую: rEspanso Match Studio.exe

Общие данные:
  portable\config
  portable\runtime
  portable\packages

Рабочая папка правил должна находиться только здесь:
  portable\config\match

Папка portable\config\config\match является ошибочной и в сборке запрещена.

rEspanso-core.exe является внутренним движком.
BAT и CMD файлы для запуска не используются и в архив не входят.
'@ | Set-Content -Path (Join-Path $packageDir "README-FIRST.txt") -Encoding UTF8

$required = @(
    (Join-Path $packageDir "rEspanso.exe"),
    (Join-Path $packageDir "rEspanso-core.exe"),
    (Join-Path $packageDir "rEspanso Match Studio.exe"),
    $matchDir,
    $runtimeDir,
    $packagesDir
)
foreach ($path in $required) {
    if (-not (Test-Path $path)) {
        throw "Portable package is missing required item: $path"
    }
}

if (Test-Path $forbiddenNestedConfig) {
    throw "Portable package contains forbidden nested configuration directory: $forbiddenNestedConfig"
}

$nestedMatchDirectories = Get-ChildItem -Path $portableDir -Recurse -Directory -Filter "match" |
    Where-Object { $_.FullName -match "[\\/]config[\\/]config[\\/]match$" }
if ($nestedMatchDirectories) {
    throw "Portable package contains nested config/config/match: $($nestedMatchDirectories.FullName -join ', ')"
}

$forbidden = Get-ChildItem -Path $packageDir -Recurse -File |
    Where-Object { $_.Extension -in @(".bat", ".cmd") }
if ($forbidden) {
    throw "Portable package contains forbidden launcher files: $($forbidden.FullName -join ', ')"
}

Compress-Archive -Path $packageDir -DestinationPath $archivePath -Force
Write-Output "Created $archivePath"

#!/usr/bin/env pwsh

$ErrorActionPreference = "Stop"
$TARGET_DIR = "target/windows/resources"

function Main {
    if ([string]::IsNullOrEmpty($env:EXEC_PATH)) {
        Write-Error 'EXEC_PATH is required, for example .\target\release\espanso.exe'
    }
    if (-not (Test-Path -Path $env:EXEC_PATH)) {
        Write-Error "Could not find rEspanso executable $env:EXEC_PATH"
    }

    if ([string]::IsNullOrEmpty($env:EDITOR_PATH)) {
        $binaryDir = Split-Path $env:EXEC_PATH -Parent
        $env:EDITOR_PATH = Join-Path $binaryDir "espanso-editor.exe"
    }
    if (-not (Test-Path -Path $env:EDITOR_PATH)) {
        Write-Error "Could not find Match Studio executable $env:EDITOR_PATH"
    }

    $vcruntimeDll = Get-ChildItem -Path "C:\Program Files\Microsoft Visual Studio" -Recurse -Filter "vcruntime140_1.dll" -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -like "*\VC\Redist\MSVC\*" -and $_.FullName -like "*\x64\*" } |
        Select-Object -First 1 -ExpandProperty FullName
    if (-not $vcruntimeDll) {
        Write-Error "Could not find vcruntime140_1.dll"
    }

    if (Test-Path $TARGET_DIR) {
        Remove-Item $TARGET_DIR -Recurse -Force
    }
    New-Item -Path $TARGET_DIR -ItemType Directory -Force | Out-Null

    $runtimeDir = Split-Path $vcruntimeDll -Parent
    Get-ChildItem -Path $runtimeDir -Filter "*.dll" | Copy-Item -Destination $TARGET_DIR
    Copy-Item -Path $env:EXEC_PATH -Destination "$TARGET_DIR/espansod.exe"
    Copy-Item -Path $env:EDITOR_PATH -Destination "$TARGET_DIR/rEspanso Match Studio.exe"

    $commandContent = '@"%~dp0espansod.exe" %*'
    $commandContent | Out-File "$TARGET_DIR/espanso.cmd" -Encoding ASCII
    Write-Output "Windows resources with Match Studio created"
}

Main @PSBoundParameters

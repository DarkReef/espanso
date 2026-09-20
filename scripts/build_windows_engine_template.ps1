#!/usr/bin/env pwsh

$ErrorActionPreference = "Stop"

Write-Host "== rEspanso Clinical Template Engine / Windows validation =="

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo was not found in PATH"
}

Write-Host "[1/6] rustfmt (advisory)"
cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) {
    Write-Warning "rustfmt reported formatting differences; build validation will continue"
}

Write-Host "[2/6] check editor"
cargo check --locked -p espanso-editor --all-targets

Write-Host "[3/6] template engine tests"
cargo test --locked -p espanso-editor --lib clinical_template_engine

Write-Host "[4/6] editor release build"
cargo build --locked --release -p espanso-editor --bin espanso-editor

Write-Host "[5/6] core + portable launcher release build"
cargo build --locked --release -p espanso --bin espanso --bin respanso-portable

$env:EDITOR_PATH = (Resolve-Path "target/release/espanso-editor.exe").Path
$env:EXEC_PATH = (Resolve-Path "target/release/espanso.exe").Path
$env:LAUNCHER_PATH = (Resolve-Path "target/release/respanso-portable.exe").Path

Write-Host "[6/6] portable package"
& "./scripts/build_windows_native_portable_with_studio.ps1"

$archive = "target/windows/rEspanso-Native-Portable-with-Studio-Windows-x86_64.zip"
if (-not (Test-Path $archive -PathType Leaf)) {
    throw "Portable archive was not created: $archive"
}

$hash = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
Write-Host "OK: $archive"
Write-Host "SHA256: $hash"

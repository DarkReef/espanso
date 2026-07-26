from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    if old not in text:
        raise SystemExit(f"marker not found in {path}: {old[:80]!r}")
    target.write_text(text.replace(old, new, 1), encoding="utf-8")


# Keep the current rEspanso branch as the source of truth and add Studio as a workspace member.
replace_once(
    "Cargo.toml",
    '  "espanso",\n',
    '  "espanso",\n  "espanso-editor",\n',
)
replace_once(
    "Cargo.toml",
    'too_many_lines = "allow"\n',
    'too_many_lines = "allow"\nuninlined_format_args = "allow"\n',
)
replace_once(
    "espanso-editor/Cargo.toml",
    'rhai = "1.25.1"',
    'rhai.workspace = true',
)

# Studio: use the same .espanso portable configuration and prevent duplicate windows.
main_path = Path("espanso-editor/src/main.rs")
main = main_path.read_text(encoding="utf-8")
main = main.replace(
    "    fs::{self, OpenOptions},\n    io::Write,\n    path::{Path, PathBuf},\n    time::SystemTime,\n",
    "    fs::{self, OpenOptions},\n    io::{ErrorKind, Read, Write},\n    path::{Path, PathBuf},\n    time::SystemTime,\n",
    1,
)
main = main.replace(
    "fn launch() -> Result<(), String> {\n    let config_root = parse_config_root().unwrap_or_else(default_config_root);",
    "fn launch() -> Result<(), String> {\n    let Some(_instance_guard) = acquire_instance_lock()? else {\n        return Ok(());\n    };\n    let config_root = parse_config_root().unwrap_or_else(default_config_root);",
    1,
)
old_portable = '''fn portable_config_root() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let directory = executable.parent()?;
    let portable_directory = directory.join("portable");
    let is_named_standalone = executable
        .file_stem()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("rEspanso Match Studio"));

    (portable_directory.is_dir() || is_named_standalone).then(|| portable_directory.join("config"))
}
'''
new_portable = '''fn portable_config_root() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let directory = executable.parent()?;

    let respanso_config = directory.join(".espanso");
    if respanso_config.is_dir() {
        return Some(respanso_config);
    }

    let portable_directory = directory.join("portable");
    let is_named_standalone = executable
        .file_stem()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("rEspanso Match Studio"));

    (portable_directory.is_dir() || is_named_standalone).then(|| portable_directory.join("config"))
}
'''
if old_portable not in main:
    raise SystemExit("portable_config_root marker not found")
main = main.replace(old_portable, new_portable, 1)
lock_code = '''
struct InstanceGuard {
    path: PathBuf,
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn acquire_instance_lock() -> Result<Option<InstanceGuard>, String> {
    let lock_path = instance_lock_path();
    for _ in 0..2 {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                writeln!(file, "{}", std::process::id()).map_err(|error| {
                    format!("Не удалось записать блокировку Studio: {error}")
                })?;
                return Ok(Some(InstanceGuard { path: lock_path }));
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                if lock_owner_is_running(&lock_path) {
                    return Ok(None);
                }
                fs::remove_file(&lock_path).map_err(|remove_error| {
                    format!(
                        "Не удалось удалить устаревшую блокировку {}: {remove_error}",
                        lock_path.display()
                    )
                })?;
            }
            Err(error) => {
                return Err(format!(
                    "Не удалось создать блокировку Studio {}: {error}",
                    lock_path.display()
                ));
            }
        }
    }
    Err("Не удалось получить блокировку единственного экземпляра Studio".to_owned())
}

fn lock_owner_is_running(lock_path: &Path) -> bool {
    use sysinfo::{PidExt, ProcessExt, System, SystemExt};

    let mut pid_text = String::new();
    if OpenOptions::new()
        .read(true)
        .open(lock_path)
        .and_then(|mut file| file.read_to_string(&mut pid_text))
        .is_err()
    {
        return false;
    }
    let Ok(pid) = pid_text.trim().parse::<u32>() else {
        return false;
    };

    let mut system = System::new();
    system.refresh_processes();
    system.process(sysinfo::Pid::from_u32(pid)).is_some_and(|process| {
        let name = process.name().to_ascii_lowercase();
        name.contains("respanso") && name.contains("studio")
    })
}

fn instance_lock_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".respanso-match-studio.lock")
}

'''
marker = "fn parse_config_root() -> Option<PathBuf> {"
if marker not in main:
    raise SystemExit("parse_config_root marker not found")
main = main.replace(marker, lock_code + marker, 1)
main_path.write_text(main, encoding="utf-8")

# CLI: `rEspanso edit --gui` opens the same Studio binary shipped beside the daemon.
replace_once(
    "espanso/src/main.rs",
    '''        .arg(Arg::with_name("target_file")
            .help(r#"Defaults to "match/base.yml", it contains the relative path of the file you want to edit,
''',
    '''        .arg(
          Arg::with_name("gui")
            .long("gui")
            .takes_value(false)
            .help("Open rEspanso Match Studio"),
        )
        .arg(Arg::with_name("target_file")
            .help(r#"Defaults to "match/base.yml", it contains the relative path of the file you want to edit,
''',
)
edit_path = Path("espanso/src/cli/edit.rs")
edit = edit_path.read_text(encoding="utf-8")
edit = edit.replace(
    '''    // Determine which is the file to edit
    let target_file = cli_args.value_of("target_file");
    let target_path = determine_target_path(&paths.config, target_file);
''',
    '''    // Determine which is the file to edit
    let target_file = cli_args.value_of("target_file");
    if cli_args.is_present("gui") || target_file == Some("gui") {
        return open_match_studio(&paths.config);
    }
    let target_path = determine_target_path(&paths.config, target_file);
''',
    1,
)
launcher = '''
fn open_match_studio(config_root: &Path) -> i32 {
    let mut candidates = Vec::new();
    if let Some(explicit) = std::env::var_os("ESPANSO_EDITOR_BIN") {
        candidates.push(PathBuf::from(explicit));
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            for name in editor_binary_names() {
                candidates.push(directory.join(name));
            }
        }
    }
    candidates.extend(editor_binary_names().iter().map(PathBuf::from));

    let mut last_error = None;
    for candidate in candidates {
        match Command::new(&candidate)
            .arg("--config-dir")
            .arg(config_root)
            .spawn()
        {
            Ok(_) => {
                println!("Opened rEspanso Match Studio for {}", config_root.display());
                return 0;
            }
            Err(error) => last_error = Some((candidate, error)),
        }
    }

    if let Some((candidate, error)) = last_error {
        eprintln!(
            "Unable to start rEspanso Match Studio using '{}': {error}",
            candidate.display()
        );
    } else {
        eprintln!("Unable to locate the rEspanso Match Studio executable");
    }
    1
}

#[cfg(target_os = "windows")]
fn editor_binary_names() -> &'static [&'static str] {
    &["rEspanso Match Studio.exe", "espanso-editor.exe"]
}

#[cfg(not(target_os = "windows"))]
fn editor_binary_names() -> &'static [&'static str] {
    &["rEspanso-Match-Studio", "espanso-editor"]
}

'''
edit_marker = "fn determine_target_path(config_path: &Path, target_file: Option<&str>) -> PathBuf {"
if edit_marker not in edit:
    raise SystemExit("edit target marker not found")
edit = edit.replace(edit_marker, launcher + edit_marker, 1)
edit_path.write_text(edit, encoding="utf-8")

# Tray event and menu command.
replace_once(
    "espanso-engine/src/event/mod.rs",
    "    ShowConfigFolder,\n",
    "    ShowConfigFolder,\n    ShowMatchStudio,\n",
)
context_path = Path("espanso-engine/src/process/middleware/context_menu.rs")
context = context_path.read_text(encoding="utf-8")
context = context.replace(
    "const CONTEXT_ITEM_OPEN_CONFIG_FOLDER: u32 = 8;",
    "const CONTEXT_ITEM_OPEN_CONFIG_FOLDER: u32 = 8;\nconst CONTEXT_ITEM_OPEN_MATCH_STUDIO: u32 = 9;",
    1,
)
context = context.replace(
    '''                    MenuItem::Simple(SimpleMenuItem {
                        id: CONTEXT_ITEM_OPEN_CONFIG_FOLDER,
                        label: "Open config folder".to_string(),
                    }),
''',
    '''                    MenuItem::Simple(SimpleMenuItem {
                        id: CONTEXT_ITEM_OPEN_MATCH_STUDIO,
                        label: "Открыть rEspanso Studio".to_string(),
                    }),
                    MenuItem::Simple(SimpleMenuItem {
                        id: CONTEXT_ITEM_OPEN_CONFIG_FOLDER,
                        label: "Open config folder".to_string(),
                    }),
''',
    1,
)
context = context.replace(
    '''                    CONTEXT_ITEM_OPEN_CONFIG_FOLDER => {
                        dispatch(Event::caused_by(
                            event.source_id,
                            EventType::ShowConfigFolder,
                        ));
                        Event::caused_by(event.source_id, EventType::NOOP)
                    }
                    9_u32..=u32::MAX => {
''',
    '''                    CONTEXT_ITEM_OPEN_CONFIG_FOLDER => {
                        dispatch(Event::caused_by(
                            event.source_id,
                            EventType::ShowConfigFolder,
                        ));
                        Event::caused_by(event.source_id, EventType::NOOP)
                    }
                    CONTEXT_ITEM_OPEN_MATCH_STUDIO => {
                        dispatch(Event::caused_by(
                            event.source_id,
                            EventType::ShowMatchStudio,
                        ));
                        Event::caused_by(event.source_id, EventType::NOOP)
                    }
                    10_u32..=u32::MAX => {
''',
    1,
)
context_path.write_text(context, encoding="utf-8")

# Reuse the existing config middleware: it already owns the actual config path.
Path("espanso-engine/src/process/middleware/open_config.rs").write_text(
    r'''/*
 * This file is part of espanso.
 *
 * Copyright (C) 2019-2021 Federico Terzi
 *
 * espanso is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

use super::super::Middleware;
use crate::event::{Event, EventType};
use log::error;
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

pub trait ConfigPathProvider {
    fn get_config_path(&self) -> &Path;
}

pub struct ConfigMiddleware<'a> {
    provider: &'a dyn ConfigPathProvider,
}

impl<'a> ConfigMiddleware<'a> {
    pub fn new(provider: &'a dyn ConfigPathProvider) -> Self {
        Self { provider }
    }
}

impl Middleware for ConfigMiddleware<'_> {
    fn name(&self) -> &'static str {
        "open_config"
    }

    fn next(&self, event: Event, _dispatch: &mut dyn FnMut(Event)) -> Event {
        match event.etype {
            EventType::ShowConfigFolder => {
                open_config_folder(self.provider.get_config_path());
                Event::caused_by(event.source_id, EventType::NOOP)
            }
            EventType::ShowMatchStudio => {
                if let Err(message) = launch_match_studio(self.provider.get_config_path()) {
                    error!("{message}");
                }
                Event::caused_by(event.source_id, EventType::NOOP)
            }
            _ => event,
        }
    }
}

fn open_config_folder(config_path: &Path) {
    let config_path = match config_path.canonicalize() {
        Ok(path) => path,
        Err(err) => {
            error!("unable to canonicalize the config path: {err}");
            config_path.to_owned()
        }
    };

    let program = match env::consts::OS {
        "macos" => "open",
        "windows" => "explorer",
        "linux" => "xdg-open",
        _ => panic!("Unsupported OS"),
    };

    if let Err(err) = Command::new(program).arg(config_path).spawn() {
        error!("unable to open config folder: {err}");
    }
}

fn launch_match_studio(config_path: &Path) -> Result<(), String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("unable to determine rEspanso executable path: {error}"))?;
    let root = executable
        .parent()
        .ok_or_else(|| "unable to determine rEspanso portable root".to_owned())?;

    let studio = studio_candidates(root)
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            format!(
                "rEspanso Match Studio was not found beside {}",
                executable.display()
            )
        })?;

    Command::new(&studio)
        .arg("--config-dir")
        .arg(config_path)
        .current_dir(root)
        .spawn()
        .map_err(|error| format!("unable to start {}: {error}", studio.display()))?;
    Ok(())
}

fn studio_candidates(root: &Path) -> Vec<PathBuf> {
    studio_binary_names()
        .iter()
        .map(|name| root.join(name))
        .collect()
}

#[cfg(target_os = "windows")]
fn studio_binary_names() -> &'static [&'static str] {
    &["rEspanso Match Studio.exe", "espanso-editor.exe"]
}

#[cfg(not(target_os = "windows"))]
fn studio_binary_names() -> &'static [&'static str] {
    &["rEspanso-Match-Studio", "espanso-editor"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_facing_studio_name_is_checked_first() {
        let candidates = studio_candidates(Path::new("portable"));
        assert!(candidates[0]
            .to_string_lossy()
            .to_ascii_lowercase()
            .contains("respanso"));
    }
}
''',
    encoding="utf-8",
)

# Preserve the native rEspanso portable layout and add the Studio executable to it.
Path("scripts/build_windows_resources.ps1").write_text(
    r'''#!/usr/bin/env pwsh

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
''',
    encoding="utf-8",
)

Path("scripts/build_windows_portable.ps1").write_text(
    r'''#!/usr/bin/env pwsh

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
''',
    encoding="utf-8",
)

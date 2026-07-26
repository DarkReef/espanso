#!/usr/bin/env pwsh

# Stop on any error
$ErrorActionPreference = "Stop"

# Enable verbose output
# Set-PSDebug -Strict -Trace 1

$TARGET_DIR = "target/windows/portable"
$RESOURCE_DIR = "target/windows/resources"

# Check if the resources were previously built
if (-not (Test-Path $RESOURCE_DIR)) {
    Write-Error "You need to build the windows resources first.`nPlease run scripts/build_windows_resources.ps1"
    exit 1
}

function Main {
    # Clean the target directory
    if (Test-Path $TARGET_DIR) {
        Remove-Item $TARGET_DIR -Recurse -Force
    }

    # Remove the portable folder if found
    if (Test-Path "target/windows/espanso-portable") {
        Remove-Item "target/windows/espanso-portable" -Recurse -Force
    }

    # Copy the resources directory, including espanso-editor.exe
    Copy-Item -Path $RESOURCE_DIR -Destination $TARGET_DIR -Recurse -Force

    # Create the launcher scripts
    $launcherContent = 'start espansod.exe launcher'
    $launcherContent | Out-File "$TARGET_DIR/START_ESPANSO.bat" -Encoding ASCII
    $editorLauncher = '@"%~dp0espanso-editor.exe" --config-dir "%~dp0portable\config"'
    $editorLauncher | Out-File "$TARGET_DIR/OPEN_MATCH_STUDIO.cmd" -Encoding ASCII

    $portableConfig = "$TARGET_DIR/portable/config"
    New-Item -Path "$portableConfig/match" -ItemType Directory -Force | Out-Null
    New-Item -Path "$TARGET_DIR/.espanso-runtime" -ItemType Directory -Force | Out-Null

    $baseMatchFile = "$portableConfig/match/base.yml"
    if (-not (Test-Path $baseMatchFile)) {
        @(
            'matches:',
            '  - label: "Portable configuration initialized"',
            '    trigger: ":respanso_example"',
            '    replace: "rEspanso Match Studio is ready"',
            '    disabled: true'
        ) | Set-Content -Path $baseMatchFile -Encoding UTF8
    }

    $readmeContent = @"
Welcome to Espanso (Portable edition)!

To start espanso, double click "START_ESPANSO.bat".
To open rEspanso Match Studio, double click "OPEN_MATCH_STUDIO.cmd".

The portable configuration is stored in:
  portable\config

Match Studio is explicitly bound to that directory and loads YAML files from:
  portable\config\match

A starter portable\config\match\base.yml is included so rules can be created immediately.

For more information, please visit the official documentation:
https://espanso.org/docs/

IMPORTANT: Don't delete any file or directory, otherwise espanso won't work.

FOR ADVANCED USERS:

Espanso also offers a rich CLI interface. To start it from the terminal, cd into the
current directory and run "espanso start". You can also run "espanso --help" for more information.

The directory contains "espansod.exe", "espanso-editor.exe" and an "espanso.cmd" file.
You should generally use the provided launcher scripts or the "espanso.cmd" wrapper.
"@
    $readmeContent | Out-File "$TARGET_DIR/README.txt" -Encoding UTF8

    Rename-Item -Path $TARGET_DIR -NewName espanso-portable
    Compress-Archive target/windows/espanso-portable target/windows/Espanso-Win-Portable-x86_64.zip -Force

    Write-Output "Espanso Portable created!"
}

Main @PSBoundParameters

#!/usr/bin/env pwsh

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Executable,

    [Parameter(Mandatory = $true)]
    [string]$Icon
)

$ErrorActionPreference = "Stop"

if (-not $IsWindows) {
    throw "Windows icon resources can only be updated on Windows."
}

$executablePath = (Resolve-Path -Path $Executable).Path
$iconPath = (Resolve-Path -Path $Icon).Path

if (-not ([System.Management.Automation.PSTypeName]'RExpanso.NativeResource').Type) {
    Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;

namespace RExpanso
{
    public static class NativeResource
    {
        public const uint LOAD_LIBRARY_AS_DATAFILE = 0x00000002;

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern IntPtr BeginUpdateResourceW(string fileName, bool deleteExistingResources);

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool UpdateResourceW(
            IntPtr updateHandle,
            IntPtr type,
            IntPtr name,
            ushort language,
            byte[] data,
            uint dataLength);

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool EndUpdateResourceW(IntPtr updateHandle, bool discard);

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern IntPtr LoadLibraryExW(string fileName, IntPtr file, uint flags);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern IntPtr FindResourceExW(
            IntPtr module,
            IntPtr type,
            IntPtr name,
            ushort language);

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool FreeLibrary(IntPtr module);

        public static IntPtr ResourceId(int value)
        {
            return new IntPtr(value);
        }

        public static Win32Exception LastError(string operation)
        {
            return new Win32Exception(Marshal.GetLastWin32Error(), operation);
        }
    }
}
'@
}

function Read-UInt16([byte[]]$Bytes, [int]$Offset) {
    return [BitConverter]::ToUInt16($Bytes, $Offset)
}

function Read-UInt32([byte[]]$Bytes, [int]$Offset) {
    return [BitConverter]::ToUInt32($Bytes, $Offset)
}

$iconBytes = [IO.File]::ReadAllBytes($iconPath)
if ($iconBytes.Length -lt 6) {
    throw "ICO file is too small: $iconPath"
}

$reserved = Read-UInt16 $iconBytes 0
$iconType = Read-UInt16 $iconBytes 2
$imageCount = Read-UInt16 $iconBytes 4
if ($reserved -ne 0 -or $iconType -ne 1 -or $imageCount -lt 1) {
    throw "Invalid Windows ICO header: $iconPath"
}

$directoryLength = 6 + (16 * $imageCount)
if ($iconBytes.Length -lt $directoryLength) {
    throw "ICO directory is truncated: $iconPath"
}

$images = @()
for ($index = 0; $index -lt $imageCount; $index++) {
    $entryOffset = 6 + (16 * $index)
    $dataLength = Read-UInt32 $iconBytes ($entryOffset + 8)
    $dataOffset = Read-UInt32 $iconBytes ($entryOffset + 12)
    $dataEnd = [uint64]$dataOffset + [uint64]$dataLength
    if ($dataLength -eq 0 -or $dataEnd -gt [uint64]$iconBytes.Length) {
        throw "ICO image $index points outside the file: $iconPath"
    }

    $imageData = [byte[]]::new($dataLength)
    [Array]::Copy($iconBytes, [int]$dataOffset, $imageData, 0, [int]$dataLength)
    $images += [pscustomobject]@{
        Width       = $iconBytes[$entryOffset]
        Height      = $iconBytes[$entryOffset + 1]
        ColorCount  = $iconBytes[$entryOffset + 2]
        Reserved    = $iconBytes[$entryOffset + 3]
        Planes      = Read-UInt16 $iconBytes ($entryOffset + 4)
        BitCount    = Read-UInt16 $iconBytes ($entryOffset + 6)
        Length      = $dataLength
        ResourceId  = 1001 + $index
        Data        = $imageData
    }
}

$groupStream = [IO.MemoryStream]::new()
$groupWriter = [IO.BinaryWriter]::new($groupStream)
try {
    $groupWriter.Write([uint16]0)
    $groupWriter.Write([uint16]1)
    $groupWriter.Write([uint16]$imageCount)
    foreach ($image in $images) {
        $groupWriter.Write([byte]$image.Width)
        $groupWriter.Write([byte]$image.Height)
        $groupWriter.Write([byte]$image.ColorCount)
        $groupWriter.Write([byte]$image.Reserved)
        $groupWriter.Write([uint16]$image.Planes)
        $groupWriter.Write([uint16]$image.BitCount)
        $groupWriter.Write([uint32]$image.Length)
        $groupWriter.Write([uint16]$image.ResourceId)
    }
    $groupWriter.Flush()
    $groupData = $groupStream.ToArray()
}
finally {
    $groupWriter.Dispose()
    $groupStream.Dispose()
}

$rtIcon = [RExpanso.NativeResource]::ResourceId(3)
$rtGroupIcon = [RExpanso.NativeResource]::ResourceId(14)
$groupId = [RExpanso.NativeResource]::ResourceId(1)
$languages = [uint16[]]@(0, 0x0409)
$updateHandle = [RExpanso.NativeResource]::BeginUpdateResourceW($executablePath, $false)
if ($updateHandle -eq [IntPtr]::Zero) {
    throw [RExpanso.NativeResource]::LastError("BeginUpdateResourceW failed for $executablePath")
}

$committed = $false
try {
    foreach ($language in $languages) {
        foreach ($image in $images) {
            $resourceId = [RExpanso.NativeResource]::ResourceId([int]$image.ResourceId)
            $updated = [RExpanso.NativeResource]::UpdateResourceW(
                $updateHandle,
                $rtIcon,
                $resourceId,
                $language,
                $image.Data,
                [uint32]$image.Data.Length)
            if (-not $updated) {
                throw [RExpanso.NativeResource]::LastError(
                    "UpdateResourceW failed for icon $($image.ResourceId) in $executablePath")
            }
        }

        $updated = [RExpanso.NativeResource]::UpdateResourceW(
            $updateHandle,
            $rtGroupIcon,
            $groupId,
            $language,
            $groupData,
            [uint32]$groupData.Length)
        if (-not $updated) {
            throw [RExpanso.NativeResource]::LastError(
                "UpdateResourceW failed for the icon group in $executablePath")
        }
    }

    if (-not [RExpanso.NativeResource]::EndUpdateResourceW($updateHandle, $false)) {
        throw [RExpanso.NativeResource]::LastError("EndUpdateResourceW failed for $executablePath")
    }
    $committed = $true
}
finally {
    if (-not $committed) {
        [void][RExpanso.NativeResource]::EndUpdateResourceW($updateHandle, $true)
    }
}

$module = [RExpanso.NativeResource]::LoadLibraryExW(
    $executablePath,
    [IntPtr]::Zero,
    [RExpanso.NativeResource]::LOAD_LIBRARY_AS_DATAFILE)
if ($module -eq [IntPtr]::Zero) {
    throw [RExpanso.NativeResource]::LastError("LoadLibraryExW failed for $executablePath")
}

try {
    $groupResource = [RExpanso.NativeResource]::FindResourceExW(
        $module,
        $rtGroupIcon,
        $groupId,
        [uint16]0x0409)
    if ($groupResource -eq [IntPtr]::Zero) {
        throw [RExpanso.NativeResource]::LastError(
            "The embedded icon group could not be verified in $executablePath")
    }

    foreach ($image in $images) {
        $resourceId = [RExpanso.NativeResource]::ResourceId([int]$image.ResourceId)
        $iconResource = [RExpanso.NativeResource]::FindResourceExW(
            $module,
            $rtIcon,
            $resourceId,
            [uint16]0x0409)
        if ($iconResource -eq [IntPtr]::Zero) {
            throw [RExpanso.NativeResource]::LastError(
                "Embedded icon $($image.ResourceId) could not be verified in $executablePath")
        }
    }
}
finally {
    [void][RExpanso.NativeResource]::FreeLibrary($module)
}

Write-Output "Embedded $imageCount rEspanso icon frames into $executablePath"

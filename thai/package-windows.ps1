# Builds PdfCraft Thai for Windows x64 and packages it, as the official release does, into
#   dist\release\pdfcraft-<version>-windows-x64.msi            per-machine installer
#   dist\release\pdfcraft-<version>-windows-x64-portable.zip   portable (settings beside the exe)
# both with the OCR models. <version> is the edition's, e.g. 0.9.0-th.1 (the MSI's numeric
# version drops the suffix).
#
# Needs, besides what thai\build-windows.ps1 needs: the network (OCR models) and WiX v5
# (winget install WiXToolset.WiXCLI --version 5.0.2.0, or dotnet tool install -g wix --version 5.0.2).
#
#   powershell -ExecutionPolicy Bypass -File thai\package-windows.ps1 -Version 0.9.0-th.1

param(
    [Parameter(Mandatory = $true)] [string] $Version,
    [switch] $SkipBuild
)
$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot

if (-not (Get-Command wix -ErrorAction SilentlyContinue)) {
    $wix = Get-ChildItem "$env:ProgramFiles\WiX Toolset v5*\bin\wix.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $wix) { throw 'WiX v5 not found: winget install WiXToolset.WiXCLI --version 5.0.2.0' }
    $env:Path = "$($wix.DirectoryName);$env:Path"
}
if (-not $SkipBuild) {
    & (Join-Path $PSScriptRoot 'build-windows.ps1')
}
# package.ps1 ships the craft-fonts licences from CRAFT_FONTS_DIR (TH Sarabun PSK's included).
$env:CRAFT_FONTS_DIR = Join-Path $Root 'target\thai-fonts\craft-fonts-3d9ab3a132ed0eff457e424226a435a901da30d3'
$env:PDFCRAFT_VERSION = $Version
& (Join-Path $Root 'packaging\windows\package.ps1') -Arch x64 -SkipBuild

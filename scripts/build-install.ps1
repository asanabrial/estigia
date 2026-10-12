# Rebuilds and performs a complete source installation of Estigia on Windows.
#
# Run from any PowerShell location:
#
#   <checkout>\scripts\build-install.ps1

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$toolchain = '1.97.0'
# The checkout, not this script's own directory: the script lives in `scripts/`
# and everything below builds and installs the crate one level up. Reading
# `$PSScriptRoot` directly here would run cargo against a directory with no
# manifest in it, and the failure names the wrong thing.
$repository = Split-Path -Parent $PSScriptRoot

function Require-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "'$Name' is required but is not available on PATH. Install Rust from https://rustup.rs/ and run this script again."
    }
}

function Invoke-Checked([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "'$Program $($Arguments -join ' ')' failed with exit code $LASTEXITCODE."
    }
}

Require-Command 'rustup'
Require-Command 'cargo'

Write-Host "Installing Rust $toolchain if needed"
Invoke-Checked 'rustup' @('toolchain', 'install', $toolchain, '--profile', 'minimal')

Push-Location $repository
try {
    Write-Host 'Compiling Estigia in release mode'
    Invoke-Checked 'rustup' @('run', $toolchain, 'cargo', 'build', '--release', '--locked')

    Write-Host 'Reinstalling the compiled binary'
    try {
        Invoke-Checked 'rustup' @('run', $toolchain, 'cargo', 'install', '--path', '.', '--locked', '--force')
    } catch {
        throw "$($_.Exception.Message) Close OpenCode and every agent using Estigia, then run the script again."
    }
} finally {
    Pop-Location
}

$installRoot = if ($env:CARGO_INSTALL_ROOT) {
    $env:CARGO_INSTALL_ROOT
} elseif ($env:CARGO_HOME) {
    $env:CARGO_HOME
} else {
    Join-Path $HOME '.cargo'
}
$estigia = Join-Path $installRoot 'bin\estigia.exe'
if (-not (Test-Path -LiteralPath $estigia -PathType Leaf)) {
    throw "Cargo reported success, but the installed executable was not found at '$estigia'."
}

Write-Host 'Performing the complete source-build installation'
Invoke-Checked $estigia @('--version')
Invoke-Checked $estigia @('update')
Invoke-Checked $estigia @('setup', '--all', '--allow-source-build')
Invoke-Checked $estigia @('status')
Invoke-Checked $estigia @('doctor')

Write-Host "Complete installation finished: $estigia"

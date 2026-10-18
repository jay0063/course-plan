# Builds the Windows artifacts on a Windows machine.
#
#   pwsh -File packaging\windows\build-windows.ps1
#   pwsh -File packaging\windows\build-windows.ps1 -SkipInstaller
#   pwsh -File packaging\windows\build-windows.ps1 -StaticCrt
#   pwsh -File packaging\windows\build-windows.ps1 -Target aarch64-pc-windows-msvc
#
# Produces:
#   dist\CoursePlan-<version>-windows-x64.zip      (portable)
#   dist\CoursePlan-Setup-<version>.exe            (Inno Setup installer)

[CmdletBinding()]
param(
    [string]$Target = "x86_64-pc-windows-msvc",
    [switch]$SkipInstaller,
    [switch]$StaticCrt
)

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..\..")

$AppName = "course-plan"
$DistDir = Join-Path $PSScriptRoot "..\..\dist"
$Staging = Join-Path $DistDir "windows"

# CRT linkage. The default (dynamic CRT) leaves the exe depending on the
# VC++ 2015-2022 Redistributable on the target machine. -StaticCrt links the
# CRT into the binary instead, so the portable zip runs on a clean Windows
# install with nothing else to install.
if ($StaticCrt) {
    $staticCrtFlag = "-C target-feature=+crt-static"
    if ($env:RUSTFLAGS) { $env:RUSTFLAGS = "$env:RUSTFLAGS $staticCrtFlag" }
    else { $env:RUSTFLAGS = $staticCrtFlag }
    Write-Host "==> CRT: static (no VC++ Redistributable needed)"
} else {
    Write-Host "==> CRT: dynamic (target machine needs the VC++ 2015-2022 Redistributable)"
}

Write-Host "==> Building release binary ($Target)"
cargo build --release --locked --target $Target

if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$version = (Select-String -Path "Cargo.toml" -Pattern '^version = "(.*)"').Matches[0].Groups[1].Value
$exeName = "$AppName.exe"

# `cargo build --target` writes into target\<triple>\release; a plain
# `cargo build --release` writes into target\release.
$exePath = Join-Path $PSScriptRoot "..\..\target\$Target\release\$exeName"
if (-not (Test-Path $exePath)) {
    $exePath = Join-Path $PSScriptRoot "..\..\target\release\$exeName"
}
if (-not (Test-Path $exePath)) { throw "Built executable not found" }

Write-Host "==> Staging portable bundle"
if (Test-Path $Staging) { Remove-Item $Staging -Recurse -Force }
New-Item -ItemType Directory -Force -Path $Staging | Out-Null
Copy-Item $exePath $Staging
Copy-Item "packaging\icons\course-plan.ico" $Staging

$zip = Join-Path $DistDir "CoursePlan-$version-windows-x64.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path "$Staging\*" -DestinationPath $zip
Write-Host "    $zip"

if (-not $SkipInstaller) {
    $iscc = Get-Command "ISCC.exe" -ErrorAction SilentlyContinue
    if (-not $iscc) {
        foreach ($candidate in @(
            "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
            "${env:ProgramFiles}\Inno Setup 6\ISCC.exe")) {
            if (Test-Path $candidate) { $iscc = $candidate; break }
        }
    }
    if ($iscc) {
        Write-Host "==> Building Inno Setup installer"
        # Pass the real version (single source: Cargo.toml) and the resolved
        # executable path (target\<triple>\release) so the .iss cannot drift.
        & $iscc "/DAppVersion=$version" "/DExePath=$exePath" "packaging\windows\course-plan.iss"
        if ($LASTEXITCODE -ne 0) { throw "Inno Setup failed" }
    } else {
        Write-Warning "ISCC.exe not found; skipping the installer."
        Write-Warning "Install Inno Setup 6, or pass -SkipInstaller."
    }
}

Write-Host "==> Done"

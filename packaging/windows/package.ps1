<#
.SYNOPSIS
  Build and package the PingCraft desktop app for Windows. Untried: Windows builds run in CI only.

.DESCRIPTION
  Produces, in $env:DIST (default: dist/release):
    pingcraft-<version>-windows-<arch>.msi            per-user installer with a setup wizard (WiX v5)
    pingcraft-<version>-windows-<arch>-portable.zip   pingcraft.exe and the notices, to run from anywhere

  SQLite: matrix-sdk's stores need it, and Windows has no system SQLite to link. The SQLite that
  rusqlite can bundle (3.53.2 in libsqlite3-sys 0.38) is older than 3.54.0 (RUSTSEC-2026-0459, deny.toml),
  so this script compiles the pinned SQLite amalgamation below into a static sqlite3.lib with the MSVC
  tools and links that (SQLITE3_LIB_DIR, SQLITE3_STATIC). The exe then carries SQLite 3.54.1, and
  check-sqlite below fails the build if it carries anything older.

  Files are not code-signed yet (v1 builds are unsigned, as CraftCloud's were).

  Needs: Rust (MSVC toolchain + the target), Visual Studio Build Tools with the C++ workload (cl.exe,
  lib.exe, found through vswhere), and WiX v5: dotnet tool install --global wix --version 5.0.2
             wix extension add -g WixToolset.UI.wixext/5.0.2 WixToolset.Util.wixext/5.0.2

  Adapted from CraftCloud's packaging/windows/package.ps1, itself from craft-launcher's (MIT OR Apache-2.0).

.EXAMPLE
  pwsh packaging/windows/package.ps1 -Arch x64
  pwsh packaging/windows/package.ps1 -Arch arm64 -SkipBuild
#>
param(
  [ValidateSet('x64', 'arm64')] [string] $Arch = 'x64',
  [switch] $SkipBuild
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

$Pkg = 'pingcraft'   # cargo package and binary
$AppName = 'PingCraft'

# The SQLite amalgamation linked into the Windows build. SHA-256 computed here; its SHA3-256 matches
# sqlite.org's download page (92281c86e2957f9b9305890f9ac04204caa7b677cc868dc326d942689df28e30).
$SqliteVersion = '3540100'
$SqliteYear = '2026'
$SqliteSha256 = '0a9c8e34352dad46006aa82836d5383bff9dc6bc9ce5a60553c05aa30e6ad1f9'

function Invoke-Native([string] $What, [scriptblock] $Block) {
  Write-Output "==> $What"
  & $Block
  if ($LASTEXITCODE -ne 0) { throw "$What failed with exit code $LASTEXITCODE" }
}

# The version lives in one place: [workspace.package] version in the root Cargo.toml.
$Version = $env:PINGCRAFT_VERSION
if (-not $Version) {
  $inPkg = $false
  foreach ($line in Get-Content (Join-Path $Root 'Cargo.toml')) {
    if ($line -match '^\s*\[') { $inPkg = ($line.Trim() -eq '[workspace.package]'); continue }
    if ($inPkg -and $line -match '^\s*version\s*=\s*"([^"]+)"') { $Version = $Matches[1]; break }
  }
}
if (-not $Version) { throw 'could not read [workspace.package] version from Cargo.toml' }
# MSI ProductVersion is numeric (major.minor.build); pre-release tags are dropped there.
$MsiVersion = ($Version -split '-')[0]

$Target = switch ($Arch) { 'x64' { 'x86_64-pc-windows-msvc' } 'arm64' { 'aarch64-pc-windows-msvc' } }
$Dist = if ($env:DIST) { $env:DIST } else { Join-Path $Root 'dist\release' }
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
New-Item -ItemType Directory -Force -Path $Dist | Out-Null

Write-Output "$AppName $Version for Windows $Arch ($Target)"

# ---- SQLite ------------------------------------------------------------------------------------
function Build-Sqlite {
  $dir = Join-Path $TargetDir "sqlite-$SqliteVersion-$Arch"
  $lib = Join-Path $dir 'sqlite3.lib'
  if (Test-Path $lib) { return $dir }
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
  $zip = Join-Path $dir 'sqlite.zip'
  Invoke-WebRequest -Uri "https://sqlite.org/$SqliteYear/sqlite-amalgamation-$SqliteVersion.zip" -OutFile $zip
  $hash = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLowerInvariant()
  if ($hash -ne $SqliteSha256) { throw "SQLite download has SHA-256 $hash, expected $SqliteSha256" }
  Expand-Archive -Path $zip -DestinationPath $dir -Force
  $src = Join-Path $dir "sqlite-amalgamation-$SqliteVersion\sqlite3.c"
  $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
  $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
  if (-not $vs) { throw 'no Visual Studio C++ tools found (vswhere)' }
  $vcvars = Join-Path $vs 'VC\Auxiliary\Build\vcvarsall.bat'
  $hostArch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'x64' }
  $vcArch = if ($hostArch -eq $Arch) { $Arch } else { "${hostArch}_$Arch" }
  # The static C runtime (/MT), as .cargo/config.toml links Rust's (+crt-static), so the exe needs no
  # Visual C++ redistributable; and thread safety, which matrix-sdk's stores rely on.
  $flags = '/nologo /c /O2 /MT /DSQLITE_THREADSAFE=1 /DSQLITE_ENABLE_FTS5 /DSQLITE_DQS=0'
  Invoke-Native "SQLite $SqliteVersion for $Arch" {
    cmd /c "call `"$vcvars`" $vcArch >nul && cd /d `"$dir`" && cl $flags `"$src`" /Fo`"$dir\sqlite3.obj`" && lib /nologo /OUT:`"$lib`" `"$dir\sqlite3.obj`""
  }
  return $dir
}

if (-not $SkipBuild) {
  $env:SQLITE3_LIB_DIR = Build-Sqlite
  $env:SQLITE3_STATIC = '1'
  # Packages carry colour emoji: fetch the pinned font (checked against its hash) and fail the build without it.
  $env:CRAFT_FONTS_REQUIRED = '1'
  Invoke-Native 'cargo xtask fonts' { cargo xtask fonts }
  Invoke-Native "cargo build ($Target)" { cargo build --release --locked -p $Pkg --target $Target }
}

$Bin = Join-Path $TargetDir "$Target\release"
$Exe = "$Pkg.exe"

# Check the PE header before packaging. Machine (COFF header) must match -Arch, so an x64 build can never ship
# labelled arm64. Subsystem (optional header) must be 2 = Windows GUI, so no console window opens with it.
function Get-PeHeader([string] $Path) {
  $bytes = [System.IO.File]::ReadAllBytes($Path)
  $pe = [BitConverter]::ToInt32($bytes, 0x3C)
  return @{ Machine = [BitConverter]::ToUInt16($bytes, $pe + 4); Subsystem = [BitConverter]::ToUInt16($bytes, $pe + 0x5C) }
}
$Machine = switch ($Arch) { 'x64' { 0x8664 } 'arm64' { 0xAA64 } }
$h = Get-PeHeader (Join-Path $Bin $Exe)
if ($h.Machine -ne $Machine) { throw "$Exe is for machine 0x$('{0:X}' -f $h.Machine), expected 0x$('{0:X}' -f $Machine) ($Arch)" }
if ($h.Subsystem -ne 2) { throw "$Exe has PE subsystem $($h.Subsystem), expected 2 (Windows GUI)" }
Write-Output "ok ${Exe}: $Arch, PE subsystem $($h.Subsystem)"

# No SQLite older than 3.54.0 in what ships (as packaging/env.sh's check_sqlite).
$text = [System.Text.Encoding]::ASCII.GetString([System.IO.File]::ReadAllBytes((Join-Path $Bin $Exe)))
if ($text.Contains('SQLite format 3')) {
  $versions = [regex]::Matches($text, '(?<![0-9.])3\.[0-9]{2}\.[0-9]+(?![0-9.])') | ForEach-Object { [version]$_.Value } | Sort-Object
  $found = $versions | Select-Object -Last 1
  if (-not $found) { throw "$Exe has SQLite compiled in, and its version could not be read" }
  if ($found -lt [version]'3.54.0') { throw "$Exe ships SQLite $found, older than 3.54.0 (RUSTSEC-2026-0459)" }
  Write-Output "ok ${Exe}: ships SQLite $found"
}

$Stage = Join-Path $TargetDir "windows-package\$Arch"
Remove-Item -Recurse -Force $Stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Stage | Out-Null
Copy-Item -LiteralPath (Join-Path $Root 'NOTICE') -Destination $Stage
Copy-Item (Join-Path $Bin $Exe) $Stage

# ---- MSI ---------------------------------------------------------------------------------------
$Msi = Join-Path $Dist "$Pkg-$Version-windows-$Arch.msi"
$Icon = Join-Path $Root 'design\assets\logo\pingcraft.ico'
Invoke-Native 'wix build' {
  wix build (Join-Path $PSScriptRoot "$Pkg.wxs") -arch $Arch `
    -ext WixToolset.UI.wixext -ext WixToolset.Util.wixext `
    -culture en-US -loc (Join-Path $PSScriptRoot "$Pkg.en-us.wxl") `
    -d "Version=$MsiVersion" -d "DisplayVersion=$Version" -d "BinDir=$Stage" -d "IconPath=$Icon" `
    -o $Msi
}
Invoke-Native 'MSI shortcut icon validation (ICE50)' {
  wix msi validate $Msi -ice ICE50 -intermediateFolder (Join-Path $Stage 'msi-validation')
}
# wix writes its debug symbols (.wixpdb) next to the MSI; keep them out of the release files.
Remove-Item -Force -ErrorAction SilentlyContinue ([IO.Path]::ChangeExtension($Msi, '.wixpdb'))

# ---- portable zip ------------------------------------------------------------------------------
$Portable = Join-Path $TargetDir "windows-package\$Pkg-$Version-windows-$Arch-portable"
Remove-Item -Recurse -Force $Portable -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Portable | Out-Null
Copy-Item (Join-Path $Stage $Exe) $Portable
foreach ($f in 'LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE', 'ATTRIBUTION.md') {
  $p = Join-Path $Root $f
  if (Test-Path $p) { Copy-Item $p $Portable }
}
Copy-Item (Join-Path $Root 'packaging\README-desktop.md') (Join-Path $Portable 'README.md')
$Zip = Join-Path $Dist "$Pkg-$Version-windows-$Arch-portable.zip"
Remove-Item -Force $Zip -ErrorAction SilentlyContinue
Compress-Archive -Path $Portable -DestinationPath $Zip

Get-Item $Msi, $Zip | Format-Table Name, Length

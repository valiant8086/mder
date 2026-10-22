# Builds a release of mder and packs it into dist\, which git ignores: the
# files in dist\mder\, and the download, a 7-Zip self-extractor named for
# the version's first two parts and the processor, such as
# dist\mder0.2-windows-x64.exe or -arm64.exe. The updater runs that same
# file with `-y -o<mder's folder>`.
#
#   pwsh scripts\dist.ps1                 # build and pack
#   pwsh scripts\dist.ps1 -Install C:\tools\mder   # also copy it there
#
# Set MDER_VERSION to build and name it as another version, for testing the
# updater. Finds CMake and Ninja from Visual Studio when they aren't on the
# PATH, and 7-Zip in Program Files when it isn't either.
param([string]$Install)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

foreach ($tool in 'cmake', 'ninja') {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
        if (-not (Test-Path $vswhere)) { throw "$tool isn't on the PATH and Visual Studio wasn't found." }
        $vs = & $vswhere -latest -products * -property installationPath
        $cmakeDir = Join-Path $vs 'Common7\IDE\CommonExtensions\Microsoft\CMake'
        $env:PATH = "$cmakeDir\CMake\bin;$cmakeDir\Ninja;$env:PATH"
        break
    }
}

cargo build --release
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

$version = $env:MDER_VERSION
if (-not $version) {
    $version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
}
$short = ($version -split '\.')[0..1] -join '.'
$out = Join-Path $root 'dist\mder'
Remove-Item $out -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item target\release\mder.exe, README.md, LICENSE-MIT.txt, LICENSE-APACHE.txt, NOTICE.txt $out

$sevenZip = (Get-Command 7z -ErrorAction SilentlyContinue).Source
if (-not $sevenZip) { $sevenZip = "$env:ProgramFiles\7-Zip\7z.exe" }
$sfxModule = Join-Path (Split-Path $sevenZip) '7z.sfx'
if (-not (Test-Path $sfxModule)) { throw "7-Zip's 7z.sfx wasn't found next to $sevenZip." }
$arch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'x64' }
$sfx = Join-Path $root "dist\mder$short-windows-$arch.exe"
Remove-Item $sfx -ErrorAction SilentlyContinue
# The files go at the top of the archive, so they unpack straight into mder's folder.
& $sevenZip a "-sfx$sfxModule" -mx=9 $sfx "$out\*" | Out-Null
if ($LASTEXITCODE -ne 0) { throw '7-Zip failed' }
"Built $sfx, version $version ($([math]::Round((Get-Item $sfx).Length / 1MB, 1)) MB)"

if ($Install) {
    New-Item -ItemType Directory -Force $Install | Out-Null
    # A running mder.exe can't be overwritten, but it can be renamed. mder
    # deletes the leftover mder.exe.old the next time it starts.
    $exe = Join-Path $Install 'mder.exe'
    $old = "$exe.old"
    Remove-Item $old -Force -ErrorAction SilentlyContinue
    if (Test-Path $exe) {
        try { Remove-Item $exe -Force -ErrorAction Stop }
        catch { Rename-Item $exe 'mder.exe.old' -Force; 'mder is running, so the new copy takes over the next time you start it.' }
    }
    Copy-Item "$out\*" $Install -Force
    "Installed to $Install. Run '$Install\mder.exe --register' once to add it to Open with."
}

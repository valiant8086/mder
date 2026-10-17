# Builds a release of mder and packs it into dist\, which git ignores.
#
#   pwsh scripts\dist.ps1                 # build, then dist\mder\ and a zip
#   pwsh scripts\dist.ps1 -Install C:\tools\mder   # also copy it there
#
# Finds CMake and Ninja from Visual Studio when they aren't on the PATH.
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

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$out = Join-Path $root 'dist\mder'
Remove-Item $out -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item target\release\mder.exe, README.md, LICENSE-MIT.txt, LICENSE-APACHE.txt, NOTICE.txt $out

$zip = Join-Path $root "dist\mder-$version-windows-x64.zip"
Remove-Item $zip -ErrorAction SilentlyContinue
Compress-Archive -Path "$out\*" -DestinationPath $zip
"Built $zip ($([math]::Round((Get-Item "$out\mder.exe").Length / 1MB, 1)) MB exe)"

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

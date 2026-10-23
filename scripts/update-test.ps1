# Sets up a test of mder's updater on Windows, with nothing published:
#
#   <Folder>\app\      an "old" mder, set to look for updates at the test server
#   <Folder>\server\   a "new" release: its self-extractor and changelog.json
#
#   pwsh scripts\update-test.ps1 -Folder C:\temp\mder-update-test
#   python -m http.server 8086 --bind 127.0.0.1 --directory C:\temp\mder-update-test\server
#
# Then run <Folder>\app\mder.exe on a file and choose Help, Check for update.
# The server only answers this machine; -Source gives another address, for
# serving the folder to other machines too.
param(
    [Parameter(Mandatory)][string]$Folder,
    [string]$OldVersion = '0.1.0',
    [string]$NewVersion = '0.1.1',
    [string]$Source = 'http://127.0.0.1:8086/'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$app = Join-Path $Folder 'app'
$server = Join-Path $Folder 'server'

try {
    $env:MDER_VERSION = $OldVersion
    & "$PSScriptRoot\dist.ps1"
    Remove-Item $app -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $app | Out-Null
    Copy-Item "$root\dist\mder\*" $app
    # Due for a daily check straight away, and pointed at the test server.
    "check_for_updates=yes`nlast_update_check=0`nupdate_source=$Source`n" | Set-Content -NoNewline (Join-Path $app 'mder.ini')

    $env:MDER_VERSION = $NewVersion
    & "$PSScriptRoot\dist.ps1"
    Remove-Item $server -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $server | Out-Null
    Copy-Item "$root\dist\mder$NewVersion-windows-*.exe" $server
} finally {
    Remove-Item Env:MDER_VERSION -ErrorAction SilentlyContinue
}

# The repo's changelog, with the test version on top.
$changelog = @(Get-Content "$root\changelog.json" -Raw | ConvertFrom-Json)
$test = [pscustomobject]@{
    version = $NewVersion
    date    = (Get-Date).ToString('M/d/yyyy')
    blurb   = 'A test update.'
    changes = @("A test release, $NewVersion, to try mder's updater.", 'Nothing else changed.')
}
ConvertTo-Json -InputObject (@($test) + $changelog) -Depth 5 | Set-Content (Join-Path $server 'changelog.json') -Encoding utf8

"Old mder $OldVersion is in $app; release $NewVersion is in $server."
"Serve it with: python -m http.server 8086 --bind 127.0.0.1 --directory `"$server`""

# Builds the release files and packs them for unpacking into the game folder. It only writes under
# dist\ and never touches the game or the user's settings. A package without a LICENSE file is for
# local testing only.

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

cargo build --locked --release -p wd_game -p wd_setup
if ($LASTEXITCODE -ne 0) { throw 'The build failed.' }

$manifest = Get-Content (Join-Path $root 'Cargo.toml') -Raw
if ($manifest -notmatch '(?m)^version = "([^"]+)"') { throw 'No version in Cargo.toml.' }
$version = $Matches[1]

$name = "NightWheel-$version"
$stage = Join-Path $root "dist\$name"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
$pluginFolder = Join-Path $stage 'red4ext\plugins\NightWheel'
$scriptFolder = Join-Path $stage 'r6\scripts\NightWheel'
New-Item -ItemType Directory -Force $pluginFolder, $scriptFolder | Out-Null

Copy-Item 'target\release\wd_game.dll' (Join-Path $pluginFolder 'NightWheel.dll')
Copy-Item 'target\release\wd_setup.exe' (Join-Path $pluginFolder 'NightWheelSetup.exe')
Copy-Item 'crates\wd_game\game\NightWheel.reds' $scriptFolder
Copy-Item 'README.md', 'THIRD_PARTY_NOTICES.md' $pluginFolder
if (Test-Path 'LICENSE') {
    Copy-Item 'LICENSE' $pluginFolder
} else {
    Write-Warning 'No LICENSE file. Do not distribute this package.'
}

# The checksums list every file by its path from the game folder.
$lines = Get-ChildItem $stage -Recurse -File | Sort-Object FullName | ForEach-Object {
    $relative = $_.FullName.Substring($stage.Length + 1).Replace('\', '/')
    $hash = (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower()
    "$hash  $relative"
}
Set-Content -Path (Join-Path $pluginFolder 'SHA256SUMS.txt') -Value $lines -Encoding ascii

$zip = Join-Path $root "dist\$name.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
# The Windows tar writes zip files. Other tar programs on PATH may not.
& (Join-Path $env:SystemRoot 'System32\tar.exe') -a -c -f $zip -C $stage r6 red4ext
if ($LASTEXITCODE -ne 0) { throw 'Packing failed.' }

Write-Output "Package: $zip"
Write-Output "SHA-256: $((Get-FileHash $zip -Algorithm SHA256).Hash.ToLower())"

# Builds the release files and packs them for unpacking into the game folder. It only writes under
# dist\ and never touches the game or the user's settings. A package without a LICENSE file is for
# local testing only.

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# Keeps this computer's folder names out of the released files. Cargo's encoded form is used because
# these paths can contain spaces.
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $HOME '.cargo' }
$rustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { Join-Path $HOME '.rustup' }
$env:CARGO_ENCODED_RUSTFLAGS = @(
    "--remap-path-prefix=$cargoHome=cargo",
    "--remap-path-prefix=$rustupHome=rustup",
    "--remap-path-prefix=$root=src",
    '-Clink-arg=/PDBALTPATH:%_PDB%'
) -join [char]0x1f
# The game SDK is C++ and keeps its header paths in its asserts, so its compiler needs this too.
$env:CXXFLAGS = "/d1trimfile:$cargoHome"
cargo clean --release -p red4ext-rs
cargo build --locked --release -p wd_game -p wd_setup
$built = $LASTEXITCODE
Remove-Item Env:CARGO_ENCODED_RUSTFLAGS, Env:CXXFLAGS
if ($built -ne 0) { throw 'The build failed.' }

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
# Plain line feeds, so `sha256sum -c` can read the file.
[IO.File]::WriteAllText((Join-Path $pluginFolder 'SHA256SUMS.txt'), (($lines -join "`n") + "`n"))

$zip = Join-Path $root "dist\$name.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
# The Windows tar writes zip files. Other tar programs on PATH may not.
& (Join-Path $env:SystemRoot 'System32\tar.exe') -a -c -f $zip -C $stage r6 red4ext
if ($LASTEXITCODE -ne 0) { throw 'Packing failed.' }

Write-Output "Package: $zip"
Write-Output "SHA-256: $((Get-FileHash $zip -Algorithm SHA256).Hash.ToLower())"

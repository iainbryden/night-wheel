# Night Wheel

A Logitech G29 driving mod for Cyberpunk 2077 on Windows, written independently in Rust and redscript. It has not been released. [docs/RELEASE_STATUS.md](docs/RELEASE_STATUS.md) lists what works, what doesn't exist yet and what blocks a release.

## What it does today

- Steers and drives with the G29, using calibrated steering and pedals.
- Has a **WHEEL SETTINGS** page in the pause menu, where you bind wheel controls to functions with one press.
- Controls vehicle functions, the camera and the radio from the wheel.
- Adds boost, with an adjustable strength.
- Adds flight: **Climb** lifts the car and keeps rising while held. **Descend** lowers it, and holding it at the ground for half a second ends flight. Flight doesn't check for buildings ahead, so the car can pass through them.

Force feedback, wheel LEDs and manual shifting don't exist yet. The mod sends nothing to the wheel.

## Requirements

Tested on patch 2.5 (Steam) with these versions, which you install separately:

| Dependency | Tested version |
|---|---|
| [RED4ext](https://github.com/WopsS/RED4ext) | 1.31.0 |
| [redscript](https://github.com/jac3km4/redscript) | 0.5.32 |
| [Codeware](https://github.com/psiberx/cp2077-codeware) | 1.21.2 |
| [TweakXL](https://github.com/psiberx/cp2077-tweak-xl) | 1.12.0 |

Logitech G HUB must be running. These G HUB settings were used in testing: operating range 240, sensitivity 24, and the centering spring for non force feedback games off.

## Install

1. Close the game and unpack the package into the game folder, so that `r6` and `red4ext` merge with the existing folders.
2. Run `red4ext\plugins\NightWheel\NightWheelSetup.exe` and calibrate the wheel. Without a calibration the wheel doesn't drive, and the game keeps its normal input.
3. Start the game, pause, and choose **WHEEL SETTINGS** to bind your controls.

## Uninstall

1. Close the game.
2. Delete `r6\scripts\NightWheel` and `red4ext\plugins\NightWheel` from the game folder.
3. To remove your settings and logs as well, delete `%LOCALAPPDATA%\NightWheel`.

The mod doesn't edit saves. Do not use it alongside another wheel mod.

## Settings and logs

Settings and logs stay on your PC in `%LOCALAPPDATA%\NightWheel`. Nothing is uploaded.

| File | Contents |
|---|---|
| `profile.cfg`, `profile.cfg.bak` | Calibration, bindings and boost strength, with a backup |
| `steering-speed.txt` | Optional steering speed multiplier from 1 to 10. It applies at the next game start, and the default is 10 |
| `night-wheel.log`, `night-wheel.old.log` | This launch's log and the last one. Each stops growing at 20 MB |

Settings from test builds in `%LOCALAPPDATA%\WheelDrive` are copied once, if the new folder has none. The old folder isn't changed.

## Build from source

Needs [rustup](https://rustup.rs/), the Visual Studio C++ build tools, CMake 3.21 or newer and LLVM, all on `PATH`. If `libclang.dll` isn't found, set `LIBCLANG_PATH` to its folder. `rust-toolchain.toml` selects the Rust version. Use a short folder path: a build under a very long temporary path failed in CMake's compiler check, and the same commit built from a shorter one.

```
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
```

To build the package in `dist\`:

```
powershell -ExecutionPolicy Bypass -File scripts\package.ps1
```

The Rust checks don't cover the game script. redscript compiles `NightWheel.reds` against the game's own scripts when the game starts, so a script error only shows up there unless you run `redscript-cli` against a copy of the game's `final.redscripts`.

## Layout

| Path | Contents |
|---|---|
| `crates/wd_core` | Calibration, settings and binding logic. No game or Windows code |
| `crates/wd_device` | Read-only wheel input through Windows Raw Input |
| `crates/wd_game` | The game plugin, and `game/NightWheel.reds`, the game script |
| `crates/wd_setup` | The setup and calibration app |
| `crates/wd_diag` | A standalone wheel diagnostic that only reads |
| `scripts/` | Packaging and the third-party list |
| `docs/`, `evidence/` | Decisions, records, research, source and dependency registers, and test results |

The `wd_` crate names are internal and predate the product name.

## How it was built

This is a new implementation written from behaviour requirements, public documentation and observation of the unmodified game and wheel, without reading any other wheel or gameplay mod. `evidence/SOURCES.csv` lists every outside source.

## License

[MIT](LICENSE). Libraries built into the package keep their own licenses, listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

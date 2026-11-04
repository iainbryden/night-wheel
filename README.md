# Wheel mod (working title)

An independently written Logitech G29 driving mod for Cyberpunk 2077 on Windows. It does not work in the game yet.

## Layout

| Path | Contents |
|---|---|
| `crates/wd_core` | Calibration, configuration and HID value logic. No dependencies, no `unsafe` |
| `crates/wd_device` | Read-only wheel input through Windows Raw Input |
| `crates/wd_diag` | Standalone wheel diagnostic |
| `crates/wd_setup` | Setup app: device view, calibration, response settings and assignments |
| `evidence/`, `docs/` | Source and dependency registers, test results, decisions, reviews |

## Build

Needs [rustup](https://rustup.rs/) and the Visual Studio C++ build tools. `rust-toolchain.toml` selects the Rust version.

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Run the setup app

```
cargo run -p wd_setup
```

The app sends nothing to the wheel and nothing to the game. Settings are saved to `%LOCALAPPDATA%\WheelDrive\profile.cfg`, with a backup beside it.

## Run the diagnostic

The diagnostic only reads. It uses Windows Raw Input, never opens the device for writing, and sends no motor or LED output.

```
cargo run -p wd_diag                       # live screen; Ctrl+C to quit
cargo run -p wd_diag -- --list             # controllers and their controls
cargo run -p wd_diag -- --plain            # one line per change
cargo run -p wd_diag -- --plain --seconds 30
```

Control names such as X or Rz come from what the device reports, not from which physical control they are.

## License

Not chosen yet (decision D14). No license is granted until one is added.

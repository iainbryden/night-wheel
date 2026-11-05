# Test P3-BUTTONS-BUILD: wheel buttons on the settings panel, built but not run

Date: 2026-11-05
Result: PASS for build and automated checks. Not run in the game.
Evidence type: AUTO
Source checkpoint and hashes: branch `game-load-test`; release `wd_game.dll` SHA-256 `361c8a623f4e3a36174bda6e6449a75ef773f39e411e155aa2bcb9428f955020`; `WheelDriveLoadTest.reds` SHA-256 `a1d7044c711bf1df68395cb615e971392c7f898cec97088bde82d49244476ae8`; `Cargo.lock` SHA-256 `7259ffdc727b80016f4d6b11d403f02e7f15f7c4a5b0a6d6a248eee0cdd4b0e9`

## What changed

- **Buttons are read.** `Device::apply_values` now also decodes held buttons into a 64-bit value on the wheel reader's thread, using a stack buffer and no allocation. The plugin stores it, and the D-pad's hat value, in atomics.
- **The panel shows what is held.** While the panel is open, it lists held controls by their reported numbers. The script logs each change, so the log gives the order of presses. Text is built only while the panel is open.
- **Naming.** The pause menu entry reads WHEEL SETTINGS and the panel's title reads NIGHT WHEEL.

The buttons do nothing in the game in this build. Nothing is sent to the game or the wheel because of them.

## Latency

The new decoding runs per report on the wheel reader's thread, and the game's poll reads steering from an atomic without waiting for it. It takes time from that thread, between reports about 2 ms apart. The `decode` stat will show the new cost against the earlier typical 4 µs or less. Nothing has been measured in this build yet.

## Checks

- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- `cargo test --locked --workspace`: 118 of 118 passed.
- `redscript-cli` compiled the script against a copy of the game's `final.redscripts`.
- Files staged in the ignored folder `out/buttons-test/`.

## Limitations

- Which number belongs to which physical control is unknown until the user presses them. That is the P1 gate.
- The value the D-pad reports at rest has not been observed. Values outside 0 to 7 are treated as not held, per the reported range.
- The panel still doesn't take over input, so the pause menu can react to mouse and keyboard, and the game's menu may or may not react to the wheel.

## Next step

With the user's approval, install with the multiplier at 2. The user presses each wheel control in a given order with the panel open, and also checks the game's own Load Game and Settings entries.

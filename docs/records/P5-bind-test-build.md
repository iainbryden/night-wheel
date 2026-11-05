# Test P5-BIND-BUILD: settings screen with Bind, built but not run

Date: 2026-11-05
Result: PASS for build and automated checks. Not run in the game.
Evidence type: AUTO
Source checkpoint and hashes: branch `game-load-test`; release `wd_game.dll` SHA-256 `804157e1c364cfebdebd6db91510ee300c31432b8f604e6f0934e376afb5cb6d`; `WheelDriveLoadTest.reds` SHA-256 `33965ec9001b69feb78065c64c3b9181c29caba83accc22423b4664d4dc6c3c4`; `Cargo.lock` SHA-256 `7259ffdc727b80016f4d6b11d403f02e7f15f7c4a5b0a6d6a248eee0cdd4b0e9`

## What it does

- **Screen.** WHEEL SETTINGS lists eight functions and each one's bound control, using the names in `docs/records/P1-control-map.md`.
- **Bind flow.** Choose a function, press a wheel control, then confirm. Cancel and a 10-second timeout are included, and a control held before listening is ignored until released. Binding a control already in use warns, and confirming moves it. Clear removes a binding.
- **Controls.** Keyboard: Up, Down, Enter, Delete, period. Wheel: D-pad, X, Square, Circle.
- **Pause menu.** While the screen is open, the pause menu's entries do nothing. Its highlight can still move underneath, and Escape still leaves the pause menu.
- **Saving.** Each change saves through the existing `config::save`, which keeps a backup. Bindings load at the next launch.
- **In the game.** While driving, a bound control presses a gamepad button through the existing XInput hook, in the same reply that carries steering.

## Functions

The pairs come from the game's `inputContexts.xml` and `inputUserMappings.xml` on patch 2.5 (SRC-032). Whether each one behaves well from the wheel is what the game test must show.

| Function | Game action | Gamepad button |
|---|---|---|
| Handbrake | `Handbrake` | A |
| Horn | `VehicleHorn` | X |
| Lights | `CycleLights` | D-pad left |
| Change camera | `ToggleVehCamera` | D-pad right |
| Exit vehicle | `Exit`, `QuickExit` | B |
| Autodrive | `HoldAutodrive` | Left stick press |
| Radio | `VehicleInsideWheel` | Right shoulder |
| Combat mode | `EnterCombatMode` | Y |

Shifting, boost, profiles and the rest are left off the screen until they work.

## Safety and latency

- Buttons apply only under the same gate as steering: connected, driving, not paused, fresh script updates.
- A control held while the gate is closed must be released first, so nothing replays on resume, on exit or after binding.
- A press that ends between two polls still counts for one poll, because dial clicks can be that short. The reader thread keeps the press, and the poll reads it from an atomic.
- At each poll, this adds a few atomic operations and no locks or allocation. It has not been measured in the game. The screen's text and locks run only while it is open, on the UI refresh.

## Checks

- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- `cargo test --locked --workspace`: 137 of 137 passed.
- `redscript-cli` compiled the script against a copy of the game's `final.redscripts`.
- A source search of `wd_game` and `wd_device` found no device-write, input-injection or vibration calls.
- Files staged in the ignored folder `out/bind-test/`.

## Limitations

- Nothing here has run in the game.
- Order of events is unknown when Enter opens the screen. If the same press reaches the screen, Bind could start at once. Period cancels it.
- Hints from the game may name the gamepad button, not the wheel control. Some functions may also respond differently to a tap and a hold.
- Right shoulder also fires in driver combat and with mounted weapons, per the game's mappings, so the Radio binding may do something else in those states. Lights and camera share their gamepad buttons with other contexts in the same way.
- L3's name rests on elimination. The steering part of calibration isn't on this screen yet.

## Game run, 2026-11-05

Approved by the user; files replaced and hash-checked, with `profile.cfg` copied to `out/game-before/profile-before-bind.cfg`. The test stopped early because of the screen. The game was still open when the log was read, so shutdown is not covered.

- **User report:** navigating the screen was poor. The wheel's D-pad also moved the pause menu's left column. The user asked for it to follow the game's Key Bindings page.
- **Log:** binding and saving worked from the screen. `profile.cfg` holds `horn = button 1` (X) from this run. The functions were not reported tested in a car.
- **Unexplained:** the plugin passes no wheel buttons to the game during pause, so the D-pad reached the menu some other way. The game's executable declares HID, and G HUB or Steam Input are also candidates. Not investigated further. Earlier reports of wheel presses and key events did not settle it.
- The script's small test line also appeared over the game's Settings pages.

## Second build: full page

Plugin 0.11.0. `wd_game.dll` SHA-256 `9935fa037607cc546a08054595fe2f95c039a650c618adcad5c404dbb96b2590`; `WheelDriveLoadTest.reds` SHA-256 `9fc03fa15d534ce3ec5e672fcb58a9730fb414314982be7036f149007cca66e4`. Staged in `out/page-test/`. Not run in the game.

- **Layout.** A full opaque page with a heading and rows that have a label on the left and a framed value on the right, following the arrangement of the game's Key Bindings page. Original widgets only, with no game or mod layout files reused. The small test line is now blank.
- **Pause menu.** Hidden while the page is open, and its entries still do nothing.
- **Mouse.** Hover selects a row and a click starts Bind. Whether this layer receives pointer events is unproven, and the log records `wheel-row-clicked`.
- **Back.** Escape still leaves the pause menu entirely. On first opening, the script logs the pause menu's function names through Codeware's reflection, to look for a way to handle going back.
- **Font.** Row text uses a font path that isn't proven. If it doesn't load, the text may be blank or use a default, while the heading and hints use the proven font.

Checks: lints clean, 138 of 138 tests passed, script compiled against a copy of the game's `final.redscripts`.

## Next step

With the user's approval and the game closed, install the second build. Then test the page, binding, each function in a car, saving across a restart, and no replay on resume.

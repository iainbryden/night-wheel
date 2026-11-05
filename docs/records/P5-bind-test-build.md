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

## Second build's game run, 2026-11-05

Approved by the user; files replaced and hash-checked. The game was still open when the log was read at about 11 minutes.

- **User report:** the user suspects conflicts between button assignments, is not sure, does not like confirming with X, and asked for the menu to be reevaluated. No per-function results in a car were reported.
- **Mouse works:** the log shows `wheel-row-clicked` many times, so this layer receives pointer events. Row text was not reported as a problem.
- **Binding and saving work:** all six requested bindings were saved with the expected numbers: horn 6, camera 5, lights 22, radio 24, handbrake 8, exit 7.
- **Going back:** the logged function list includes `gameuiMenuGameController.OnBack` and `PauseMenuGameController.OnGlobalRelease`. They are candidates for handling Escape. What they do has not been tested.

Conflicts visible in the design, whether or not they are what the user saw:

- X, Square, Circle and the D-pad both run the screen and can be bound.
- Each function presses a gamepad button that the game also uses elsewhere. Right shoulder and D-pad left are examples in the game's mappings.
- The route by which the wheel's D-pad reached the pause menu is still unknown.
- Nothing logs which gamepad buttons were pressed in a car, so a conflict there can't be shown from this log.

The game was then closed. The plugin unloaded and no new crash report was written.

## Third build: one press to bind

The decision is in `docs/decisions/D22-settings-page-input.md`. Plugin 0.12.0. `wd_game.dll` SHA-256 `4c77f2ebdbd914118d8c475f64d5cf980e3f5f3d4bfdf6a7b03cf7fe9291b1f2`; `WheelDriveLoadTest.reds` SHA-256 `131606555b693adb1f5de6e30aee432e599bee1cd931498914aa1aec263a25e8`. Staged in `out/oneclick-test/`. Not run in the game.

- One press sets a binding, and a control already in use moves.
- The wheel no longer runs the page.
- The game's `back` action cancels a bind or leaves the page, through a wrap of `OnGlobalRelease`.
- Pressing a bound control lights its row for a moment.
- Each stats line now has `fired=`, naming the functions the wheel pressed in that second, so conflicts in a car can be read from the log. Hot-path cost is one atomic write, only when something is pressed.

Checks: lints clean, 141 of 141 tests passed, script compiled against a copy of the game's `final.redscripts`.

## Third build's game run, 2026-11-05

Approved by the user; files replaced and hash-checked. No crash report, and the game was still open when the log was read.

- **Going back failed.** The log shows `wheel-back`, then `session-resume` about 10 ms later. So the wrap runs, and the pause menu closes anyway.
- **Circle bound** to Autodrive, so the game did not treat the wheel's Circle as back.
- **Horn.** `fired=horn` appears in about 29 separate seconds, including after the user says it stopped. The user says it works once and returns after leaving and re-entering the car, and recalls this as a possible game bug. The user chose to skip the keyboard comparison, so the cause is not established.
- **Autodrive.** The user hit something with it on, and the game did not crash. The wheel's steering and pedals still apply during Autodrive, which may be fighting it. This is not established.
- **Handbrake.** The log shows it never fired in this run, though the user recalls it working. To be retried.

## Fourth build: more functions and page sections

Plugin 0.13.0. `wd_game.dll` SHA-256 `bbf5e3bbc4d4f38e0bb4ffcc4a654686def9d8652256028fef0f3d8a5b76cffc`; `WheelDriveLoadTest.reds` SHA-256 `86f2a9185db9a1609cd7ae76862bcad02526f0f3a693ace730ae4ac0bbc77a44`. Staged in `out/radio-test/`. Not run in the game.

- **Look behind:** the game's `VehicleInverseCameraToggle`, on the right stick press (SRC-032). It may be a toggle.
- **Radio:** on/off, next and previous station, and volume up and down, called by the game script once per press while driving. They use `ToggleRadioReceiver`, `NextRadioReceiverStation`, `SetRadioReceiverStation` and the `CarRadioVolume` setting. The names were checked only by compiling, each action logs what it did, and a text listing of names in the game's script file was used to find candidates.
- **Previous station** does not wrap at the first station, because no station count was found.
- **Volume** changes the game's own Car Radio Volume setting, the same one in its sound settings. Whether it persists after quitting is unknown.
- **Sections:** the page now has Driving and Radio sections with title bands, and shorter rows so 14 fit.

### UI elements

Using the game's own widgets would look the most native. Public docs (SRC-043) confirm that its UI files live under `base/gameplay/gui` and that a script can spawn items from them. The names of the items in the Settings page are not documented in what was read, and finding them needs a tool that opens the game's files, such as WolvenKit, which is not installed. So this build draws its own elements in the game's colours.

Checks: lints clean, 143 of 143 tests passed, script compiled against a copy of the game's `final.redscripts`.

## Next step

With the user's approval, install. The user binds the requested set and tests it in a car.

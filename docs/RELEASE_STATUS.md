# Release status

Date: 2026-11-05. Version 0.21.0, on branch `main`.

**Night Wheel isn't ready for release.** The code is ready for review. It is missing features the product requirements call for, and most of the required lifecycle tests have no recorded result. A public preview would need a decision to reduce the requirements for that preview.

"User report" below means one person, one G29 and one PC. Automated tests don't prove game or hardware behaviour.

## Features

| Feature | Status | Evidence |
|---|---|---|
| Steering and pedals | Works | User report; `docs/decisions/D20-steering-rate-test.md` |
| One-press binding on an in-game page | Works | User report; `docs/decisions/D22-settings-page-input.md` |
| Boost | Works, but has no duration limit or cooldown | User report; `docs/decisions/D23-boost-test.md` |
| Flight with Climb and Descend | Works, and passes through buildings | User report; `docs/decisions/D25-flight-test.md` |
| Vehicle function bindings | Mostly work | User report; horn only worked once per car entry |
| Radio controls | Work | User report |
| Camera controls | Not confirmed | No report recorded |
| Autodrive | Not confirmed | The script's check hasn't been confirmed in play |
| Calibration | Works in a separate app only | User report covers its wheel reading; the full flow isn't recorded |
| Steering speed choice | Text file only | Default is 10. The user's file is 2, and no final value is chosen |
| Force feedback | **Not built** | Research only: `docs/research/P2-force-feedback.md` |
| Wheel LEDs | **Not built** | Route undecided |
| Manual and Auto transmission | **Not built** | Feasibility not studied |
| Response settings on the in-game page | **Not built** | The separate app has them |
| Profiles | **Not built** | One settings file |

## Known problems

- **Esc closes the whole pause menu** from the settings page. Period goes back. A fix was tried, and its result was never reported.
- **Whether the map fix worked** was never reported. The wheel is gated in menus because it was making the map toggle between gamepad and keyboard.
- **Flight has no check for buildings ahead**, and speed doesn't carry over when it ends, so the car stops abruptly. Over ground the rays can't see, Descend can't end flight; leaving the car does. All its strengths are provisional.
- **The wheel can change mode.** It once re-appeared as `PID_C260` and couldn't be read until G HUB was opened. The trigger is unknown.
- **Buttons 13 to 19** on the wheel were never identified.

## What blocks a release

Release requirements:

1. Build or formally defer force feedback, LEDs and the transmission.
2. Give boost bounded duration and cooldown.
3. Record lifecycle tests: save and reload during play, pause, Alt-Tab, wheel disconnect, rebinding, leaving the car and shutdown.
4. Add a game version gate. Nothing yet stops the plugin on an untested patch.
5. Detect conflicting wheel mods and explain them.
6. Add an opt-in diagnostic summary for support.
7. Review the TweakXL dependency formally, because it has no decision record of its own.
8. Include the full license texts for bundled Rust libraries. `THIRD_PARTY_NOTICES.md` lists them but doesn't contain the texts.
9. Check the name on Nexus Mods and for trademarks, and review the platform's publishing policy at release time.
10. Test the package on a clean install.

## Review notes

- **Start with:** `crates/wd_game/game/NightWheel.reds`, then `crates/wd_game/src/xinput_hook.rs`, `shared.rs` and `drive.rs`.
- **Latency:** nothing between a wheel report and the game's gamepad poll takes a lock, allocates or writes a file. Measurement stays in because the user requires latency to be measured, and it logs one summary line a second.
- **Pause detection** depends on the script's event names, in `pause_after` in `lib.rs`. A dedicated call would be cleaner.
- **Game script names:** teleporting, rays and physics pushes were found by compiling against the game's own scripts, and their game behaviour is known only from the test runs in `docs/decisions/`.
- **Collision groups:** the names `Static` and `Terrain` come from the game's script file. The test log showed both finding ground.

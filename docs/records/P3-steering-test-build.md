# Test P3-STEER-BUILD: wheel input through XInput polls, built but not run

Date: 2026-11-04
Result: PASS for build and automated checks. Not run in the game.
Evidence type: AUTO
Source checkpoint and hashes: branch `game-load-test`; release `wd_game.dll` SHA-256 `134f8045a21f56a38ebb8a414d8da1e9ecf6001d4617c57df199bacb3aedf4e3`; `WheelDriveLoadTest.reds` SHA-256 `dbf2eac81ea359f23c5858e6ea77cebaeec474161f442211dd7c4380bccdd9d6`; `Cargo.lock` SHA-256 `7259ffdc727b80016f4d6b11d403f02e7f15f7c4a5b0a6d6a248eee0cdd4b0e9`; debug `wd_setup.exe` SHA-256 `a8a0cb24ac034d7764f183c6943df5ef8331f9e367c0c75cdc054cbaab3da226`
Decision: D19, approved by the user for building this test.

## What it does

- Hooks `XInputGetState` and `XInputGetCapabilities` for controller slot 0, following Microsoft's documentation (SRC-034).
- Fills steering (left stick X), throttle (right trigger) and brake (left trigger) from `%LOCALAPPDATA%\WheelDrive\profile.cfg`, computing them when the game polls.
- The axis mapping comes only from calibration. **With no usable settings file, the wheel never drives.** No such file exists yet, so the user must run Calibrate first.
- Does not touch vibration and sends nothing to the wheel.

## Safety rules, covered by unit tests

The same stick and triggers walk and fire on foot, so the wheel applies only when all of these are true:

- The wheel is connected.
- The script reports the player is the driver.
- The game is not paused.
- The script reported within the last 500 ms. That limit is provisional, set above stalls of up to about 312 ms seen so far.

Otherwise the game receives its normal input, or neutral values if no real gamepad is present.

A control with no reading since the wheel connected counts as neutral. Review before any run found that the wheel reports nothing until moved, and a default raw 0 could have read as a fully pressed pedal. Readings are cleared when the wheel disconnects. A real gamepad still works when the wheel's control is at rest.

## Checks

- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: 107 of 107 passed.
- `redscript-cli` compiled the script against a copy of the game's `final.redscripts`, which confirms that the driver check function exists. Its meaning is only shown by the game run.
- A source search found no device-write, input-injection or `XInputSetState` use.
- Files staged in the ignored folder `out/steering-test/`.

## What the game run must show

| Question | Evidence |
|---|---|
| Does the game call `XInputGetState`? | `game polled XInputGetState`; `poll_interval` |
| Does the driver check work? | `driving-on` and `driving-off` at the right moments |
| Does the car respond correctly? | User report: direction, throttle, brake |
| Does V stay unaffected on foot? | User report |
| Does a pause or wheel loss release control? | Log and user report |
| How fresh is the value at a poll? | `poll_age`, with its window limit |

## Limitations

- The hook, driver check and steering have not run. Any of them may fail.
- Timings exclude USB and driver time before the report, and time from poll to screen.
- XInput limits, game deadzones and steering filters still apply.
- The game may show gamepad prompts, or react when the virtual gamepad disappears on wheel loss.
- Buttons, transmission, boost and force feedback are not included.
- The wheel may re-appear as `PID_C260` again, which stops wheel input until G HUB restores it.

## Calibration, 2026-11-04 20:54

The user completed Calibrate in the setup app, the first real use of its clicks and the wizard. It saved these settings:

| Control | Axis | Range |
|---|---|---|
| Steering | `0001:0030` (X) | Left 0, centre 33078, right 65535 |
| Throttle | `0001:0032` (Z) | Rest 255, full 0 |
| Brake | `0001:0035` (Rz) | Rest 255, full 0 |

Both pedals read 0 when fully pressed, which confirms the raw-default risk fixed above was real. Those axis identities come from the user following the wizard's prompts. Whether each one matches the physical control is confirmed by the game run.

## Game run, 2026-11-04

Files replaced and hash-checked with user approval. G HUB open. Log read while the game was still running, so the run's end, shutdown and final save state are not covered.

| Question | Result | Evidence |
|---|---|---|
| Does the game use `XInputGetState`? | **Yes** | First poll 6 s after load; hooks attached |
| Do steering and pedals work in the car? | **Yes** | User report |
| Does the driver check work? | **Yes, by log** | `driving-on` and `driving-off` logged; 0 seconds with a stale script while driving |
| Does it feel right? | **No** | User: laggy; the car's wheel turns more slowly than the physical wheel; some input delay when turning |
| Pause and on-foot behaviour | Not yet reported by the user | 0 applied polls in paused seconds. 39 applied polls in seconds logged as not driving, which fits the seconds around a transition because the state is sampled once per second, but this is not proven |

Timing during 194 seconds of driving:

| Stage | Measured |
|---|---|
| Decoding | Typically at most 2 to 4 microseconds; worst 937 microseconds |
| Value age at poll while moving | Commonly at most 2 ms; capped by the 50 ms counting window, and inflated by slow movement |
| Game's XInput polling | Very steady: median 62 per second, about 16 ms apart; worst gap under 20 ms |
| Script updates | Often about 77 per second, independent of the poll rate |

Final check after the game closed: clean shutdown, no wheel disconnect, and no new crash report. The user confirmed the wheel did not affect V on foot.

**One save changed.** `AutoSave-1` (`sav.dat`, `metadata.9.json`, `screenshot.png`) was rewritten during the run, along with `user.gls`. This fits the game's own autosave, but it means an autosave was written with the mod loaded, which is what the rule against modifying saves is meant to watch. The earlier copy is in `out/game-before/saves/AutoSave-1`. Later runs should avoid driving long enough to autosave, or expect this. Whether that save is now marked as modded has not been checked.

One paused second logged applied polls. Applied polls are counted per second and the state is sampled at the second's end, so this fits the second in which pause began, but it is not proven.

### Reading of the results

The poll rate stayed near 62 per second while script updates ran faster, so the game appears to poll XInput on a fixed cycle of about 16 ms. That is the largest wait measured up to the point where the value enters the game: at most about 16 ms plus report spacing.

The car's wheel turning more slowly than the physical wheel suggests a limit after the input, such as a steering rate limit or filter tuned for gamepads, or the `Preset_Vehicle` deadzone curve. **This is a hypothesis.** No measurement yet covers what the game does after the poll. `VehicleSteeringSettings_Record` exists in the SDK's declarations, with its fields not yet examined.

## Next step

Examine the game's steering settings read-only, and measure input-to-steering response in the game.

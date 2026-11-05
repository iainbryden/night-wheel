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

## Next step

Files replaced and hash-checked with user approval. The user runs the game test.

# Decision D19: how wheel input reaches the game

Status: accepted by the user on 2026-11-04 for building the test. Nothing here has been tested in the game, and the route can still change on the results.
Date: 2026-11-04
Requirements affected: steering, throttle, brake and button input. User requirement: lowest possible latency.

## Baseline

Measured in the game: plugin decoding at most a few microseconds, wheel reports 2 to 2.6 ms apart, and game script updates about 13 to 22 ms apart. The aim is to add no wait of our own and to have the newest value ready at the moment the game asks for it.

## Observations

Read-only, from the installed game and from public declarations:

- `r6\config\inputUserMappings.xml` maps driving to named actions. `LeftX_Axis_Vehicle` takes `IK_Pad_LeftAxisX` and keys, `Acceleration_Axis` takes `IK_Pad_RightTrigger` and keys, and `Deceleration_Axis` takes `IK_Pad_LeftTrigger` and keys. No wheel or joystick key name appeared in what was checked.
- `r6\config\inputDeadzones.xml` has a `Preset_Vehicle` deadzone.
- The executable declares a dependency on `XINPUT9_1_0.dll`. Only dependency names were listed; no code was disassembled.
- RED4ext.SDK declares the game's input system classes as opaque, with no named fields. Its vehicle headers show no steering input field in what was searched.
- Codeware declares vehicle `acceleration` and `deceleration` fields and no steering field. Whether writing them has any effect is unknown.
- `red4ext-rs` exposes the SDK's hook attachment.

## Alternatives

| Route | Latency | Patch risk | Other costs |
|---|---|---|---|
| **Answer the game's XInput poll from the plugin** | Newest value at each poll, with no driver or OS hop | Low: `XInputGetState` is a public Windows function, not a game internal | Hook in the game process; gamepad limits and game steering filter still apply; game may switch to gamepad button prompts |
| Virtual gamepad driver | Adds a driver and OS path | Low | Requires a driver install; another dependency; same gamepad limits |
| Inject into the game's input system | Potentially best fidelity | High | Input classes are opaque, so it would need undocumented internals and reverse engineering, which the User Agreement restricts |
| Write vehicle state each frame | Unknown; may lose to the game's own update | High | Steering not exposed; write effect unknown; quarantined assumptions Q2 and Q3 apply |
| Edit input mapping files | No gain | Medium | Changes game files; no wheel key names found |

## Proposal

Test the XInput route first. The plugin hooks `XInputGetState` in the game process and fills in steering, throttle and brake from the shared wheel values, computed when the game polls. Everything stays read-only toward the wheel, and no force feedback is sent.

## Risks and unknowns

- **Unproven:** that the game gets its gamepad state through `XInputGetState`. The dependency suggests it, nothing more. The test logs whether and how often it is called.
- The game's poll rate decides the real latency. The test measures it.
- Vehicle deadzone and steering filter sit after this point. They may limit precision, and need measuring against a calibrated wheel.
- XInput axes are 16-bit and triggers 8-bit, which is coarser than the wheel for pedals.
- A hook is the same kind of native change as the loader, so the terms note in D18 applies. Publication still needs its own review.
- If the wheel stops reporting, inputs must return to neutral. A real gamepad must still work when the wheel is absent.
- Transmission and boost are not addressed by this route.
- Microsoft's XInput pages must be read before building (SRC-034).

## Validation

Log hook calls and poll interval, confirm the car steers and pedals work, measure value age at poll, and confirm neutral on wheel loss.

## Reversal

The hook is one module in `wd_game`. Other routes can replace it behind the same shared wheel values.

# P2 research: force feedback and LED routes for the G29

Date: 2026-11-05
Status: documents only. No force was sent, no SDK code was linked or run, and nothing here has been tested on the wheel.

## What the game gives us

The game's executable declares `XINPUT9_1_0`, `HID`, `USER32` and `WINMM`, and not DirectInput (SRC-033). That is a name listing, so it suggests only that the game would not compete for exclusive DirectInput access.

No force signal from the game has been found. The game has no wheel support, and no tire force is known to be exposed to scripts. So forces must be computed by this mod from what it can read. Proven readable: speed, heading and the steering action. Unknown: collisions, surface, airborne state and engine speed.

The game may also send gamepad vibration through `XInputSetState`. The plugin does not look at this today. Whether the game calls it while driving, and what it signals, is an open question that can be observed with no force on the wheel.

## Route comparison

| Route | Forces | LEDs | Terms and shipping | Concerns |
|---|---|---|---|---|
| **DirectInput directly** | Constant, spring, damper, friction, inertia, ramp, periodic effects (SRC-036) | Not documented | Part of Windows. Already declared in the `windows` crate this project uses | Docs are archived. Needs exclusive access and a top-level window owned by the process. Another program can take access away |
| **Logitech Steering Wheel SDK 8.75.30** | Ready-made spring, damper, constant, road and collision effects (SRC-037) | `LogiPlayLeds`, driven by RPM values | Licence grants install and use, reserves all other rights, and states no right to redistribute. The manual is marked Confidential | Dated 2015 and lists Windows XP to 8. Needs a call every frame and finds its window through the foreground. The package has no DLL. One client only |
| **Windows.Gaming.Input** | Condition, constant, periodic, ramp, with master gain, pause, stop and reset (SRC-008, SRC-038) | Not documented | Part of Windows | Written for UWP. Desktop use, background behaviour and exit behaviour are not stated in what was read |
| **GameInput** | Force feedback on the wheel axis for HID devices (SRC-039) | Not seen | Not reviewed | A focus policy exists, and its effect on background force feedback was not found. Least known route |
| **Writing the wheel's own output reports** | Would need the private protocol | Would need the private protocol | No | The 17-byte output report exists, but its format is not documented by a source this project can use. Rejected |

## Proposal, not yet a decision

1. **Forces: DirectInput directly**, tried first in a standalone test. It has no licence or shipping question, needs nothing added to the project, and lets the mod's own thread update a force without waiting for a frame call, which suits the latency requirement. Its actual delay is unmeasured.
2. **LEDs: undecided.** The SDK is the only documented LED route found. Its licence and missing DLL need resolving first. `LogiPlayLedsDInput` takes an RPM range, so a speed bar could pass speed in place of RPM, but the requirement for a steady final cue conflicts with its documented flashing above redline.
3. **Keep Windows.Gaming.Input as the fallback** if DirectInput cannot hold the wheel while the game is focused.

## What the forces could be

These are candidates to test, not chosen values.

| Effect | Based on | Proven input? |
|---|---|---|
| Centering that strengthens with speed | Speed | Yes |
| Damping, heavier when slow | Speed | Yes |
| Pull from cornering | Heading rate and speed | Yes, but only an approximation of tire force |
| Impact jolt | Sudden speed change | Inferred from speed; not tested |
| Road and impact vibration | The game's vibration signal, if it sends one | No |
| Soft stops at the lock | Wheel position | Yes |

## Safety

From the documents:

- Stopping: `DISFFC_STOPALL` stops playing effects, `DISFFC_RESET` removes them and disables the actuators, and `DISFFC_SETACTUATORSOFF` mutes them (SRC-040). These need exclusive access.
- Unacquiring unloads effects (SRC-040), so losing the device should remove forces. This needs confirming on the wheel.

Not stated in anything read, so it must be tested:

- **What the wheel does if the game crashes while a force is playing.** This is the main safety question. A candidate design is to give every effect a short finite duration and keep renewing it, so a force ends by itself if the process dies. Whether the G29 and driver honour this is untested.
- How forces interact with G HUB's centering spring settings.
- Whether exclusive access can be taken and held from a plugin inside the game.

Force ownership follows the plan: magnitude and rate limits, one owner, a visible Stop, and forces cleared on pause, exit, focus loss, disconnect and shutdown.

## Next steps, in order

1. Read-only and without force: enumerate the G29 through DirectInput and report what it says it supports.
2. Observe, with no force, whether the game calls `XInputSetState` while driving.
3. With the user's specific approval, a standalone test at low force with the wheel clear, a Stop control and a stated procedure. It covers direction, stop, disconnect and killing the process.
4. Measure the delay from a change in force to the wheel moving, if a method can be found without extra hardware. Otherwise record it as unmeasured.
5. Only then, try forces in the game.

## Limits of this research

- Several Microsoft pages are archived DirectX 9 material, and `DIPROP_AUTOCENTER` and `DIPROP_FFGAIN` were not confirmed from them.
- G HUB setting behaviour comes from community guides and not from Logitech (SRC-041).
- A name search of the G HUB folder, limited in depth, found no steering wheel DLL. A deeper search has not been done.
- The official SDK package was saved locally by a page fetch, outside the repository. Only its readme and manual were read. Its demo was not run and its sample code was not opened. Whether to use it is the user's decision.

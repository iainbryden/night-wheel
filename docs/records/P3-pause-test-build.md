# Test P3-PAUSE-BUILD: pause screen and latency test, built but not run

Date: 2026-11-04
Result: PASS for build and automated checks. Not run in the game.
Evidence type: AUTO
Source checkpoint and hashes: branch `game-load-test`; release `wd_game.dll` SHA-256 `194d5644b59bf84f4e034e2d1d5edebe0f0ae11da5454020dfba5d122a3e255d`; `WheelDriveLoadTest.reds` SHA-256 `ddba09faebeaaee0adc78594467cefd19f02b5d2082162b1fc177e11e344eed7`; `Cargo.lock` SHA-256 `0b3c434243f21f5e892e80e57cffef94ae410ae1cdab9d29dcbd7dcbb2ea7204`

## What the test asks

| Question | How it shows |
|---|---|
| Can text be drawn during pause? | A line of text on the menu layer, seen by the user |
| Does it update during pause? | The X number changes as the wheel turns, if the per-frame callback runs during pause |
| Does input work during pause? | The comma and period keys change VALUE on screen |
| How fast does the game read the value? | `read_interval` in the log, in play and paused |
| How old is the value when read? | `value_age` in the log, counted only while the wheel is moving |
| How long does decoding take? | `decode` in the log, from starting to handle a report to publishing the value |

The script's read runs once per frame in the game's delay system, as a stand-in for the later game input path.

## Latency changes

- `wd_device` gains `run_values_only` and `Device::apply_values`, which decode reports with no allocation. Button decoding is not on this path yet.
- Wheel value and timing are shared as atomics, with no locks.
- All file writing moved to a logger thread. The game's thread and the wheel reader no longer open files. Script events and connection messages allocate and queue a message, but are occasional.
- Key presses now only increment a counter.

## Checks

- `cargo fmt`, `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- `cargo test --locked --workspace`: 98 of 98 passed.
- `redscript-cli` compiled the script with Codeware against a copy of the game's `final.redscripts`. The only warning is in Codeware.
- A source search found no device-write or input-injection calls.
- Files staged in the ignored folder `out/pause-test/`.

## Limitations

- **Software timing only.** These numbers do not include USB and driver time before the report reaches the plugin, or the time from the game's read to the frame on screen. Those need other measurement.
- `read_interval` is a callback rate, not a measured frame rate, and the callback may stop or slow during pause. Its behaviour there is one of the questions.
- `value_age` depends on how often the wheel reports, so a value of a few milliseconds may be the report spacing rather than a delay in this code.
- The measurement itself has a small cost, and the per-frame script call is test-only.
- X is taken as steering for this test. Individual controls are still unconfirmed.
- Hidden test text widgets remain on the menu layer until the game closes. A leftover is expected for this test only.
- Whether the text appears depends on `inkMenuLayer` holding the pause menu in 2.5, which comes from community data dated to patch 2.12.

## Game run, 2026-11-04 19:48

Approved by the user; both files replaced and hash-checked. Resolve, CurseForge and Chrome were found closed after the run.

| Question | Result | Evidence |
|---|---|---|
| Can text be drawn during pause? | **Yes** | User saw it; `text-shown` |
| Does it hide on resume? | **Yes** | User report |
| Does keyboard input work during pause? | **Yes** | Comma and period changed VALUE on screen; 50 key presses logged while paused |
| Does the display update live during pause? | **No, with this callback** | Delay callback ran only in the seconds around each transition; about 123 seconds without a read during the long pause |
| Does X change? | **No, with two causes** | See below |
| Crash or changed saves? | None | No new crash report; only `user.gls` changed |

### Why X was frozen

1. The per-frame delay callback largely stops during pause, so nothing refreshed the text. Key presses refreshed it, but with no new value read.
2. **The wheel left the system 16 seconds before the pause.** The log has `controller disconnected` at 19:50:55, with the last report at about 19:50:39 and none afterward. The first test read 13,790 reports during pause, so this was not caused by pausing.

After the run, Windows no longer listed the wheel as `VID_046D PID_C24F`. It listed the same USB port as `VID_046D PID_C260`, "G29 Driving Force Racing Wheel (PS4)" on the `LGHUBWinUSB` driver, and `wd_diag --list` found no game controllers. The Windows Plug and Play log shows the wheel starting as `PID_C260` at 19:50:56.

Follow-up the same evening: the user said G HUB had not been opened for any of these tests. When they opened it at about 19:56, the wheel returned to `PID_C24F`, and `wd_diag --list` read it again. So G HUB can move the wheel into the readable mode, and the wheel can leave that mode with G HUB closed. Why it re-appeared at 19:50:55 is not established. The plugin has no device-write path, but that alone does not rule out an indirect effect. The user reported the wheel's power was on; a PS3/PS4 mode switch on the wheel was only a guess and was not confirmed to exist.

### Timing in play, before the disconnect

| Measure | Result |
|---|---|
| Wheel reports while moving | Average 390 per second, up to 504; about 2 to 2.6 ms apart |
| Decoding, from starting to handle a report to publishing | Typically at most 2 to 4 microseconds; worst 112 microseconds |
| Game reads | Median 70 per second, about 14 ms apart; range 50 to 83 per second from 80 to 140 s; worst gap 312 ms |
| Value age at read | Typically at most 1 to 2 ms; sampled worst 9 to 17 ms |

Reports only arrive when something changes, so slow movement inflates value age. Percentiles are bucket upper bounds. The read rate is a callback rate, used as a stand-in for frame rate. On these numbers, the game's update interval is far larger than the plugin's own decoding time.

### Findings for the product

- Wheel mode can change while the game runs, leaving a device this code cannot read. The mod must detect it and say so, and must release any held inputs.
- As observed, reading the wheel may depend on G HUB having set its mode. Setup instructions and the mod's messages must cover this. Switching mode from this project would require writing to the device, which needs research and specific approval.
- A live in-game calibration screen needs a pause-safe update, not the delay system.

## Script update for the retest

The text is now refreshed by a looping UI animation, using the animation calls Codeware's own buttons use. The looping options were checked only by compiling against the game's scripts. `WheelDriveLoadTest.reds` SHA-256 `bef8054536a9a97aa6547dcd899c634472e529b5aea3248cd4a7cd803bd3df13`. `wd_game.dll` is unchanged. Compiled offline with no warnings from this script; not yet run.

## Next step

Get approval to replace the script, then retest with G HUB open and watch for another mode change.

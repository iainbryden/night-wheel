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

## Next step

Get approval to replace the two test files in the game, then run it with other heavy software closed.

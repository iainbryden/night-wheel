# Test P3-REPORT-BUILD: steering settings and response report, built but not run

Date: 2026-11-04
Result: PASS for build and automated checks. Not run in the game.
Evidence type: AUTO
Source checkpoint and hashes: branch `game-load-test`; release `wd_game.dll` SHA-256 `bf97dc4e4568643685cca36ec62a4900fb795936e61ab5b0e4329eb4e245a5f9`; `WheelDriveLoadTest.reds` SHA-256 `d08989f117cdc5dd0d028b2d2e84f019d0d8d5ee3da183359e4052dd17c39cdf`; `Cargo.lock` SHA-256 `7259ffdc727b80016f4d6b11d403f02e7f15f7c4a5b0a6d6a248eee0cdd4b0e9`

## What it adds

- **Stick-to-action table:** what each range of our stick value becomes in the game's `TurnX` action, written at shutdown. This shows the effect of the game's deadzones.
- **`action_delay`:** time from our stick value changing to the next `TurnX` action report.
- **Settings report, once per launch on first driving:**
  - functions declared on the steering settings and drive model record types
  - Float, Int32 and Bool properties on the vehicle
  - values from no-argument getters returning those types on the vehicle's drive model and steering records

## Method and boundary

- Uses Codeware's documented reflection interface and the game's input listener. Nothing is disassembled or patched.
- It sets no properties and calls only no-argument getters on game data records. Driving behaviour is unchanged from the previous build.
- `gamedata*` names come from RED4ext.SDK declarations. `RegisterInputListener`, `ListenerAction` and `GetMountedVehicle` were checked only by compiling against the game's scripts, and their behaviour is confirmed by this run.
- No existing mod was consulted.

## Checks

- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- `cargo test --locked --workspace`: 109 of 109 passed.
- `redscript-cli` compiled the script against a copy of the game's `final.redscripts`.
- Files staged in the ignored folder `out/report-test/`.

## Limitations

- Calling record getters through reflection has not run. The report runs once per launch, on first driving, and a crash there would be easy to place.
- The report may produce nothing useful if the steering rate is not exposed to scripts.
- `action_delay` covers only the path to the script's action report, and that may not be the point the vehicle uses. The time from `TurnX` to the car's wheels is not measured by this build, unless the report reveals a value to follow.
- The table keeps only the latest value for each range, so the wheel must be swept slowly across its range.
- A game autosave can still occur during the run.

## Game run, 2026-11-04

Approved by the user; saves backed up to `out/game-before/saves-before-report`, files replaced and hash-checked. No crash, clean exit, no save changed; only `user.gls` changed. The full report is kept locally at `out/game-before/report-dump.txt`.

### Stick value to game action

| Stick position | `TurnX` |
|---|---|
| 0.19 to 0.38 | 0.000 |
| 0.48 | 0.153 |
| 0.57 | 0.338 |
| 0.67 | 0.499 |
| 0.76 | 0.667 |
| 0.86 | 0.877 |
| 0.95 | 1.000 |

The negative side is close to a mirror of this. Each row is the latest value seen in a range about 0.095 wide. A straight line through these points leaves zero at about 0.40 and reaches full at about 0.93.

**So the first 40% or so of stick travel does nothing in each direction.** That sits alongside the `Preset_Vehicle` deadzone entry in `inputDeadzones.xml` (`bCoeff` 0.4), although that link is inferred. With a wheel mapped straight across, this is a dead band around centre, then a steeper response. It fits the user's report of delayed response when turning.

### Delay

Stick value change to next `TurnX` report: commonly at most 8 ms, nearly always at most 16 ms, worst 28 ms, over 1,313 changes. This fits one polling cycle, with no large delay in this part of the path. It does not cover the path from `TurnX` to the car's wheels.

### Vehicle drive settings, for the vehicle driven

| Setting | Value |
|---|---|
| `MaxWheelTurnDeg` | 50 |
| `WheelTurnMaxAddPerSecond` | 95 |
| `WheelTurnMaxSubPerSecond` | 128 |
| `UseAlternativeTurnUpdate` | true |
| `TurnUpdateInputSlowChangeSpeed` / `FastChangeSpeed` | 0.1 / 1.0 |
| `TurnUpdateInputDiffForSlowChange` / `ForFastChange` | 0 / 1 |
| `TurnUpdateInputDiffProgressionPow` | 0.8 |
| `TurnUpdateBaseSpeedThreshold` / `Mid` / `Max` | 10 / 0 / 25 |
| `TurnUpdateMidSpeedTurnMul` / `MaxSpeedTurnMul` | 0.5 / 0.45 |
| `TurnUpdateMidSpeedTurnChangeMul` / `MaxSpeedTurnChangeMul` | 1.25 / 2.7 |

These names suggest the game limits how fast the wheels turn, and scales turning with speed. If the units are degrees and seconds, 95 per second over 50 degrees would take about half a second to reach full lock. **Units and meaning are inferred from names, and their effect has not been tested.** Other vehicles may differ. Limits like these could explain why the car's wheel lags the physical wheel, and could remain after the deadzone fix.

## Fix built: deadzone compensation

`drive.rs` now maps wheel position past the dead range: zero stays exactly zero, other values start at 0.40 of stick travel and rise to 0.93, and full lock sends full stick. It adds one small calculation at poll time and changes no game file.

Plugin 0.5.0 `wd_game.dll` SHA-256 is recorded in `evidence/TEST_RESULTS.csv`. The script is unchanged. The stick-to-action table now has 41 ranges, to check the result.

The two constants are measured on one run and one vehicle, and depend on the game's settings. They need rechecking after patches.

## Next step

Get approval to replace the plugin, then test how steering feels and read the new table. If steering still lags, consider adjusting the rate settings in game data as a separate decision.

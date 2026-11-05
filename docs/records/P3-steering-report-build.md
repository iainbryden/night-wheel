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

## Next step

Get approval to replace the two test files. Back up saves again before the run.

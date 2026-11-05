# Test P3-LOAD-BUILD: first game load test, built but not run

Date: 2026-11-04
Result: PASS for build and automated checks. Not run in the game.
Evidence type: AUTO
Source checkpoint and hashes: branch `game-load-test`; release `wd_game.dll` SHA-256 `021a7c233032d4405bb1a2667c863ad42afda14c6da4e3580195982cb7c5326c`; `WheelDriveLoadTest.reds` SHA-256 `2305b4ebe3ccbf7c950ae37472389706c1f730a9fc6b2b6491cb6b3ad4525396`; `Cargo.lock` SHA-256 `0b3c434243f21f5e892e80e57cffef94ae410ae1cdab9d29dcbd7dcbb2ea7204`
Versions: Rust 1.99.1; `red4ext-rs` 0.11.2 at `1ea91e760c0626245449f21e184e9f860b943c48`; CMake 4.4.5; LLVM 23.1.5; MSVC 14.29

## What the test does

- The plugin writes to `%LOCALAPPDATA%\WheelDrive\load-test.log` and once to the loader's log.
- It reads the wheel on a background thread and logs a report count once a second, with the pause state.
- The script calls the plugin once at load, then reports session ready, pause, resume, end and key presses. It never records which key.
- It sends nothing to the wheel, calls no game function and changes no game state.

## Checks

- `cargo fmt --check` and `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- `cargo test --locked --workspace`: 94 of 94 passed, including 7 new in `wd_game`.
- The DLL exports `Main`, `Query` and `Supports`.
- A source search found no device-write or input-injection calls.
- Files staged in the ignored folder `out/load-test/`, laid out like the game folder.

## Defects found

`bindgen` failed to compile: Cargo paired it with `syn` 2 and `prettyplease` 0.3, which uses `syn` 3. `prettyplease` is locked to 0.2.37, which `bindgen` accepts and which uses `syn` 2. Upstream's lock file uses `prettyplease` 0.3 with `syn` 3, so our build differs from theirs here. `prettyplease` only formats generated code.

## Limitations

- **The script has not been compiled.** redscript compiles it when the game starts, so a mistake appears only then.
- **Nothing shows that the DLL loads.** Upstream says linking does not establish runtime safety.
- A full debug workspace build could not replace `wd_setup.exe` because the app was open. Its source did not change, and lints and tests covered it.
- The plugin uses the library's default runtime-independent setting, which does not guarantee compatibility across game updates. It relies on no game-version-specific access beyond function registration.
- `wd_device` registers for gamepad and joystick Raw Input inside the game process. A process has one target per usage, so this could displace any such registration the game makes. The run must check that keyboard and any gamepad still work.
- There is no in-game screen yet. This test only covers loading, script calls, pause events and input arrival. The screen is the next test.
- `Session/Pause` may not correspond to opening the pause menu. The Codeware wiki describes it only as fired when the game is paused.

## Deployment plan, needing approval

1. Record the exact game build and list the game's `red4ext`, `r6\scripts`, `r6\cache` and `bin\x64` contents before any change. Back up anything that gets replaced.
2. Download RED4ext v1.31.0, redscript v0.5.32 and Codeware v1.21.2 from their GitHub releases, record hashes, and list what each archive adds.
3. Copy those and `out/load-test/` into the game folder, recording every added file.
4. The user starts the game, loads a save or starts a new game, avoids saving, opens the pause menu, turns the wheel, presses keys, resumes and quits.
5. Read `load-test.log` and the loader logs.
6. Remove every added file and restore anything replaced, unless the user wants to keep the loader for the next test. Compare against the step 1 listing.

## Game run, 2026-11-04

Approved by the user. 19 files were added and none overwritten. Game: patch 2.5, Steam build 26161058; RED4ext reported product 2.50, file version 3.0.88.46235. Two launches appear in the logs, at 19:27 and 19:33.

| Question | Result | Evidence |
|---|---|---|
| Does the plugin load? | Yes, both launches | Loader log; `plugin loaded` |
| Does the script compile? | Yes, on the game's bundle offline and in the game | `redscript-cli`; loader log |
| Can a script call the plugin? | Yes | `script ping 41, reply 42` |
| Do pause events arrive? | Yes, once per launch | `session-pause`, then `session-resume`, on one menu visit each |
| Are wheel reports read during pause? | Yes | 13,790 reports over 35 paused seconds |
| Do key presses arrive during pause? | Yes | 14 presses logged while paused |
| Clean shutdown? | Yes, first launch | Plugin and loader unload logged in order |
| Errors? | None reported by the user | No new crash report folder |

Limits:

- The user reported the game felt somewhat laggy, cause unknown. No frame rate was measured, with or without the mod, so this neither implicates nor clears it.
- `Session/Pause` was seen to line up with the user's pause steps by order only.
- Keyboard and gamepad behaviour in play was not separately confirmed by the user. Key events reached the script.
- Which keys or controls were used was not recorded, by design.
- No game save changed at the first check, while the second launch was still running. `user.gls` in the saves folder changed. Recheck after the game closes.
- Rollback has not been run.

## Next step

Decide whether to keep the loader for the pause-screen test, and compare lag with the mod files removed.

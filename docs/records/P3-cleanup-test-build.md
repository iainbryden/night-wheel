# Test P3-CLEANUP-BUILD: steering clean-up and pause menu entry, built but not run

Date: 2026-11-05
Result: PASS for build and automated checks. Not run in the game.
Evidence type: AUTO
Source checkpoint and hashes: branch `game-load-test`; release `wd_game.dll` SHA-256 `de5b3137472b112574d933fc58308d39f896439f62a4eea4758d3aa68cca65e8`; `WheelDriveLoadTest.reds` SHA-256 `ed94a75863ffaa4351f08ae7f8c745584aa727939de6ed34bb2b3fda96193b34`; `Cargo.lock` SHA-256 `7259ffdc727b80016f4d6b11d403f02e7f15f7c4a5b0a6d6a248eee0cdd4b0e9`

## What changed

- **Per-frame response samples removed**, along with the settings report and the speed level code. Each frame the script now only reports whether the player is driving. The script's steering action listener and the once-per-second stats stay, and use atomics and a logger thread.
- **Dead spot correction refitted** from 0.40 and 0.93 to 0.35 and 0.90.
- **Steering speed multiplier from a file.** `%LOCALAPPDATA%\WheelDrive\steering-speed.txt` holds one number from 1 to 10. A missing or invalid file gives 10, the only value proven to keep up. It applies at game start, so changing it needs a restart.
- **Pause menu test.** A WHEEL entry is added after the game's own entries. Choosing it opens a plain panel with the live steering value and the multiplier. The period key is a fallback way to open and close the panel, and a line at the top of the pause screen says so.

## Dead spot data

From about 20 minutes of driving in the start-up run, using 7,773 samples where the stick had held still for at least 40 ms, with a straight line fitted to medians in steps of 0.025 of stick travel:

| Side | Action leaves zero at | Action reaches full at |
|---|---|---|
| Right | 0.347 | 0.902 |
| Left | 0.348 | 0.899 |

The earlier compensation never sent stick values between zero and 0.40, so the zero point is extrapolated. The fit covers one drive at the steering speed test values, and mostly one vehicle.

## Method and boundary

`PauseMenuGameController`, `PopulateMenuItemList`, `AddMenuItem`, `OnMenuItemActivated`, `PauseMenuListItemData` and `m_menuListController` were checked only by compiling against a copy of the game's scripts. A made-up method name was refused by the same compiler, so the check has meaning. Compiling shows the names and types exist, and not what they do at run time. The game's scripts were not decompiled, and no existing mod was consulted.

## Checks

- `cargo clippy --locked --workspace --all-targets -- -D warnings`: clean.
- `cargo test --locked --workspace`: 114 of 114 passed.
- `redscript-cli` compiled the script against a copy of the game's `final.redscripts`.
- Files staged in the ignored folder `out/cleanup-test/`.

## Limitations

- Nothing here has run in the game.
- The panel doesn't take over input, so the pause menu may still react underneath it, and Escape is expected to leave the pause menu. Solving that is part of the real screen.
- The entry's label is plain English text and is not translated.
- `OnMenuItemActivated` is wrapped, so a change there in a game update could affect the pause menu. Only the WHEEL entry is intercepted.

## Next step

With the user's approval, install and run with the multiplier at 4. The user reports whether the WHEEL entry appears and opens the panel, whether the pause menu's other entries still work, and whether steering still keeps up.

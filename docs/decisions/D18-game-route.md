# Decision D18: game loading and in-game settings route

Status: proposed, awaiting the user. Nothing here has been tested in the game.
Date: 2026-11-04
Requirements affected: R12 (settings during pause) and everything that runs in the game.

## Problem

The mod needs native code in the game to read the wheel and later drive force feedback. It also needs a settings and calibration screen that works during pause.

## Findings

- **Patch 2.5 mod menu:** the official notes describe browsing and installing mods through mod.io from the in-game menu. The official modding page adds an enable checkbox. Neither mentions per-mod settings, so no official settings route has been found.
- **REDmod:** the official documentation covers scripts, tweaks, archives, sound and animation. It does not mention native code or input devices. That documentation is dated 2022.
- **Pause behaviour:** no document read says whether game UI drawing or input works during pause, for any option. Only a test can settle it.

## Loading alternatives

| Option | Native code | Licence | Patch 2.5 | Verdict |
|---|---|---|---|---|
| REDmod only | Not documented | REDmod EULA | Official | Cannot be the whole route for wheel input or force feedback |
| RED4ext plugin | Yes, a DLL with a documented load sequence | MIT | v1.31.0 sets 2.5 as its minimum | Proposed |
| CET Lua only | Not a plugin route | GPL-3.0 with a Lua mod exception | Not stated in what was read | Rejected |

Rust or C++ inside a RED4ext plugin:

| Option | For | Against |
|---|---|---|
| `red4ext-rs` | Matches D16; `wd_core` and `wd_device` could be reused directly; MIT | Needs CMake 3.21 or newer and libclang, neither present here (bundled CMake is 3.20); no stated game version support; its smoke test checks linking only |
| C++ with RED4ext.SDK | The loader's own documented route | Adds a second language and a boundary to the Rust code |

Proposed: `red4ext-rs`, with C++ as the fallback if the load test fails because of the wrapper.

## In-game screen alternatives

| Option | For | Against |
|---|---|---|
| Game UI (ink) through redscript and Codeware | Looks like the game; MIT; Codeware lists 2.5 and documents new widgets, a UI framework, and pause and input events | Two more user installs; pause drawing and input undocumented; Codeware wiki is at 1.18.0, behind release 1.21.2; its third-party licence file is unread |
| redscript alone | One fewer dependency | Reviewed documentation only shows hiding and showing existing widgets, on a page marked deprecated |
| Own overlay drawn by the plugin | No script framework; could reuse `egui` | No approved documentation for hooking the renderer; highest crash and patch risk |
| CET overlay | Provides ImGui windows | GPL exception is written for Lua mods and does not clearly cover a native plugin; pause behaviour undocumented; another install |
| Existing shared settings mod | Not applicable | Barred: settings must be implemented in this product |
| Setup app only | Already built | Does not meet R12 |

Proposed order: game UI through redscript and Codeware; then an own overlay only if that fails the pause test.

## Terms

Factual reading, not legal advice. Publication should get its own review.

- The Fan Content Guidelines allow mods that do not breach the game's terms, and name overlay software as an acceptable example. They require non-commercial use and an "unofficial" notice.
- The User Agreement (in effect 2026-09-09) restricts reverse engineering and code injection, and has a broad definition of cheats covering software that affects the game. It wins over the guidelines if they conflict, and has no express exception for mods or for loaders.
- No document read addresses third-party loaders or native mods. Points 72 to 130 of the User Agreement were not read verbatim.
- This matters mainly for publication. Whether a loader-based mod can be listed on mod.io is unknown.

## Validation: first game test

Smallest test, one question at a time:

1. The game starts with the loader, and the plugin's load is logged.
2. A script calls a native function in the plugin.
3. A minimal screen appears during pause.
4. That screen takes keyboard or controller input during pause.
5. The plugin reads the wheel during pause, with no motor output.
6. Removing every added file returns the game to its starting state.

Record the exact game build first. No save is modified, no existing mod is consulted, and a new game or throwaway save is used if one is needed. Deployment needs the user's approval at the time.

## Reversal

The plugin and script layer can be replaced without changing `wd_core`. All game-side files are additions that can be listed and removed.

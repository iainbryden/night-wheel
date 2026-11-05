# Decision D21: all player settings live in the mod's own in-game screen

Status: accepted by the user on 2026-11-05 for where settings live. A pause menu entry and a panel opened from it worked in a test on 2026-11-05; see `docs/records/P3-cleanup-test-build.md`. Input handling for a real screen is unproven.
Date: 2026-11-05
Requirements affected: R07, R08, R12.

## Problem

Players need to calibrate and bind controls. The user wants this done in the game, not in a separate app. The question is where in the game it belongs.

## Alternatives

| Place | Finding | Result |
|---|---|---|
| The game's MODS menu | The official 2.5 notes describe browsing and installing mods. Neither they nor the modding page mention per-mod settings. No way for a mod to add a screen here is known | Not usable unless evidence appears |
| A MOD SETTINGS section | Not mentioned in the official notes. A search of the install found no files named for a settings mod. Depending on a shared settings mod is excluded by D02 | Rejected as a dependency |
| The game's control settings | They hold the game's own keyboard and gamepad bindings and do not know the wheel's buttons. Changing them would alter the player's normal controls | Rejected. The mod still respects the game's existing bindings |
| The mod's own screen, opened from the pause menu | Text and key input during pause already work. A selectable menu entry and a full interactive screen are unproven | **Chosen** |

## Choice

Build an original screen for the mod, opened during pause. First try an entry in the pause menu. If that can't be done safely, use a key and a wheel button while paused, which the pause test suggests is achievable. The action's label is "Bind".

The setup app stops being the place for player settings. It may remain as a developer tool. D17 is superseded for that purpose.

## G HUB values on the user's PC

Profile "Cyberpunk 2077: Default": sensitivity 24, operating range 240, centering spring in non force feedback games off. The user reports this works for them and may not be a common preference.

The mod must not require these values. Calibration measures the wheel as set, and G HUB's sensitivity is applied before the mod sees the wheel, so tuning done here reflects this setup only.

## Validation

A test build proves a way to open the screen during pause, moves through it with both keyboard and wheel, and shows that binding does not trigger gameplay. Then the checks in P5 apply.

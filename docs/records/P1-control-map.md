# Test P1-CONTROLS: which report number is which G29 control

Date: 2026-11-05
Result: PARTIAL. All listed controls reported, and one pair needs confirming.
Evidence type: USER, GAME
Source checkpoint and hashes: `6e0900a`; `wd_game.dll` SHA-256 `361c8a623f4e3a36174bda6e6449a75ef773f39e411e155aa2bcb9428f955020`; script SHA-256 `a1d7044c711bf1df68395cb615e971392c7f898cec97088bde82d49244476ae8`
Device: `VID_046D PID_C24F` rev 8900 in G HUB's "Cyberpunk 2077: Default" profile. Game 2.5, Steam build 26161058.

## Method

With the panel open in the pause menu, the user pressed 10 left paddle presses as a marker, then each control once in a listed order. The script logged each change in held controls. The log is kept locally at `out/game-before/load-test-run12.log`. Earlier free presses in the same run were not used. Names come from pairing the log's order with the listed order, so they rest on the user having kept to it.

## Map

| Control | Reports as |
|---|---|
| D-pad up, right, down, left | Hat 0, 2, 4, 6 |
| X, Square, Circle, Triangle | Button 1, 2, 3, 4 |
| Right paddle, left paddle | Button 5, 6 |
| R2 | Button 7 |
| L2 | Button 11 (see below) |
| R3 | Button 8 (see below) |
| L3 | Button 12 |
| Share, Options, PS | Button 9, 10, 25 |
| Plus, minus | Button 20, 21 |
| Dial clockwise, counter-clockwise | Button 22, 23 |
| Red Enter | Button 24 |

The left paddle marker logged Button 6 ten times, and matches its place in the list.

## Open points

- **L2 and R3 may be swapped.** The log order was 7, 11, 8, 12. The pairing of numbers suggests the user could have pressed the right side first. The order needs confirming with the user before these names are used.
- **Buttons 13 to 19 were not seen.** The report has 25 buttons, and these seven have no control from this list. An add-on shifter is a possibility, and has not been checked.
- **The dial sends short pulses.** Each dial click appeared as two pulses, and some were logged as lasting about 10 to 100 ms. The shortest is below the game's roughly 16 ms poll and the panel's refresh, so binding must catch presses on the wheel reader's thread rather than by sampling held state, or dial clicks will be missed. Pulses shorter than the panel's refresh may also have gone unlogged.
- The D-pad also reported a diagonal (Hat 7) once during free presses, so diagonals exist and binding must define what they mean.
- G HUB's button assignments for this profile were not checked. They might change what is reported.

## Same run

- **Decode time:** typically at most 8 µs, against at most 4 µs before buttons were read. Worst seen 74 µs. Wheel reports arrive about 2 ms apart, on a thread the game's read doesn't wait for.
- **Stability:** no crash report, plugin unloaded at about 10 minutes, wheel connected throughout.
- **Saves:** the game wrote `AutoSave-15` and `user.gls`. The user has said they do not need recent autosaves.
- **Menus, from the user's screenshots:** WHEEL SETTINGS appears above Quit Game in the pause menu. The main menu doesn't have it. Neither menu shows a MOD SETTINGS entry. The panel's left edge slightly overlaps the pause menu's column.
- **Steering speed x2:** two read-backs show 2, 0.3, 200, 260. About 1.7 minutes were spent driving. The user's report is pending.

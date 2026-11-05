# Test P1-CONTROLS: which report number is which G29 control

Date: 2026-11-05
Result: PASS for the listed controls, on one wheel in one G HUB profile. Buttons 13 to 19 were not seen.
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
| L2 | Button 8 |
| R3 | Button 11 |
| L3 | Button 12 (by elimination) |
| Share, Options, PS | Button 9, 10, 25 |
| Plus, minus | Button 20, 21 |
| Dial clockwise, counter-clockwise | Button 22, 23 |
| Red Enter | Button 24 |

The left paddle marker logged Button 6 ten times, and matches its place in the list.

## Open points

- **Retest of the shoulder buttons.** The first sequence logged 7, 11, 8, 12, which was not in the listed order. In a retest (`out/game-before/load-test-run13.log`), the user read L2 as Button 8 and R3 as Button 11. L3 as Button 12 follows by elimination and was not pressed on its own in the retest.
- **Buttons 13 to 19 were not seen.** The report has 25 buttons, and these seven have no control from this list. An add-on shifter is a possibility, and has not been checked.
- **The dial sends short pulses.** Each dial click appeared as two pulses, and some were logged as lasting about 10 to 100 ms. The shortest is below the game's roughly 16 ms poll and the panel's refresh, so binding must catch presses on the wheel reader's thread rather than by sampling held state, or dial clicks will be missed. Pulses shorter than the panel's refresh may also have gone unlogged.
- The D-pad also reported a diagonal (Hat 7) once during free presses, so diagonals exist and binding must define what they mean.
- G HUB's button assignments for this profile were not checked. They might change what is reported.

## Same run

- **Decode time:** typically at most 8 µs, against at most 4 µs before buttons were read. Worst seen 74 µs. Wheel reports arrive about 2 ms apart, on a thread the game's read doesn't wait for.
- **Stability:** no crash report, plugin unloaded at about 10 minutes, wheel connected throughout.
- **Saves:** the game wrote `AutoSave-15` and `user.gls`. The user has said they do not need recent autosaves.
- **Menus, from the user's screenshots:** WHEEL SETTINGS appears above Quit Game in the pause menu. The main menu doesn't have it. Neither menu shows a MOD SETTINGS entry. The panel's left edge slightly overlaps the pause menu's column.
- **Steering speed x2:** two read-backs show 2, 0.3, 200, 260. About 1.7 minutes were spent driving. In the retest, the user reports the tires keep up and are "properly reactive", with the caveat "at the moment". Tire angle is not measured.
- **Game's own pause entries:** the user reports Load Game and Settings work as expected.
- **Centre:** with the wheel roughly centred within its play, the panel read 34113, against a calibrated centre of 33078. The user reports play in the middle with no force holding the wheel.

## Steering check at the wheel

Read from the panel by the user, by hand, one reading each, with G HUB sensitivity 24 and range 240.

| Position | Value |
|---|---|
| Left edge of the play | 30338 |
| Right edge of the play | 35087 |
| About an inch past the right edge | 45356 |
| About an inch past the left edge | 16453 |

- **The sensor sees the play.** The value spans 4,749 across it, so the play is not lost before the reading, and no dead spot was found at this point in the path.
- **Centre.** The middle of the play is 32712. The calibrated 33078 is inside the play and about 366 away, so the earlier "about 3.8 degrees off" came from where the wheel was resting and overstated the error.
- **Small turns are large, not flattened.** About an inch moved the value by about 10,300 to the right and 13,900 to the left. A straight line over 240 degrees would give about 2,700 for 10 degrees. So G HUB's sensitivity 24 is not softening small turns here, which was the earlier suspicion. The inch is an estimate and may be off, and the shape of the response is not established.
- **The car can steer within the play.** With a dead zone of 0, resting at the play's edges gives an intent of about 0.08 left and 0.06 right, with nothing holding the wheel at centre. An optional small dead zone or force feedback centring may help. R01 already includes a dead zone setting.

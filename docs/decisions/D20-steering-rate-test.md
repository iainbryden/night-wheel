# Decision D20: test the game's steering speed values with TweakXL

Status: test passed by user report on 2026-11-05, applying the values at game start. No setting chosen, and not yet accepted for the product.
Date: 2026-11-04, updated 2026-11-05

## Problem

With the operating range fixed, the user reports that the car takes too long to catch up with a quick wheel turn. The plugin hands over each value within about 16 ms of it changing, so the remaining delay appears to lie after the input. The drive model reports `WheelTurnMaxAddPerSecond` 95, `WheelTurnMaxSubPerSecond` 128, `TurnUpdateInputFastChangeSpeed` 1.0 and `TurnUpdateInputSlowChangeSpeed` 0.1. Their names suggest speed limits. Their meaning and units are inferred.

## Alternatives

| Route | For | Against |
|---|---|---|
| TweakXL `SetFlat` and `UpdateRecord` | Script can change values in memory; supports A/B levels with no file edits; MIT; states 2.5 support | One more install; a dependency to review if kept |
| RED4ext.SDK TweakDB through C++ | No extra install for players | `red4ext-rs` does not wrap it; needs a C++ layer or new bindings; not worth building before the cause is proven |
| REDmod `.tweak` files | Official route | Needs the REDmod DLC, deployment and modded launch; each change needs a redeploy; poor for comparing values |
| Overdrive the stick to outrun the filter | No data change | Needs a model of game internals; risk of oscillation |

## Proposal

Add TweakXL 1.12.0 for this test only. During pause, comma and period choose level 0 to 3, which multiply those four values by 1, 2, 4 or 10 on resume.

- In memory only, for that game session. Nothing is written to game files or saves.
- Stock values are read first. If any value isn't found where expected, nothing is changed.
- Applies to the drive model of the vehicle driven, and to any other vehicle sharing that model.

## Risks and unknowns

- The running vehicle may not use a changed value until re-entered, or at all.
- Higher values may make the car twitchy or unstable. These multipliers are test points, not recommended settings.
- TweakDB changes through this route could count as a change a game update breaks. Publication and dependency review would come later.
- Flat names were derived from the record's getter names, and are checked by reading stock values first.

## Validation

The log shows stock values matching the earlier report, `written=true updated=true` for each level. The user compares levels for catch-up speed and stability.

## Result, 2026-11-04

Approved by the user; TweakXL added and the script replaced, all hash-checked. Two launches. No crash.

- Stock values were found in both launches, on different drive models: 1.0, 0.1, 95, 128 and 1.0, 0.15, 100, 130.
- Levels 1, 2 and 3 logged `written=true updated=true`.
- User report: the car still takes too long to catch up.
- `AutoSave-5` changed in the saves folder, as well as `user.gls`. The user has said they do not need recent autosaves.

**This does not rule these values out.** The test has three gaps:

1. Nothing read the values back after writing, so the write taking effect is unproven.
2. A vehicle already in the world may keep values it read when it spawned. The test re-entered the same car, and never tried a newly spawned one.
3. Nothing measured the tire angle, so the result rests on feel alone.

## Reversal

Removing TweakXL and the script lines ends it. Stock values return on the next launch regardless.

## Measurement build, 2026-11-04

Built to close the gaps above; not yet run in the game. Plugin 0.6.0 and script hashes are in `evidence/TEST_RESULTS.csv`.

- **Read-back:** each time the player becomes the driver, the log shows the four values read through the vehicle's record, the applied level and the vehicle's entity ID.
- **New car:** the test procedure includes calling a different vehicle after setting a level.
- **Response samples:** each frame while driving and not paused, up to 15,000 per launch, the log gets time, stick value, game steering action, vehicle heading and speed. This allocates on the game's thread and belongs to this build only. Script and plugin values are sampled at slightly different moments.

No tire or steering angle was found exposed to scripts. Heading change while moving is a proxy: it covers the game's steering and the car's physics together, and cannot separate them. It gives no result while parked.

Names such as `GetWorldYaw` and `GetCurrentSpeed` were checked only by compiling against the game's scripts.

## Measurement run, 2026-11-04

Approved by the user; files replaced and hash-checked. The game was still running when the log was read, so shutdown and save state are not covered.

- **User report:** "not making a difference".
- **Read-back:** the record values changed, from 1.0, 0.1, 95, 128 at level 0 to 10, 1, 950, 1280 at level 3. So the record is updated.
- **Same car:** both readings show entity `9020198`. **No newly spawned car was tested**, so a car keeping its spawn-time values is still possible. These values may also not be what limits the tires.
- **Measurement failure:** the 15,000 sample limit was reached at about 363 s, before level 3 began at about 400 s, so there is no level 3 response data. That limit was too low for the time the run took.
- **Level 0 response:** a steering action follows the stick with no lag resolved at this sampling. Heading rate follows `TurnX` by about 170 ms, with a weak correlation of 0.45, and only 13% of samples were moving. This is a rough indication only.

## Decision needed

Steering speed through data changes has now failed to help in two runs. One untested case remains: a car spawned after the values change. Do that one decisive test, or stop.

## Start-up run, 2026-11-05

The user chose the decisive test. Values are multiplied by 10 for every drive model in `ScriptableTweak.OnApply`, before any car exists. Plugin 0.7.0; hashes are in `evidence/TEST_RESULTS.csv`.

- **User report:** "Yes, faster". This is the first change in this series that the user could see.
- **Applied:** the log shows 108 drive models and 432 values changed, so the record type name is correct and all four values exist on every model.
- **Read-back:** car `9020198` read 10, 1, 950, 1280 and was parked for about 10 s. Car `9020199` read 10, 1.5, 1000, 1300 and was driven for about 20 minutes.
- **Stability of the run:** the wheel stayed connected, the plugin unloaded on quit, and no new crash report was written.
- **Saves:** `AutoSave-8` to `AutoSave-14` and `user.gls` were written by the game during the run. The user has said they do not need recent autosaves. Nothing was deleted.

**What this shows.** The values do affect the tires when applied before the car exists, and changing them on a car already in the world did not visibly help. The likely reading is that a car takes these values when it is created, but this rests on one report and the two earlier runs, not on a direct measurement.

**What it does not show.**

- Tire angle is still not measured, so "faster" is the user's judgement, and how closely the tires now match the wheel is unknown.
- All four values changed together, so which ones matter is unknown.
- 10 is a test point, not a chosen setting. Whether it causes twitchiness has not been reported.
- Heading rate followed `TurnX` by about 60 ms with a correlation of 0.53. The earlier 170 ms came from a run with only 13% of samples moving, against 93% here, so the two are not a fair before and after.
- TweakXL remains test-only pending the dependency review. Its use for the product is not decided here.

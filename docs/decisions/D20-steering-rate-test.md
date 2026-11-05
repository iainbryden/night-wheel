# Decision D20: test the game's steering speed values with TweakXL

Status: proposed for a test only, awaiting the user. Nothing has been tested in the game.
Date: 2026-11-04

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

## Reversal

Removing TweakXL and the script lines ends it. Stock values return on the next launch regardless.

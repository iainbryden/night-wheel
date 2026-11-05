# Decision D23: test boost as a physics push

Status: proposed for a feasibility test, requested by the user on 2026-11-05. Not run in the game.
Date: 2026-11-05
Requirements affected: R16.

## Request

A control that raises the car's speed beyond its top speed, with a strength setting from a preset 25% up to 100%, set to a button. The user also asks whether sparks, exhaust effects or sound can go with it.

## Alternatives

| Route | Finding | Result |
|---|---|---|
| Change speed values in game data | The steering tests showed that a car takes these values when created, so this can't switch on and off while driving. The package also rules out permanent stat changes as a shortcut | Rejected |
| Speed modifier functions | `AddMaxSpeedModifier` appears as a name in the game's script file, but it could not be called on the vehicle types tried | Not usable so far |
| Push the car through physics | `PhysicalImpulseEvent`, `GetTotalMass`, `GetWorldForward` and `QueueEvent` all compile. Whether a car responds is unknown | **Chosen for the test** |

## The test

- While the bound control is held and the car moves forward, each frame adds a forward push, up to a limit.
- **Limit:** the fastest speed seen without boost in this car, or the speed at the press if higher, raised by the strength. So the limit depends on how fast the car has already gone, which the package warns against for final use. The test logs the car's engine record values to look for a real top speed.
- **Push strength:** about 8.5 m/s² at 25%, capped at 16 m/s². These are provisional values for the test, not chosen settings.
- **Ends on:** release, brake, pause, leaving the car, a different car, and Autodrive.
- **Not included yet:** duration, cooldown, a check for being airborne, and reverse handling beyond the forward speed check. The package requires bounded limits and a decision on these before boost is a finished feature.
- The strength saves as `boost.percent`, 5 to 100, default 25. Files without it load normally. No vehicle data changes and nothing goes in a save.

## Sparks, exhaust and sound

Calling effects and sounds on a vehicle compiles (`GameObjectEffectHelper.StartEffectEvent`, `GameObject.PlaySoundEvent`). What is missing is the names. A listing of names in the game's script file shows no exhaust or spark effect that could be used, and guessing is not acceptable. Finding real names needs a tool that opens the game's files, such as WolvenKit, which is not installed and needs the user's approval. So this build has no effects. Whether suitable ones exist for every vehicle is unknown.

## Risks

- A push may make a car unstable, flip it or send it airborne.
- It may do nothing, if vehicle physics ignores this event.
- At high speed, the game's world streaming may not keep up.
- The User Agreement has a broad definition of cheats. This is a single-player driving feature, and the question remains open for publishing.

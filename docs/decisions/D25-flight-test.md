# Decision D25: test a flight mode

Status: requested by the user on 2026-11-05 for a test. Not run in the game. This feature is not in the package's requirements and goes beyond the planned wheel mod. It is added at the user's direction, and replaces the hover test in D24.
Date: 2026-11-05

## The test

- **Mode.** One button switches flight on and off. The saved name stays `hover`, so the user's existing binding carries over.
- **Height.** Holds a target height, starting 3 metres above where flight began. Climb and Descend controls move the target at 8 metres a second, within 400 metres of the start height.
- **Turning.** The wheel sets a turn rate of up to 60 degrees a second. The rate is measured from the heading each frame, smoothed, and corrected with opposed side pushes.
- **Level.** The tilt of the nose and right side, and how fast each changes, drive opposed vertical pushes. No banking.
- **Momentum.** Throttle gives up to 12 m/s² along the heading, to about 70 in the game's speed units. The brake gives up to 10 m/s² against travel. Sideways slide eases slowly towards the heading.
- **Boost.** Runs as usual on top.
- **Landing.** Switching off descends at 4 metres a second until vertical speed stays near zero for half a second, then flight ends.
- **Ends on:** landing, leaving the car, and Autodrive. Going into a menu doesn't end it.

All strengths are provisional test values. No vehicle data changes and nothing goes in a save.

## First run, 2026-11-05

Approved by the user; files replaced and hash-checked. User report: "Hover is not that great. It lifts off the ground slightly, or it hops." The game was still open when the log was read, and it closed before the second build was made.

- **Sustained flight happened.** From 87 to 102 seconds, the car held about 2.2 to 3.2 metres above its start, at speeds up to about 43, with pitch and roll mostly within about 0.1.
- **Slight lift is explained.** The target was only 3 metres, and Climb and Descend were never bound.
- **Unexpected climb.** From about 103 seconds, the car rose over 100 metres above a 3 metre target, while boost was held. It kept rising at several metres a second for seconds after release, with calculated lift at its lower limit of zero. Boost pushed along the nose, which explains part of this. Why gravity didn't bring it down sooner isn't explained.
- **Hopping.** From 428 and 479 seconds, the car stayed about 2 to 4 metres below its start, with vertical speed swinging between about -8 and +4, and calculated lift should have been high. Whether this was terrain or something holding the car down isn't known. The hop may also come from quick on and off presses seen at 438 seconds.
- **Turning.** A steering value of 0.5 gave about 12 degrees a second, against 30 wanted.
- **Not reported:** whether Esc stayed on the pause menu, and the map. This run logged no use of the settings page.

## Second build

Plugin 0.18.0. `wd_game.dll` SHA-256 `d588afe221537309bcd7d8a04bfa2805a2acd1cff1f717dd08992852ef2e70f8`; `WheelDriveLoadTest.reds` SHA-256 `166b12bb4372f3b4ba236d284e711231d7e4a16dc41a700e0b766ec4de0890cf`. Staged in `out/flight2-test/`. Not run in the game.

- Lift-off goes to 15 metres.
- Height control can now push down, from -15 to +40 m/s².
- Climb and Descend are 12 metres a second.
- Boost adds level thrust in flight, and its normal push is off while flying.
- Turning correction is more than doubled.
- The log adds calculated lift.

Checks: lints clean, 146 of 146 tests passed, script compiled against a copy of the game's `final.redscripts`.

## Second run, 2026-11-05

Approved by the user, who asked to test; files replaced and hash-checked. User report: climb and descend don't work.

- **The controls work.** Climb and Descend were bound to plus and minus, `fired` names them, and the target height moved as far as 84 metres.
- **The car didn't follow.** With lift at its 40 m/s² limit, the height stayed between about 0 and 2.5 metres, with vertical speed swinging between about -5 and +4 and pitch reaching -0.59. This is the hopping seen before.
- **Inferred cause.** Lift was applied at the vehicle's position, which may not be its centre of mass, so strong lift tips the nose down into the ground. Not proven. The earlier hold at 3 metres used lift near 10, where the tipping would be small.

## Third build

Plugin 0.19.0. `wd_game.dll` SHA-256 `eb09b162a72d04bca27031ee835fb100d08e55ea14000c5685b644391289e5b0`; `WheelDriveLoadTest.reds` SHA-256 `353d5adef434ca7fd1927b00c3f6828dc5f1165687e62e9ec46aaf06b9260e59`. Staged in `out/flight3-test/`. Not run in the game.

- Lift, thrust and the turning and levelling pairs act around the centre of mass, using the drive model's `Center_of_mass_offset`. The directions of that offset are assumed from its name, and the log records it.
- Lift is limited to 20 m/s², and levelling is stronger.
- The target height stays within 15 metres of the car, so letting go of Climb stops the climb soon.

Checks: lints clean, 146 of 146 tests passed, script compiled against a copy of the game's `final.redscripts`.

## Third run, 2026-11-05

Approved by the user; files replaced and hash-checked. User report: gravity brings the vehicle back down, and L2 makes it hop.

- **Level now.** Parked, pitch and roll stayed near 0.00, and the centre of mass offset was small (0, 0.06, 0.13), for a mass of 1600. So tipping wasn't the main cause, and the earlier inference is only partly right.
- **Lift doesn't deliver.** Calculated lift was at its 20 m/s² limit the whole time, about twice gravity. The car stayed at about 0 to 2.3 metres parked and about 7 to 9.6 metres when moving, with vertical speed swinging between about -4.6 and +6.2. A true net upward 10 m/s² would have climbed quickly. So much of the push isn't reaching the car, or something is working against it.
- **Candidates, none proven:** events being dropped or limited when several arrive in a frame; the event's `radius` reducing its effect; added gravity in the game's vehicle physics, since names such as `BaseAddedGravity` and `DriveSpeedAddedGravity` appear in the game's script file; and irregular script ticks.

## Direct alternative, not built

`GetTeleportationFacility().Teleport` compiles on a vehicle. Setting the car's position each frame would avoid gravity and physics entirely. It may stutter, may not collide with buildings, and may upset the game's vehicle state. No script function to set velocity or switch off gravity was found among the names tried.

## Fourth build: position control

Chosen by the user. Plugin 0.20.0. `wd_game.dll` SHA-256 `8a3646d5ed0a8f874a1f155188f5e9ec4ff2e9b06c047bb7d8b4f7826ef257f6`; `WheelDriveLoadTest.reds` SHA-256 `6029056eb66f3a153718043a8a6e5df815420b968b7d7792aa1c500c01582905`. Staged in `out/flight4-test/`. Not run in the game.

- The car's position and heading are set each frame with `GetTeleportationFacility().Teleport`, and the mod simulates motion.
- It starts with the car's current horizontal velocity and rises to 15 metres. The wheel sets up to 60 degrees a second. Throttle gives up to 15 m/s², the brake up to 25, boost adds thrust, and sideways motion fades.
- Climb and Descend are 15 metres a second, within 400 metres of the start height.
- A downward ray in the `Static` collision group keeps the car above ground and ends a landing near it. If no ground is found, a landing ends at the start height.
- One upward push each frame cancels gravity, to limit fall speed building between frames.
- The log records clearance, simulated and actual height, and physics vertical speed.

Assumptions to check: `EulerAngles.Yaw` matches `GetWorldYaw`, the ray's arguments and group are right, and teleporting each frame doesn't upset the vehicle. There is no check for buildings ahead, and the car level with no banking. Its speed isn't carried over when flight ends.

Checks: 146 of 146 tests passed and lints were clean before a final script-only change, and the final script compiled against a copy of the game's `final.redscripts`.

## Fourth run, 2026-11-05

Approved by the user; files replaced and hash-checked. User report: "flight model is good", but lift-off goes higher than needed. This build is the reference to return to. Log kept as `out/game-before/load-test-run24.log`. No new crash report appeared.

- **Position control holds.** Over about 30 flights, height stayed on target when no height control was held, and the actual height stayed within about 1 metre of the simulated height, with a few readings up to about 2.
- **The ground ray often finds nothing.** Of 1938 logged samples, 1109 found no ground. Many of those are high up, but 161 were within 20 metres of the flight's start height, so the ray misses some surfaces within its 200 metre range. Which surfaces isn't known.
- **A landing can end in mid-air.** With no ground found, a landing ends at the start height and the car drops. In one flight it ended about 110 in world height, and two seconds later flight began again about 25 metres lower. That is close to a free fall. Another run shows about -35 m/s of vertical speed at the next start.
- **Resting clearance.** Parked, the ray reads about 0.25 to 0.36 metres, so a landing that ends at the 0.8 metre floor drops about half a metre.

## Fifth build: lower lift-off

Script change only. Lift-off goes to 5 metres instead of 15. `wd_game.dll` is unchanged at plugin 0.20.0, SHA-256 `8a3646d5ed0a8f874a1f155188f5e9ec4ff2e9b06c047bb7d8b4f7826ef257f6`; `WheelDriveLoadTest.reds` SHA-256 `1e440e03375616ad9e1756a89e8c3f2156ad415ca8411f97a5e4e546155ce589`. Never installed or run in the game, and replaced by the sixth build. The script compiled against a copy of the game's `final.redscripts`.

## Sixth build: Climb and Descend start and end flight

Chosen by the user, who first declined the idea and then asked to try it. Script change only. `wd_game.dll` is unchanged at plugin 0.20.0, SHA-256 `8a3646d5ed0a8f874a1f155188f5e9ec4ff2e9b06c047bb7d8b4f7826ef257f6`; `WheelDriveLoadTest.reds` SHA-256 `25dc91ca4b6bbd8d3d861bb26e10f617462713f37f285e1c4941fbf1f7e66929`. Staged in `out/flight6-test/`. Not run in the game. The script compiled against a copy of the game's `final.redscripts`. The Rust tests weren't rerun, because no Rust changed.

- A fresh press of Climb starts flight when it is off, or takes back a landing. Lift-off goes to 3 metres, and holding Climb keeps rising.
- Holding Descend with clearance under 1.2 metres for half a second ends flight, so it can't drop the car from the air.
- The flight mode button still works as before, with the same 3 metre lift-off and the same landing. It stays as a way out where the ray can't see the ground, and the mid-air landing above still applies to it.
- When the `Static` ray misses, the log tries `Terrain`, `Water`, `Road` and `World Static`, once a second. These names appear in the game's script file, but whether each is a collision group isn't known.
- Speed still isn't carried over when flight ends, so ending flight while moving fast stops the car abruptly.

## Sixth run, 2026-11-05

Approved by the user; files replaced and hash-checked. User report: the combined buttons don't work well, because Descend often sends the car below the surface. The user asked to go back to the single mode button and keep the lower lift-off. Log kept as `out/game-before/load-test-run25.log`. No new crash report appeared.

- **Likely cause.** The ground check only looked at `Static`. Of 313 logged misses, the `Terrain` ray hit in 274, sometimes less than a metre below the car. So there was no floor over those surfaces. `Water` hit in 7. `Road` and `World Static` never hit.
- **Not new.** Descend's movement code didn't change in the sixth build, so the same gap was there before. Using Descend to reach the ground each time may be why it showed up now. That part isn't proven.
- **Still unseen.** 39 of the 313 misses found nothing in `Terrain` either.

## Seventh build: back to one mode button

Script change only. `wd_game.dll` is unchanged at plugin 0.20.0, SHA-256 `8a3646d5ed0a8f874a1f155188f5e9ec4ff2e9b06c047bb7d8b4f7826ef257f6`; `WheelDriveLoadTest.reds` SHA-256 `5b18947c653da52596c3f8bf344a0729ac142c015aa1af454b5dc2973c8f12c0`. Staged in `out/flight7-test/`. Not run in the game. The script compiled against a copy of the game's `final.redscripts`. Compared with the fourth build, which the user liked, only two things differ:

- Lift-off goes to 3 metres instead of 15.
- The ground check takes the higher of the `Static` and `Terrain` hits, so the floor and landings should also work over terrain.

Climb and Descend only change height again. The mid-air landing still applies where neither ray finds ground. Never installed, and kept staged as the fallback.

## Eighth build: two buttons with the floor fix

Chosen by the user, to test whether the floor bug was the real problem with the sixth build. Script change only. `wd_game.dll` is unchanged at plugin 0.20.0, SHA-256 `8a3646d5ed0a8f874a1f155188f5e9ec4ff2e9b06c047bb7d8b4f7826ef257f6`; `WheelDriveLoadTest.reds` SHA-256 `c4a15422ae9c04acf814a343e0b515a5a58355445afb03fbf34484022949576b`. Staged in `out/flight8-test/`. Not run in the game. The script compiled against a copy of the game's `final.redscripts`.

- Climb starts flight and Descend ends it, as in the sixth build.
- The ground check takes the higher of the `Static` and `Terrain` hits, as in the seventh build.
- The flight mode button stays as a way out during this test, and is to be removed from flight if Descend proves reliable.
- The log notes each second that neither ray finds ground. It no longer tries `Water`, `Road` or `World Static`.

## Eighth run, 2026-11-05

Approved by the user; files replaced and hash-checked. User report: "Works well." The car still goes through some surfaces, but not through the floor on landings, and the user accepts that. Two buttons is better than a toggle. Log kept as `out/game-before/load-test-run26.log`. No new crash report appeared.

- 16 flights: 11 ended by holding Descend at the ground, and 3 by the flight mode button's landing. The mode button's landing was started 5 times.
- There is still no check for buildings ahead, which is the likely reason for going through surfaces while flying. That cause isn't proven.

## Unknowns

- Whether the car's inertia suits these values. The first version may wobble or feel wrong.
- Air resistance in the game's physics, and whether it limits speed.
- Whether time stops in every menu. If it doesn't, pushes stop while physics runs and the car would fall.
- World streaming at height and speed, collisions with buildings, and game logic for falling vehicles. If flight stops at height, nothing softens the fall.
- A landing test relies on stopping, so it could end in mid-air if the car hangs at near zero vertical speed for half a second.

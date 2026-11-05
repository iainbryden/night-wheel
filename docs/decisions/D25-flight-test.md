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

## Unknowns

- Whether the car's inertia suits these values. The first version may wobble or feel wrong.
- Air resistance in the game's physics, and whether it limits speed.
- Whether time stops in every menu. If it doesn't, pushes stop while physics runs and the car would fall.
- World streaming at height and speed, collisions with buildings, and game logic for falling vehicles. If flight stops at height, nothing softens the fall.
- A landing test relies on stopping, so it could end in mid-air if the car hangs at near zero vertical speed for half a second.

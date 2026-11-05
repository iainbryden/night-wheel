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

## Unknowns

- Whether the car's inertia suits these values. The first version may wobble or feel wrong.
- Air resistance in the game's physics, and whether it limits speed.
- Whether time stops in every menu. If it doesn't, pushes stop while physics runs and the car would fall.
- World streaming at height and speed, collisions with buildings, and game logic for falling vehicles. If flight stops at height, nothing softens the fall.
- A landing test relies on stopping, so it could end in mid-air if the car hangs at near zero vertical speed for half a second.

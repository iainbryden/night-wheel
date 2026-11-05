# Decision D24: test hover as physics pushes

Status: proposed for a feasibility test, requested by the user on 2026-11-05. Not run in the game. This feature is not in the package's requirements, and is added at the user's direction.
Date: 2026-11-05

## Request

A button that lifts the car off the ground while held, and lowers it on release. The car keeps taking its direction from the wheel.

## The test

Uses the same `PhysicalImpulseEvent` route that moved the car for boost.

- **Height:** holds about 2 metres above the height where the control was pressed. It doesn't read the ground, so a slope can bring the car into contact, and a drop leaves it higher.
- **Steering:** with the tires off the ground, the game's steering has nothing to work with. So the wheel's steering action drives two opposed side pushes, at points 2 metres ahead of and behind the car.
- **Direction of travel:** sideways slide is damped, so the car moves where it points.
- **Pedals:** throttle and brake push the car along or slow it.
- **Landing:** after release, the fall is eased for up to 1.5 seconds.
- **Ends on:** release, leaving the car, menus and pause, and Autodrive.

All strengths are provisional test values. No vehicle data changes and nothing goes in a save.

## Unknowns

- Whether the event's position is honoured. If it is ignored, the opposed pushes cancel out and the wheel won't turn the car. The log records height, speed, steering and heading change once a second.
- Whether the car stays level. Nothing here corrects tilt.
- Whether the game's vehicle physics fights being held off the ground, or whether the game treats it as a crash or a fall.
- How it behaves near traffic, bridges and buildings.

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

## Result, 2026-11-05

Approved by the user; files replaced and hash-checked. User report: the car hovered slightly, with no momentum once the wheels were off the ground. The user wants to go further, to a flying vehicle. Readings from the log, once a second:

- **Height works.** On long holds it stayed at about 1.4 to 2.07 above the start, against a target of 2.
- **Offset pushes turn the car.** So the event's position is honoured. But the heading changed by up to about 330 degrees in a second, with steering values near zero. Nothing damps rotation in the air, so small steering builds into a spin.
- **Momentum.** Speed fell from about 21 to low single figures within a few seconds. The sideways damping is the likely cause, because a spinning car turns its travel into sideways slide, which is then removed. This is inferred. The game's air resistance may also play a part.
- **Going back.** The new scenario wrap fired (`wheel-back-scenario`). The pause menu's resume came about 0.5 seconds later in both cases, where before it came at the same moment. Whether the user stayed on the pause menu has not been reported.

A flying vehicle looks possible by the same route, because lift and turning both work. It needs yaw control by measured turn rate, tilt correction, no sideways damping of normal flight, and climb and descent controls. It is not proven. World streaming at height and speed, collisions and game logic for falling vehicles are untested.

## Unknowns at the time of building

- Whether the event's position is honoured. If it is ignored, the opposed pushes cancel out and the wheel won't turn the car. The log records height, speed, steering and heading change once a second.
- Whether the car stays level. Nothing here corrects tilt.
- Whether the game's vehicle physics fights being held off the ground, or whether the game treats it as a crash or a fall.
- How it behaves near traffic, bridges and buildings.

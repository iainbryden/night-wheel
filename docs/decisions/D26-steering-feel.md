# Decision D26: steering feel belongs in the mod

Status: built for testing, version 0.22.0, on branch `steering-feel`. Not run in the game.
Date: 2026-11-06

## Why

The steering felt right only at G HUB sensitivity 4 and operating range 210, and sensitivity 4 is far from neutral. New users shouldn't need outlier values. D21 already says the mod must not require particular G HUB values, and nothing on the in-game page could change the feel.

## What was built

Three sliders in a new Steering section of the settings page:

| Slider | Range | Steps | Applies | Saved as |
|---|---|---|---|---|
| Steering range | 180 to 900 degrees | 30 | At once | `steering.range` |
| Center response | 1.0 to 4.0 | 0.1 | At once | `steering.exponent` |
| Steering speed | x1 to x10 | 1 | Next game start, because cars take these values when the game starts | `steering.speed` |

- **Range** is measured against G HUB at 900, its widest setting, because the mod can't read G HUB. With G HUB at another value, the degrees shown won't be real degrees.
- **Center response** is the response curve that already existed in the settings, and the setup app could already change it. 1.0 is straight. Higher values soften center, and full lock stays at full lock.
- **Older settings files** load unchanged. No range means 900, which is the full travel as before, so anyone keeping today's G HUB values feels no change until they move a slider.
- **The steering speed text file** is adopted once if the settings have no speed, and the next save keeps it.
- **On the wheel path**, range and curve are read from atomics, with no lock, and the curve adds one power calculation per poll. Its cost isn't measured.

## A correction

The user was told that the mod's jump to about 35% stick might make center touchy. The code doesn't support that. `compensate` is fitted so the game's steering action starts from zero at that point, so the touchiness may come from the game's own steering. Why such a low sensitivity feels right remains unknown.

## Still to do

1. With G HUB at sensitivity 50 and range 900, the user finds slider values that match the feel at 4 and 210. That G HUB's 50 is neutral is the project's understanding, not confirmed by Logitech.
2. Those values become the defaults for new profiles. They stay neutral until then, so nothing changes for this user by surprise.
3. Without G HUB's narrow range, the wheel may turn past full lock with nothing to stop it. If that feels wrong, narrowing in G HUB stays an option, and the setup note needs to say so.

## Limits

The mod can only narrow what G HUB allows. A user with G HUB set narrower than the slider gets less range than shown, and the mod can't detect it. The page's layout grew by one section, and it's still untested on ultrawide and other UI scales.

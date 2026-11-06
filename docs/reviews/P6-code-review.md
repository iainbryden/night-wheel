# Code review of 0.21.0

Date: 2026-11-05. The user supplied a review of version 0.21.0 from source inspection. The reviewer didn't run the game, compile the script against it or use a G29. It concerns this project's own code and contains no outside code. Each finding was checked against the source here before any change.

Nothing was posted to GitHub.

The fixes are in version 0.21.1. They have passed automated tests and an offline script compile, and **none has been tried in the game or on the wheel**.

## Findings

| # | Finding | Check | Fix |
|---|---|---|---|
| 1 | Unplugging the wheel can leave boost or pedals applied | Confirmed. Also, if the reader thread failed, the wheel stayed marked connected and its last readings kept applying | Losing the wheel, or the reader ending for any reason, clears everything that came from it. The script's reads of boost, held controls and pedals are checked again when read, including that the game is still polling |
| 2 | Flight state can carry across a reload or vehicle change | Confirmed | Session start and end clear boost, flight and Autodrive state. Flight is tied to one vehicle and ends if the vehicle changes |
| 3 | A press during pause or loading can apply on the first eligible poll, and there is no focus check | Confirmed. `RIDEV_INPUTSINK` itself has to stay, because the reader's message-only window is never in front | The gate now includes focus, read once a frame so the input path doesn't change. Any change of pause, focus, driving or connection, or a gap in polls or script updates, closes one poll, requires held controls to be released and forgets waiting radio actions |
| 4 | Unload can return while worker threads still run | Confirmed | The log thread now notices a stop request within 100 ms. If a thread still hasn't ended after its wait, the library stays loaded until the process ends. This was chosen over an unlimited wait, which could hang the game's shutdown |
| 5 | Boost can do nothing on the first press after a crawl | Confirmed | The speed limit is now based on at least 20 in the game's speed units, a provisional value. Above that speed, behaviour doesn't change |

## Added after the review

Version 0.21.2 adds boost's limits, using the user's values and rules. All are untested in the game, and the times are provisional until tried on the G29.

- A burst lasts up to 5 seconds.
- Any end to a burst starts a 3 second cooldown, including release and braking.
- A new burst takes a fresh press of the button after the cooldown, so holding can't restart it, tapping can't get round the limit, and letting go of the brake doesn't count as a press.
- Driving and flight share one timer, so switching between them can't reset it. A new session clears the cooldown, because game time may restart with a session.

## Not changed

- **Flight after losing the wheel:** the car stays in the air and coasts, because Descend comes from the wheel. Leaving the car ends flight.
- **Status messages:** the settings page still doesn't separate a disconnected wheel, an unsupported wheel mode or a failed hook.
- **Steering speed:** still set in a text file.
- **Settings page:** Esc, and the layout at ultrawide and other UI scales, are untested.
- **Build:** the reviewer's workspace build stopped when RED4ext's CMake step tried to write into a read-only Cargo cache. That hasn't happened here, and it wasn't reproduced.

## First runs of 0.21.2, 2026-11-05

Approved by the user; files replaced and checksums verified. User report: "All good so far." The logs of two runs are kept as `out/game-before/night-wheel-run27.log` and `night-wheel-run28.log`. No new crash report appeared, and both runs reached a clean plugin unload.

- **Shown by the logs:** settings copied once from the old folder, flight started and ended with Climb and Descend, and each run shut down cleanly.
- **Boost:** four bursts, each ending within about 0.3 seconds as released, and the time limit never triggered. Those would be short taps, which the user hasn't confirmed. The same route carried Climb and Descend held for many seconds, so a cut-out is unlikely but not excluded.
- **Not shown by the logs:** a 5 second hold, a wheel disconnect and a reload during flight. Tests 1, 2, 5 and 6 below can't be counted as passed from these runs. Test 3 leaves no log entry.

## Run of 0.21.3, 2026-11-05

Log kept as `out/game-before/night-wheel-run29.log`. No new crash report appeared, and the run reached a clean plugin unload.

- **Boost limit works, per the log.** One burst held from 565.6 to 570.6 seconds on the log's clock ended by the time limit at 5.0 seconds. The next started 6.4 seconds later, and other bursts ended on release.
- **Still not shown:** a wheel disconnect, with none logged, and a reload during flight, with no flight ended by a session change. Boost during flight and held buttons leave no clear log entry and haven't been reported.

## To test in the game

1. Unplug the wheel while holding boost, and while flying with the throttle down.
2. Reload a save while flying, and leave a car while flying and enter another.
3. Hold a bound button through a pause, a load and Alt-Tab. It must not act until released and pressed again.
4. Press boost from a crawl in a car just entered.
5. Hold boost for over 5 seconds: it should stop and stay off until a new press after the cooldown.
6. Tap boost quickly, brake during a burst, and switch into flight during a burst. None should restart or extend it.
7. Quit to desktop, and check for a clean shutdown in the log.

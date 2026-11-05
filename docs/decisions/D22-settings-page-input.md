# Decision D22: one press to bind, and the wheel doesn't run the page

Status: accepted by the user on 2026-11-05. Not yet tested in the game.
Date: 2026-11-05
Requirements affected: R07, R08, R12.

## Problem

The user rejected the first two versions of the settings screen. Confirming each bind was an extra step with little value, and wheel buttons both ran the page and could be bound, which is a conflict by design.

## Choice

- **One press binds.** Choose a row, press a wheel control, and it is set and saved. This departs from the confirm step in the package's `UX_AND_SETTINGS.md`, by the user's decision. A wrong press is fixed by binding again.
- **A control already in use moves.** The message names the row it left.
- **The mouse and keyboard run the page.** The wheel is only read to choose a binding and to light a row in a live test. So every wheel control can be bound, including X, Square, Circle and the D-pad. The cost is that this page needs a mouse or keyboard.
- **Going back.** The game's `back` action, which its mappings give to Escape and the right mouse button, cancels a bind or returns to the pause menu.

## Change from the proposal

Right-click to clear was proposed. The game's mappings already give the right mouse button to `back`, so it follows the game instead, and Delete clears.

## Alternative not taken

Listing the wheel's controls and picking a function for each would rule out overlaps. The user pointed to the game's Key Bindings page, so this follows that.

## Unknowns

- Whether wrapping `OnGlobalRelease` stops the pause menu from closing. The name comes from reflection and the signature from compiling. Its behaviour is unproven, and the game may also close the menu by another route.
- The route by which the wheel's D-pad reached the pause menu. If the game sees wheel buttons directly, a wheel button could still act on the hidden menu, such as Circle acting as `back` during a bind.

## Validation

See `docs/records/P5-bind-test-build.md`.

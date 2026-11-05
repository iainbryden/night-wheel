//! The in-game settings screen: choosing a function and binding a wheel control to it. Free of
//! Windows and game types so it can be tested without either. It runs only while the screen is
//! open, and never on the steering path.

use crate::drive::{HAT_DOWN, HAT_LEFT, HAT_RIGHT, HAT_SHIFT, HAT_UP, bit_for};
use std::collections::BTreeSet;
use wd_core::assignment::{Action, Bindings};
use wd_core::input::{ButtonId, Control, Sample};
use wd_core::listen::{ButtonListener, LISTEN_TIMEOUT_MS, ListenOutcome};

const BUTTON_PAGE: u16 = 0x09;

// Gamepad button values from Microsoft's XINPUT_GAMEPAD documentation.
const PAD_DPAD_LEFT: u16 = 0x0004;
const PAD_DPAD_RIGHT: u16 = 0x0008;
const PAD_LEFT_THUMB: u16 = 0x0040;
const PAD_RIGHT_SHOULDER: u16 = 0x0200;
const PAD_A: u16 = 0x1000;
const PAD_B: u16 = 0x2000;
const PAD_X: u16 = 0x4000;
const PAD_Y: u16 = 0x8000;

/// The functions on the screen and the gamepad button each one presses. The pairs come from the
/// game's `inputContexts.xml` and `inputUserMappings.xml` on patch 2.5 (SRC-032).
pub const ROWS: [(Action, u16); 8] = [
    (Action::Handbrake, PAD_A),
    (Action::Horn, PAD_X),
    (Action::Lights, PAD_DPAD_LEFT),
    (Action::CameraChange, PAD_DPAD_RIGHT),
    (Action::VehicleExit, PAD_B),
    (Action::Autodrive, PAD_LEFT_THUMB),
    (Action::RadioMenu, PAD_RIGHT_SHOULDER),
    (Action::CombatMode, PAD_Y),
];

// Screen controls on the wheel. See docs/records/P1-control-map.md.
const WHEEL_UP: u64 = HAT_UP << HAT_SHIFT;
const WHEEL_DOWN: u64 = HAT_DOWN << HAT_SHIFT;
const WHEEL_SELECT: u64 = 1 << 0; // X
const WHEEL_CLEAR: u64 = 1 << 1; // Square
const WHEEL_CLOSE: u64 = 1 << 2; // Circle

/// A request from the keyboard or the wheel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Up,
    Down,
    Select,
    Clear,
    Close,
}

impl Command {
    /// The number the game script uses for each key.
    pub fn from_code(code: i32) -> Option<Self> {
        match code {
            1 => Some(Self::Up),
            2 => Some(Self::Down),
            3 => Some(Self::Select),
            4 => Some(Self::Clear),
            5 => Some(Self::Close),
            _ => None,
        }
    }
}

/// The wheel's name for a control, from the observed map. Numbers not on the map keep their number.
pub fn control_name(control: Control) -> String {
    match control {
        Control::Hat(0) => "D-PAD UP".to_owned(),
        Control::Hat(2) => "D-PAD RIGHT".to_owned(),
        Control::Hat(4) => "D-PAD DOWN".to_owned(),
        Control::Hat(6) => "D-PAD LEFT".to_owned(),
        Control::Hat(position) => format!("D-PAD {position}"),
        Control::Button(button) => match button.usage {
            1 => "X".to_owned(),
            2 => "SQUARE".to_owned(),
            3 => "CIRCLE".to_owned(),
            4 => "TRIANGLE".to_owned(),
            5 => "RIGHT PADDLE".to_owned(),
            6 => "LEFT PADDLE".to_owned(),
            7 => "R2".to_owned(),
            8 => "L2".to_owned(),
            9 => "SHARE".to_owned(),
            10 => "OPTIONS".to_owned(),
            11 => "R3".to_owned(),
            12 => "L3".to_owned(),
            20 => "PLUS".to_owned(),
            21 => "MINUS".to_owned(),
            22 => "DIAL RIGHT".to_owned(),
            23 => "DIAL LEFT".to_owned(),
            24 => "RED ENTER".to_owned(),
            25 => "PS".to_owned(),
            other => format!("BUTTON {other}"),
        },
    }
}

/// The controls in a set of control bits. A diagonal counts as its two directions.
fn held_controls(bits: u64) -> BTreeSet<Control> {
    let mut held: BTreeSet<Control> = (1..=60u16)
        .map(|usage| {
            Control::Button(ButtonId {
                page: BUTTON_PAGE,
                usage,
            })
        })
        .filter(|&control| bit_for(control).is_some_and(|bit| bits & bit != 0))
        .collect();
    for (direction, position) in [(HAT_UP, 0), (HAT_RIGHT, 2), (HAT_DOWN, 4), (HAT_LEFT, 6)] {
        if bits & (direction << HAT_SHIFT) != 0 {
            held.insert(Control::Hat(position));
        }
    }
    held
}

/// A sample for the listener. It holds one hat position, so a diagonal counts as its first one.
fn sample_of(bits: u64) -> Sample {
    let mut sample = Sample::default();
    for control in held_controls(bits) {
        match control {
            Control::Button(button) => {
                sample.buttons.insert(button);
            }
            Control::Hat(position) => {
                sample.hat.get_or_insert(position);
            }
        }
    }
    sample
}

/// Names what is held, for the screen.
pub fn held_names(bits: u64) -> String {
    let names: Vec<String> = held_controls(bits).into_iter().map(control_name).collect();
    if names.is_empty() {
        "NONE".to_owned()
    } else {
        names.join(" + ")
    }
}

#[derive(Debug)]
enum Mode {
    Browse,
    Listening(ButtonListener),
    Confirm(Control),
}

/// What the screen is doing.
#[derive(Debug)]
pub struct Screen {
    selected: usize,
    mode: Mode,
    message: String,
    done: bool,
}

impl Screen {
    pub fn new() -> Self {
        Self {
            selected: 0,
            mode: Mode::Browse,
            message: String::new(),
            done: false,
        }
    }

    /// True once the player has asked to leave.
    pub fn done(&self) -> bool {
        self.done
    }

    fn action(&self) -> Action {
        ROWS[self.selected].0
    }

    /// Acts on a request. Returns true when `bindings` changed.
    pub fn command(
        &mut self,
        command: Command,
        now_ms: u64,
        held: u64,
        bindings: &mut Bindings,
    ) -> bool {
        match (&self.mode, command) {
            (Mode::Browse, Command::Up) => {
                self.selected = (self.selected + ROWS.len() - 1) % ROWS.len();
                self.message.clear();
                false
            }
            (Mode::Browse, Command::Down) => {
                self.selected = (self.selected + 1) % ROWS.len();
                self.message.clear();
                false
            }
            (Mode::Browse, Command::Select) => {
                let sample = sample_of(held);
                self.mode = Mode::Listening(ButtonListener::new(
                    now_ms,
                    Some(&sample),
                    LISTEN_TIMEOUT_MS,
                ));
                self.message.clear();
                false
            }
            (Mode::Browse, Command::Clear) => {
                let had_one = bindings.get(self.action()).is_some();
                bindings.clear(self.action());
                had_one
            }
            (Mode::Browse, Command::Close) => {
                self.done = true;
                false
            }
            (Mode::Confirm(control), Command::Select) => {
                let control = *control;
                for other in bindings.conflicts(self.action(), control) {
                    bindings.clear(other);
                }
                bindings.assign(self.action(), control);
                self.mode = Mode::Browse;
                true
            }
            (_, Command::Close) => {
                self.mode = Mode::Browse;
                self.message = "CANCELLED".to_owned();
                false
            }
            _ => false,
        }
    }

    /// Takes the wheel's state: `held` now, and `pressed` for anything pressed since the last call,
    /// including a press too short to still be held. Returns true when `bindings` changed.
    pub fn wheel(&mut self, now_ms: u64, held: u64, pressed: u64, bindings: &mut Bindings) -> bool {
        if let Mode::Listening(listener) = &mut self.mode {
            let sample = sample_of(held | pressed);
            match listener.update(now_ms, Some(&sample)) {
                ListenOutcome::Waiting => {}
                ListenOutcome::Detected(control) => self.mode = Mode::Confirm(control),
                ListenOutcome::TimedOut => {
                    self.mode = Mode::Browse;
                    self.message = "NO PRESS DETECTED".to_owned();
                }
            }
            return false;
        }
        let command = if pressed & WHEEL_SELECT != 0 {
            Command::Select
        } else if pressed & WHEEL_CLOSE != 0 {
            Command::Close
        } else if pressed & WHEEL_CLEAR != 0 {
            Command::Clear
        } else if pressed & WHEEL_UP != 0 {
            Command::Up
        } else if pressed & WHEEL_DOWN != 0 {
            Command::Down
        } else {
            return false;
        };
        self.command(command, now_ms, held, bindings)
    }

    pub fn set_message(&mut self, message: String) {
        self.message = message;
    }

    /// The screen's text.
    pub fn text(&self, bindings: &Bindings, steering: i32, speed: f32, held: u64) -> String {
        let mut text = String::from("NIGHT WHEEL - WHEEL SETTINGS\n");
        for (index, (action, _)) in ROWS.iter().enumerate() {
            let marker = if index == self.selected { "> " } else { "   " };
            let bound = bindings
                .get(*action)
                .map_or_else(|| "NOT BOUND".to_owned(), control_name);
            let label = action.label().to_uppercase();
            text.push_str(&format!("{marker}{label}: {bound}\n"));
        }
        text.push('\n');
        let label = self.action().label().to_uppercase();
        match &self.mode {
            Mode::Browse => {
                text.push_str(&format!(
                    "HOLDING {}   STEERING {steering}   SPEED X{speed}\n",
                    held_names(held)
                ));
                if !self.message.is_empty() {
                    text.push_str(&format!("{}\n", self.message));
                }
                text.push_str("UP/DOWN OR D-PAD: SELECT   ENTER OR X: BIND\n");
                text.push_str("DELETE OR SQUARE: CLEAR   PERIOD OR CIRCLE: CLOSE");
            }
            Mode::Listening(_) => {
                text.push_str(&format!("PRESS A WHEEL CONTROL FOR {label}\n"));
                text.push_str("PERIOD CANCELS");
            }
            Mode::Confirm(control) => {
                let name = control_name(*control);
                text.push_str(&format!("BIND {label} TO {name}?\n"));
                for other in bindings.conflicts(self.action(), *control) {
                    let other = other.label().to_uppercase();
                    text.push_str(&format!("THIS CLEARS {name} FROM {other}\n"));
                }
                text.push_str("ENTER OR X: CONFIRM   PERIOD OR CIRCLE: CANCEL");
            }
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEFT_PADDLE: u64 = 1 << 5;
    const DIAL: u64 = 1 << 21;

    fn usage(usage: u16) -> Control {
        Control::Button(ButtonId {
            page: BUTTON_PAGE,
            usage,
        })
    }

    #[test]
    fn selection_moves_and_wraps() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        screen.command(Command::Up, 0, 0, &mut bindings);
        assert_eq!(screen.selected, ROWS.len() - 1);
        screen.command(Command::Down, 0, 0, &mut bindings);
        assert_eq!(screen.selected, 0);
    }

    #[test]
    fn a_press_is_confirmed_before_it_binds() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        screen.command(Command::Select, 0, 0, &mut bindings);
        assert!(!screen.wheel(10, LEFT_PADDLE, LEFT_PADDLE, &mut bindings));
        assert_eq!(bindings.get(Action::Handbrake), None);
        assert!(screen.command(Command::Select, 20, 0, &mut bindings));
        assert_eq!(bindings.get(Action::Handbrake), Some(usage(6)));
    }

    #[test]
    fn a_control_held_before_listening_needs_a_release() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        screen.command(Command::Select, 0, LEFT_PADDLE, &mut bindings);
        screen.wheel(10, LEFT_PADDLE, 0, &mut bindings);
        assert!(matches!(screen.mode, Mode::Listening(_)));
        screen.wheel(20, 0, 0, &mut bindings);
        screen.wheel(30, LEFT_PADDLE, LEFT_PADDLE, &mut bindings);
        assert!(matches!(screen.mode, Mode::Confirm(_)));
    }

    #[test]
    fn a_pulse_too_short_to_be_held_is_still_caught() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        screen.command(Command::Select, 0, 0, &mut bindings);
        screen.wheel(10, 0, DIAL, &mut bindings);
        assert!(matches!(screen.mode, Mode::Confirm(control) if control == usage(22)));
    }

    #[test]
    fn listening_gives_up_and_changes_nothing() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        screen.command(Command::Select, 0, 0, &mut bindings);
        assert!(!screen.wheel(LISTEN_TIMEOUT_MS, 0, 0, &mut bindings));
        assert!(matches!(screen.mode, Mode::Browse));
        assert_eq!(bindings.iter().count(), 0);
    }

    #[test]
    fn cancelling_returns_without_closing_or_binding() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        screen.command(Command::Select, 0, 0, &mut bindings);
        screen.wheel(10, LEFT_PADDLE, LEFT_PADDLE, &mut bindings);
        screen.command(Command::Close, 20, 0, &mut bindings);
        assert!(matches!(screen.mode, Mode::Browse));
        assert!(!screen.done());
        assert_eq!(bindings.iter().count(), 0);
    }

    #[test]
    fn closing_from_browse_finishes() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        screen.command(Command::Close, 0, 0, &mut bindings);
        assert!(screen.done());
    }

    #[test]
    fn binding_a_used_control_warns_and_moves_it() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        bindings.assign(Action::Horn, usage(6));
        screen.command(Command::Select, 0, 0, &mut bindings);
        screen.wheel(10, LEFT_PADDLE, LEFT_PADDLE, &mut bindings);
        let text = screen.text(&bindings, 0, 2.0, 0);
        assert!(text.contains("THIS CLEARS LEFT PADDLE FROM HORN"), "{text}");
        screen.command(Command::Select, 20, 0, &mut bindings);
        assert_eq!(bindings.get(Action::Handbrake), Some(usage(6)));
        assert_eq!(bindings.get(Action::Horn), None);
    }

    #[test]
    fn clear_reports_a_change_only_when_there_was_a_binding() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        assert!(!screen.command(Command::Clear, 0, 0, &mut bindings));
        bindings.assign(Action::Handbrake, usage(6));
        assert!(screen.command(Command::Clear, 0, 0, &mut bindings));
        assert_eq!(bindings.get(Action::Handbrake), None);
    }

    #[test]
    fn the_wheel_works_the_screen() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        screen.wheel(0, 0, WHEEL_DOWN, &mut bindings);
        assert_eq!(screen.selected, 1);
        screen.wheel(0, 0, WHEEL_UP, &mut bindings);
        assert_eq!(screen.selected, 0);
        screen.wheel(0, WHEEL_SELECT, WHEEL_SELECT, &mut bindings);
        assert!(matches!(screen.mode, Mode::Listening(_)));
        screen.wheel(5, 0, 0, &mut bindings);
        screen.wheel(10, LEFT_PADDLE, LEFT_PADDLE, &mut bindings);
        screen.wheel(20, WHEEL_SELECT, WHEEL_SELECT, &mut bindings);
        assert_eq!(bindings.get(Action::Handbrake), Some(usage(6)));
        screen.wheel(30, 0, WHEEL_CLOSE, &mut bindings);
        assert!(screen.done());
    }

    #[test]
    fn every_function_presses_a_different_gamepad_button() {
        let flags: BTreeSet<u16> = ROWS.iter().map(|(_, flag)| *flag).collect();
        assert_eq!(flags.len(), ROWS.len());
    }

    #[test]
    fn names_come_from_the_map_and_unknown_numbers_stay_numbers() {
        assert_eq!(control_name(usage(8)), "L2");
        assert_eq!(control_name(usage(11)), "R3");
        assert_eq!(control_name(usage(13)), "BUTTON 13");
        assert_eq!(control_name(Control::Hat(4)), "D-PAD DOWN");
    }

    #[test]
    fn held_names_list_every_control() {
        assert_eq!(held_names(0), "NONE");
        assert_eq!(held_names(1), "X");
        let diagonal = (HAT_UP | HAT_RIGHT) << HAT_SHIFT;
        assert_eq!(held_names(diagonal), "D-PAD UP + D-PAD RIGHT");
    }
}

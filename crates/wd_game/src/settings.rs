//! The in-game settings screen: choosing a function and binding a wheel control to it. The mouse
//! and keyboard run the screen, and the wheel is only used to choose a control, so no wheel button
//! ever has two jobs. Free of Windows and game types so it can be tested without either. It runs
//! only while the screen is open, and never on the steering path.

use crate::drive::{HAT_DOWN, HAT_LEFT, HAT_RIGHT, HAT_SHIFT, HAT_UP, bit_for};
use std::collections::BTreeSet;
use wd_core::assignment::{Action, Bindings};
use wd_core::config::{Config, STEERING_RANGE, STEERING_SPEED_RANGE};
use wd_core::input::{ButtonId, Control, Sample};
use wd_core::listen::{ButtonListener, LISTEN_TIMEOUT_MS, ListenOutcome};

const BUTTON_PAGE: u16 = 0x09;

// Gamepad button values from Microsoft's XINPUT_GAMEPAD documentation.
const PAD_DPAD_LEFT: u32 = 0x0004;
const PAD_DPAD_RIGHT: u32 = 0x0008;
const PAD_DPAD_UP: u32 = 0x0001;
const PAD_DPAD_DOWN: u32 = 0x0002;
const PAD_START: u32 = 0x0010;
const PAD_LEFT_SHOULDER: u32 = 0x0100;
const PAD_LEFT_THUMB: u32 = 0x0040;
const PAD_RIGHT_THUMB: u32 = 0x0080;
const PAD_RIGHT_SHOULDER: u32 = 0x0200;
const PAD_A: u32 = 0x1000;
const PAD_B: u32 = 0x2000;
const PAD_X: u32 = 0x4000;
const PAD_Y: u32 = 0x8000;

// Functions with no gamepad button, which the game script carries out once per press. These sit
// above the gamepad bits. The script reads them shifted down by `SCRIPT_SHIFT`.
pub const SCRIPT_SHIFT: u32 = 16;
const SCRIPT_RADIO_TOGGLE: u32 = 1 << SCRIPT_SHIFT;
const SCRIPT_RADIO_NEXT: u32 = 2 << SCRIPT_SHIFT;
const SCRIPT_RADIO_PREVIOUS: u32 = 4 << SCRIPT_SHIFT;
const SCRIPT_VOLUME_UP: u32 = 8 << SCRIPT_SHIFT;
const SCRIPT_VOLUME_DOWN: u32 = 16 << SCRIPT_SHIFT;
/// Held rather than pressed once, so the script asks for it each frame.
pub const SCRIPT_BOOST: u32 = 32 << SCRIPT_SHIFT;
const SCRIPT_FLY_UP: u32 = 128 << SCRIPT_SHIFT;
const SCRIPT_FLY_DOWN: u32 = 256 << SCRIPT_SHIFT;

/// The functions on the screen and how each one reaches the game. Gamepad pairs come from the
/// game's `inputContexts.xml` and `inputUserMappings.xml` on patch 2.5 (SRC-032).
pub const ROWS: [(Action, u32); 22] = [
    (Action::Handbrake, PAD_A),
    (Action::Horn, PAD_X),
    (Action::Lights, PAD_DPAD_LEFT),
    (Action::VehicleExit, PAD_B),
    (Action::Autodrive, PAD_LEFT_THUMB),
    (Action::CombatMode, PAD_Y),
    (Action::Boost, SCRIPT_BOOST),
    (Action::FlyUp, SCRIPT_FLY_UP),
    (Action::FlyDown, SCRIPT_FLY_DOWN),
    (Action::Scanner, PAD_LEFT_SHOULDER),
    (Action::PauseMenu, PAD_START),
    (Action::CameraChange, PAD_DPAD_RIGHT),
    (Action::RearView, PAD_RIGHT_THUMB),
    (Action::CameraReset, PAD_LEFT_THUMB),
    (Action::ZoomIn, PAD_DPAD_UP),
    (Action::ZoomOut, PAD_DPAD_DOWN),
    (Action::RadioMenu, PAD_RIGHT_SHOULDER),
    (Action::RadioToggle, SCRIPT_RADIO_TOGGLE),
    (Action::RadioNext, SCRIPT_RADIO_NEXT),
    (Action::RadioPrevious, SCRIPT_RADIO_PREVIOUS),
    (Action::RadioVolumeUp, SCRIPT_VOLUME_UP),
    (Action::RadioVolumeDown, SCRIPT_VOLUME_DOWN),
];

/// The row with the boost strength slider beneath it.
pub const BOOST_ROW: usize = 6;

/// Where each section starts on the page, its title, and its column.
const SECTIONS: [(usize, &str, usize); 5] = [
    (0, "Driving", 0),
    (7, "Flight", 0),
    (9, "Other", 0),
    (11, "Camera", 1),
    (16, "Radio", 1),
];

/// The title of the section that starts at a row, if one does.
pub fn section_at(row: usize) -> &'static str {
    SECTIONS
        .iter()
        .find(|(start, _, _)| *start == row)
        .map_or("", |(_, title, _)| title)
}

/// The page column a row sits in.
pub fn column_of(row: usize) -> usize {
    SECTIONS
        .iter()
        .rev()
        .find(|(start, _, _)| *start <= row)
        .map_or(0, |(_, _, column)| *column)
}

/// The steering sliders on the page, in order.
pub const SLIDER_COUNT: i32 = 3;
const RANGE_STEP: i64 = 30;
const CURVE_STEP: f64 = 0.1;
const CURVE_RANGE: std::ops::RangeInclusive<f64> = 1.0..=4.0;

/// How far along its range a value is, from 0 to 1.
fn along(value: f64, low: f64, high: f64) -> f64 {
    ((value - low) / (high - low)).clamp(0.0, 1.0)
}

pub fn slider_label(index: i32) -> &'static str {
    match index {
        0 => "Steering range",
        1 => "Center response",
        2 => "Steering speed (next start)",
        _ => "",
    }
}

pub fn slider_value(index: i32, config: &Config) -> String {
    match index {
        0 => format!("{} DEGREES", config.steering_range),
        1 => format!("{:.1}", config.steering.exponent),
        2 => format!(
            "x{}",
            config
                .steering_speed
                .unwrap_or(f64::from(crate::drive::DEFAULT_STEERING_SPEED))
        ),
        _ => String::new(),
    }
}

pub fn slider_fraction(index: i32, config: &Config) -> f64 {
    match index {
        0 => along(
            f64::from(config.steering_range),
            f64::from(*STEERING_RANGE.start()),
            f64::from(*STEERING_RANGE.end()),
        ),
        1 => along(
            config.steering.exponent,
            *CURVE_RANGE.start(),
            *CURVE_RANGE.end(),
        ),
        2 => along(
            config
                .steering_speed
                .unwrap_or(f64::from(crate::drive::DEFAULT_STEERING_SPEED)),
            *STEERING_SPEED_RANGE.start(),
            *STEERING_SPEED_RANGE.end(),
        ),
        _ => 0.0,
    }
}

/// Moves a slider by whole steps, within its limits. Returns true when the value changed.
pub fn slider_adjust(index: i32, steps: i32, config: &mut Config) -> bool {
    let before = config.clone();
    match index {
        0 => {
            let moved = i64::from(config.steering_range) + i64::from(steps) * RANGE_STEP;
            config.steering_range = moved.clamp(
                i64::from(*STEERING_RANGE.start()),
                i64::from(*STEERING_RANGE.end()),
            ) as u32;
        }
        1 => {
            // Whole tenths, so repeated steps don't gather rounding errors in the saved file.
            let moved = config.steering.exponent + f64::from(steps) * CURVE_STEP;
            config.steering.exponent =
                ((moved * 10.0).round() / 10.0).clamp(*CURVE_RANGE.start(), *CURVE_RANGE.end());
        }
        2 => {
            let current = config
                .steering_speed
                .unwrap_or(f64::from(crate::drive::DEFAULT_STEERING_SPEED));
            config.steering_speed = Some(
                (current + f64::from(steps))
                    .clamp(*STEERING_SPEED_RANGE.start(), *STEERING_SPEED_RANGE.end()),
            );
        }
        _ => {}
    }
    *config != before
}

/// How long a row stays lit after its control is pressed, so a dial click can be seen.
const LIT_MS: u64 = 300;

/// A request from the keyboard or mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Up,
    Down,
    Select,
    Clear,
    Close,
}

impl Command {
    /// The number the game script uses for each request.
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

/// The saved names of the functions whose gamepad buttons are in `flags`, for the log.
pub fn fired_names(flags: u32) -> String {
    let names: Vec<&str> = ROWS
        .iter()
        .filter(|(_, flag)| flags & flag != 0)
        .map(|(action, _)| action.key())
        .collect();
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(",")
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

#[derive(Debug)]
enum Mode {
    Browse,
    Listening(ButtonListener),
}

/// What the screen is doing.
#[derive(Debug)]
pub struct Screen {
    selected: usize,
    mode: Mode,
    message: String,
    done: bool,
    lit_until: [u64; ROWS.len()],
}

impl Screen {
    pub fn new() -> Self {
        Self {
            selected: 0,
            mode: Mode::Browse,
            message: String::new(),
            done: false,
            lit_until: [0; ROWS.len()],
        }
    }

    /// True once the player has asked to leave.
    pub fn done(&self) -> bool {
        self.done
    }

    fn action(&self) -> Action {
        ROWS[self.selected].0
    }

    fn label(action: Action) -> String {
        action.label().to_uppercase()
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
                if had_one {
                    self.message = format!("{} CLEARED", Self::label(self.action()));
                }
                had_one
            }
            (Mode::Browse, Command::Close) => {
                self.done = true;
                false
            }
            (Mode::Listening(_), Command::Close) => {
                self.mode = Mode::Browse;
                self.message = "CANCELLED".to_owned();
                false
            }
            _ => false,
        }
    }

    /// Takes the wheel's state: `held` now, and `pressed` for anything pressed since the last call,
    /// including a press too short to still be held. Returns true when `bindings` changed.
    /// While listening, one fresh press sets the binding. Otherwise, pressing lights its row.
    pub fn wheel(&mut self, now_ms: u64, held: u64, pressed: u64, bindings: &mut Bindings) -> bool {
        if let Mode::Listening(listener) = &mut self.mode {
            let sample = sample_of(held | pressed);
            return match listener.update(now_ms, Some(&sample)) {
                ListenOutcome::Waiting => false,
                ListenOutcome::Detected(control) => {
                    self.bind(control, bindings);
                    true
                }
                ListenOutcome::TimedOut => {
                    self.mode = Mode::Browse;
                    self.message = "NO PRESS DETECTED".to_owned();
                    false
                }
            };
        }
        for (row, (action, _)) in ROWS.iter().enumerate() {
            let bit = bindings.get(*action).and_then(bit_for).unwrap_or(0);
            if bit & (held | pressed) != 0 {
                self.lit_until[row] = now_ms + LIT_MS;
            }
        }
        false
    }

    /// Sets the selected function's control, taking it from any function that had it.
    fn bind(&mut self, control: Control, bindings: &mut Bindings) {
        let action = self.action();
        let name = control_name(control);
        self.message = format!("{} SET TO {name}", Self::label(action));
        for other in bindings.conflicts(action, control) {
            bindings.clear(other);
            self.message
                .push_str(&format!(", REMOVED FROM {}", Self::label(other)));
        }
        bindings.assign(action, control);
        self.mode = Mode::Browse;
    }

    pub fn set_message(&mut self, message: String) {
        self.message = message;
    }

    /// Moves the selection to a row, as a mouse hover does. Ignored mid-bind.
    pub fn select(&mut self, row: usize) {
        if matches!(self.mode, Mode::Browse) && row < ROWS.len() && row != self.selected {
            self.selected = row;
            self.message.clear();
        }
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    /// True while a row's control has just been pressed.
    pub fn row_lit(&self, row: usize, now_ms: u64) -> bool {
        self.lit_until.get(row).is_some_and(|until| now_ms < *until)
    }

    /// The function named on a row.
    pub fn row_label(row: usize) -> String {
        ROWS.get(row)
            .map_or_else(String::new, |(action, _)| action.label().to_owned())
    }

    /// What a row's box shows.
    pub fn row_value(&self, row: usize, bindings: &Bindings) -> String {
        let Some((action, _)) = ROWS.get(row) else {
            return String::new();
        };
        if row == self.selected && matches!(self.mode, Mode::Listening(_)) {
            return "PRESS A WHEEL CONTROL".to_owned();
        }
        bindings
            .get(*action)
            .map_or_else(|| "NOT BOUND".to_owned(), control_name)
    }

    /// The line above the hints.
    pub fn status(&self) -> String {
        match &self.mode {
            Mode::Browse => self.message.clone(),
            Mode::Listening(_) => {
                format!("PRESS A WHEEL CONTROL FOR {}", Self::label(self.action()))
            }
        }
    }

    /// The controls available now.
    pub fn hints(&self) -> &'static str {
        match self.mode {
            Mode::Browse => "CLICK OR ENTER  BIND     DELETE  CLEAR     PERIOD  BACK",
            Mode::Listening(_) => "PERIOD  CANCEL",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drive::HAT_SHIFT;

    const LEFT_PADDLE: u64 = 1 << 5;
    const DIAL: u64 = 1 << 21;
    const WHEEL_X: u64 = 1;

    fn usage(usage: u16) -> Control {
        Control::Button(ButtonId {
            page: BUTTON_PAGE,
            usage,
        })
    }

    fn steering_config() -> Config {
        use wd_core::assignment::Bindings;
        use wd_core::calibration::{PedalCalibration, SteeringCalibration};
        use wd_core::config::DeviceId;
        use wd_core::input::AxisId;
        use wd_core::wizard::AxisMap;
        let pedal = PedalCalibration {
            rest: 255,
            full: 0,
            rest_deadzone: 0.0,
            full_deadzone: 0.0,
            exponent: 1.0,
            inverted: false,
        };
        let axis = |usage| AxisId { page: 1, usage };
        Config {
            device: DeviceId {
                vendor: 1,
                product: 2,
            },
            axes: AxisMap {
                steering: axis(0x30),
                throttle: axis(0x31),
                brake: axis(0x32),
            },
            steering: SteeringCalibration {
                left: 0,
                center: 100,
                right: 200,
                deadzone: 0.0,
                exponent: 1.0,
                sensitivity: 1.0,
                inverted: false,
            },
            throttle: pedal,
            brake: pedal,
            bindings: Bindings::default(),
            boost_percent: 25,
            steering_range: 900,
            steering_speed: None,
        }
    }

    #[test]
    fn sliders_step_and_stop_at_their_limits() {
        let mut config = steering_config();
        assert!(slider_adjust(0, -1, &mut config));
        assert_eq!(config.steering_range, 870);
        assert!(!slider_adjust(0, 1000, &mut steering_config()));
        slider_adjust(0, -1000, &mut config);
        assert_eq!(config.steering_range, 180);

        assert!(slider_adjust(1, 3, &mut config));
        assert_eq!(config.steering.exponent, 1.3);
        slider_adjust(1, 1000, &mut config);
        assert_eq!(config.steering.exponent, 4.0);
        assert!(!slider_adjust(1, 1, &mut config));

        assert!(slider_adjust(2, 1, &mut config));
        assert_eq!(config.steering_speed, Some(3.0));
        slider_adjust(2, 1000, &mut config);
        assert_eq!(config.steering_speed, Some(10.0));
        config.validate().unwrap();
    }

    #[test]
    fn sliders_show_their_values_and_ignore_unknown_numbers() {
        let mut config = steering_config();
        assert_eq!(slider_value(0, &config), "900 DEGREES");
        assert_eq!(slider_value(1, &config), "1.0");
        assert_eq!(slider_value(2, &config), "x2");
        assert_eq!(slider_fraction(0, &config), 1.0);
        assert_eq!(slider_fraction(1, &config), 0.0);
        assert_eq!(slider_value(9, &config), "");
        assert!(!slider_adjust(9, 1, &mut config));
    }

    #[test]
    fn selection_moves_and_wraps() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        screen.command(Command::Up, 0, 0, &mut bindings);
        assert_eq!(screen.selected(), ROWS.len() - 1);
        screen.command(Command::Down, 0, 0, &mut bindings);
        assert_eq!(screen.selected(), 0);
    }

    #[test]
    fn one_press_sets_the_binding() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        screen.command(Command::Select, 0, 0, &mut bindings);
        assert!(screen.wheel(10, LEFT_PADDLE, LEFT_PADDLE, &mut bindings));
        assert_eq!(bindings.get(Action::Handbrake), Some(usage(6)));
        assert!(matches!(screen.mode, Mode::Browse));
        assert_eq!(screen.status(), "HANDBRAKE SET TO LEFT PADDLE");
    }

    #[test]
    fn a_control_held_before_listening_needs_a_release() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        screen.command(Command::Select, 0, LEFT_PADDLE, &mut bindings);
        assert!(!screen.wheel(10, LEFT_PADDLE, 0, &mut bindings));
        assert!(!screen.wheel(20, 0, 0, &mut bindings));
        assert!(screen.wheel(30, LEFT_PADDLE, LEFT_PADDLE, &mut bindings));
    }

    #[test]
    fn a_pulse_too_short_to_be_held_is_still_caught() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        screen.command(Command::Select, 0, 0, &mut bindings);
        assert!(screen.wheel(10, 0, DIAL, &mut bindings));
        assert_eq!(bindings.get(Action::Handbrake), Some(usage(22)));
    }

    #[test]
    fn listening_gives_up_and_changes_nothing() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        screen.command(Command::Select, 0, 0, &mut bindings);
        assert!(!screen.wheel(LISTEN_TIMEOUT_MS, 0, 0, &mut bindings));
        assert!(matches!(screen.mode, Mode::Browse));
        assert_eq!(bindings.iter().count(), 0);
        assert_eq!(screen.status(), "NO PRESS DETECTED");
    }

    #[test]
    fn cancelling_returns_without_closing_or_binding() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        screen.command(Command::Select, 0, 0, &mut bindings);
        screen.command(Command::Close, 10, 0, &mut bindings);
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
    fn binding_a_used_control_moves_it_and_says_so() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        bindings.assign(Action::Horn, usage(6));
        screen.command(Command::Select, 0, 0, &mut bindings);
        screen.wheel(10, LEFT_PADDLE, LEFT_PADDLE, &mut bindings);
        assert_eq!(bindings.get(Action::Handbrake), Some(usage(6)));
        assert_eq!(bindings.get(Action::Horn), None);
        assert_eq!(
            screen.status(),
            "HANDBRAKE SET TO LEFT PADDLE, REMOVED FROM HORN"
        );
    }

    #[test]
    fn clear_reports_a_change_only_when_there_was_a_binding() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        assert!(!screen.command(Command::Clear, 0, 0, &mut bindings));
        bindings.assign(Action::Handbrake, usage(6));
        assert!(screen.command(Command::Clear, 0, 0, &mut bindings));
        assert_eq!(bindings.get(Action::Handbrake), None);
        assert_eq!(screen.status(), "HANDBRAKE CLEARED");
    }

    #[test]
    fn the_wheel_never_runs_the_screen() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        let everything = WHEEL_X | 0b110 | (HAT_UP | HAT_DOWN) << HAT_SHIFT;
        assert!(!screen.wheel(0, everything, everything, &mut bindings));
        assert_eq!(screen.selected(), 0);
        assert!(matches!(screen.mode, Mode::Browse));
        assert!(!screen.done());
    }

    #[test]
    fn any_control_can_be_bound_including_ones_that_used_to_run_the_screen() {
        for bit in [WHEEL_X, 0b10, 0b100, HAT_UP << HAT_SHIFT] {
            let mut screen = Screen::new();
            let mut bindings = Bindings::default();
            screen.command(Command::Select, 0, 0, &mut bindings);
            assert!(screen.wheel(10, bit, bit, &mut bindings));
            let bound = bindings.get(Action::Handbrake);
            assert_eq!(bound.and_then(bit_for), Some(bit));
        }
    }

    #[test]
    fn pressing_a_bound_control_lights_its_row_for_a_moment() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        bindings.assign(Action::Horn, usage(22));
        assert!(!screen.row_lit(1, 100));
        screen.wheel(100, 0, DIAL, &mut bindings);
        assert!(screen.row_lit(1, 100 + LIT_MS - 1));
        assert!(!screen.row_lit(1, 100 + LIT_MS));
        assert!(!screen.row_lit(0, 100));
        assert!(!screen.row_lit(99, 100));
    }

    #[test]
    fn a_row_shows_its_binding_or_that_it_is_listening() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        assert_eq!(screen.row_value(0, &bindings), "NOT BOUND");
        bindings.assign(Action::Horn, usage(6));
        assert_eq!(screen.row_value(1, &bindings), "LEFT PADDLE");
        screen.command(Command::Select, 0, 0, &mut bindings);
        assert_eq!(screen.row_value(0, &bindings), "PRESS A WHEEL CONTROL");
        assert_eq!(screen.row_value(1, &bindings), "LEFT PADDLE");
        assert_eq!(screen.row_value(99, &bindings), "");
    }

    #[test]
    fn hovering_selects_except_during_a_bind() {
        let mut screen = Screen::new();
        let mut bindings = Bindings::default();
        screen.select(3);
        assert_eq!(screen.selected(), 3);
        screen.select(99);
        assert_eq!(screen.selected(), 3);
        screen.command(Command::Select, 0, 0, &mut bindings);
        screen.select(1);
        assert_eq!(screen.selected(), 3);
    }

    #[test]
    fn sections_start_where_they_should() {
        assert_eq!(section_at(0), "Driving");
        assert_eq!(section_at(7), "Flight");
        assert_eq!(section_at(11), "Camera");
        assert_eq!(section_at(16), "Radio");
        assert_eq!(section_at(1), "");
        assert_eq!(ROWS[7].0, Action::FlyUp);
        assert_eq!(ROWS[11].0, Action::CameraChange);
        assert_eq!(ROWS[16].0, Action::RadioMenu);
        assert_eq!(ROWS[BOOST_ROW].0, Action::Boost);
    }

    #[test]
    fn rows_fall_in_their_section_column() {
        assert_eq!(column_of(0), 0);
        assert_eq!(column_of(10), 0);
        assert_eq!(column_of(11), 1);
        assert_eq!(column_of(21), 1);
    }

    #[test]
    fn every_route_has_one_bit_and_gamepad_routes_fit_a_gamepad() {
        for (_, route) in ROWS {
            assert_eq!(route.count_ones(), 1);
        }
        let pad_only: u32 = ROWS
            .iter()
            .filter(|(action, _)| *action == Action::Handbrake)
            .map(|(_, route)| *route)
            .sum();
        assert_eq!(pad_only >> SCRIPT_SHIFT, 0);
        assert_eq!(SCRIPT_RADIO_TOGGLE >> SCRIPT_SHIFT, 1);
    }

    #[test]
    fn fired_functions_are_named_for_the_log() {
        assert_eq!(fired_names(0), "none");
        assert_eq!(fired_names(PAD_X), "horn");
        assert_eq!(fired_names(PAD_A | PAD_X), "handbrake,horn");
        assert_eq!(fired_names(SCRIPT_VOLUME_UP), "radio_volume_up");
    }

    #[test]
    fn each_function_appears_once() {
        let actions: BTreeSet<Action> = ROWS.iter().map(|(action, _)| *action).collect();
        assert_eq!(actions.len(), ROWS.len());
    }

    #[test]
    fn names_come_from_the_map_and_unknown_numbers_stay_numbers() {
        assert_eq!(control_name(usage(8)), "L2");
        assert_eq!(control_name(usage(11)), "R3");
        assert_eq!(control_name(usage(13)), "BUTTON 13");
        assert_eq!(control_name(Control::Hat(4)), "D-PAD DOWN");
    }
}

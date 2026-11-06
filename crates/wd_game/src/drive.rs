//! Turns calibrated wheel values into gamepad values, and decides when they may apply.
//! Free of Windows and game types so it can be tested without either.

use wd_core::config::Config;
use wd_core::input::Control;

/// With no update from the game's script for this long, the wheel stops driving.
pub const HEARTBEAT_LIMIT_US: u64 = 500_000;

/// The gamepad values the wheel controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Output {
    pub thumb_x: i16,
    pub left_trigger: u8,
    pub right_trigger: u8,
}

/// Where the game's steering action leaves zero and reaches full, as a fraction of stick travel.
/// Fitted to the game's `TurnX` action on patch 2.5; see docs/records/P3-cleanup-test-build.md.
const STICK_DEAD_END: f64 = 0.35;
const STICK_FULL: f64 = 0.90;

/// The steering speed multiplier when no valid choice exists. The only value proven to keep up.
pub const DEFAULT_STEERING_SPEED: f32 = 10.0;
/// No value above the highest one tried, and none below the game's own speed.
const STEERING_SPEED_RANGE: std::ops::RangeInclusive<f32> = 1.0..=10.0;

/// The multiplier chosen in a settings file's text, if it holds one valid number.
pub fn parse_steering_speed(text: &str) -> Option<f32> {
    text.trim()
        .parse::<f32>()
        .ok()
        .filter(|value| STEERING_SPEED_RANGE.contains(value))
}

/// Undoes the game's vehicle deadzone, so wheel position maps straight to the game's action.
/// Zero stays exactly zero. Any other intent starts at the edge of the dead range.
fn compensate(intent: f64) -> f64 {
    if intent == 0.0 {
        return 0.0;
    }
    let size = intent.abs().min(1.0);
    let stick = if size >= 1.0 {
        1.0
    } else {
        STICK_DEAD_END + (STICK_FULL - STICK_DEAD_END) * size
    };
    stick.copysign(intent)
}

/// Steering intent, -1.0..=1.0, to a stick value. Centre is exactly 0.
fn to_thumb(intent: f64) -> i16 {
    (intent.clamp(-1.0, 1.0) * f64::from(i16::MAX)).round() as i16
}

/// Pedal intent, 0.0..=1.0, to a trigger value.
fn to_trigger(intent: f64) -> u8 {
    (intent.clamp(0.0, 1.0) * f64::from(u8::MAX)).round() as u8
}

/// The steering choices that can change while the game runs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tuning {
    pub range_scale: f64,
    pub exponent: f64,
}

impl Tuning {
    pub fn of(config: &Config) -> Self {
        Self {
            range_scale: f64::from(wd_core::config::GHUB_NEUTRAL_RANGE)
                / f64::from(config.steering_range),
            exponent: config.steering.exponent,
        }
    }
}

/// Gamepad values for raw steering, throttle and brake readings. The wheel reports nothing until
/// something moves, so a control with no reading yet is neutral: a raw default could otherwise
/// read as a fully pressed pedal.
pub fn output(
    config: &Config,
    [steering, throttle, brake]: [Option<i32>; 3],
    tuning: Tuning,
) -> Output {
    Output {
        thumb_x: steering.map_or(0, |raw| {
            to_thumb(compensate(config.steering.respond(
                raw,
                tuning.range_scale,
                tuning.exponent,
            )))
        }),
        right_trigger: throttle.map_or(0, |raw| to_trigger(config.throttle.normalize(raw))),
        left_trigger: brake.map_or(0, |raw| to_trigger(config.brake.normalize(raw))),
    }
}

/// Control bits: numbered buttons from the lowest bit, then the D-pad's four directions on top.
pub const HAT_SHIFT: u32 = 60;
pub const HAT_UP: u64 = 1;
pub const HAT_RIGHT: u64 = 2;
pub const HAT_DOWN: u64 = 4;
pub const HAT_LEFT: u64 = 8;
const BUTTON_BITS: u64 = (1 << HAT_SHIFT) - 1;
const BUTTON_PAGE: u16 = 0x09;

/// The control bits for a wheel report. A diagonal counts as both its directions. A hat value
/// outside its reported 0 to 7 means not pressed.
pub fn control_bits(buttons: u64, hat: Option<i64>) -> u64 {
    let directions = match hat {
        Some(0) => HAT_UP,
        Some(1) => HAT_UP | HAT_RIGHT,
        Some(2) => HAT_RIGHT,
        Some(3) => HAT_RIGHT | HAT_DOWN,
        Some(4) => HAT_DOWN,
        Some(5) => HAT_DOWN | HAT_LEFT,
        Some(6) => HAT_LEFT,
        Some(7) => HAT_LEFT | HAT_UP,
        _ => 0,
    };
    (buttons & BUTTON_BITS) | directions << HAT_SHIFT
}

/// The bit for a bound control, when it has one.
pub fn bit_for(control: Control) -> Option<u64> {
    match control {
        Control::Button(button) if button.page == BUTTON_PAGE => (1..=u64::from(HAT_SHIFT))
            .contains(&u64::from(button.usage))
            .then(|| 1 << (button.usage - 1)),
        Control::Button(_) => None,
        Control::Hat(0) => Some(HAT_UP << HAT_SHIFT),
        Control::Hat(2) => Some(HAT_RIGHT << HAT_SHIFT),
        Control::Hat(4) => Some(HAT_DOWN << HAT_SHIFT),
        Control::Hat(6) => Some(HAT_LEFT << HAT_SHIFT),
        Control::Hat(_) => None,
    }
}

/// Decides which controls count at a poll, from those held, those pressed since the last poll and
/// those waiting for a release. Returns the controls that count and the new waiting set.
/// Anything held while the wheel may not apply must be released first, so that it never replays
/// on resume. A short press still counts for one poll, because a dial click can end between polls.
pub fn gate_controls(held: u64, pressed: u64, waiting: u64, may_apply: bool) -> (u64, u64) {
    if !may_apply {
        return (0, held);
    }
    let waiting = waiting & held;
    ((held | pressed) & !waiting, waiting)
}

/// The routes pressed by the controls that count, from (control bit, route) pairs.
pub fn route_flags(controls: u64, pairs: impl Iterator<Item = (u64, u32)>) -> u32 {
    pairs.fold(0, |flags, (bit, flag)| {
        if bit != 0 && controls & bit != 0 {
            flags | flag
        } else {
            flags
        }
    })
}

/// The same stick and triggers also walk and fire on foot, so the wheel only applies while the
/// player is driving, the game has focus and is not paused, and the script's updates are fresh.
/// Focus is checked here because the reader has to receive input in the background.
pub fn may_apply(
    connected: bool,
    driving: bool,
    focused: bool,
    paused: bool,
    heartbeat_age_us: u64,
) -> bool {
    connected && driving && focused && !paused && heartbeat_age_us <= HEARTBEAT_LIMIT_US
}

/// Uses the wheel's value for each control unless it is at rest, so a real gamepad still works.
pub fn merge(pad: Output, wheel: Output) -> Output {
    Output {
        thumb_x: if wheel.thumb_x != 0 {
            wheel.thumb_x
        } else {
            pad.thumb_x
        },
        left_trigger: wheel.left_trigger.max(pad.left_trigger),
        right_trigger: wheel.right_trigger.max(pad.right_trigger),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wd_core::assignment::Bindings;
    use wd_core::calibration::{PedalCalibration, SteeringCalibration};
    use wd_core::config::DeviceId;
    use wd_core::input::AxisId;
    use wd_core::wizard::AxisMap;

    /// Pedals that read 0 when fully pressed, the case where a raw default would be dangerous.
    fn config() -> Config {
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

    fn neutral() -> Tuning {
        Tuning {
            range_scale: 1.0,
            exponent: 1.0,
        }
    }

    #[test]
    fn a_saved_range_and_curve_become_the_tuning() {
        let mut config = config();
        assert_eq!(Tuning::of(&config), neutral());
        config.steering_range = 225;
        config.steering.exponent = 2.0;
        assert_eq!(
            Tuning::of(&config),
            Tuning {
                range_scale: 4.0,
                exponent: 2.0
            }
        );
    }

    #[test]
    fn the_tuning_changes_the_stick() {
        let wide = output(&config(), [Some(125), None, None], neutral()).thumb_x;
        let narrow = Tuning {
            range_scale: 4.0,
            exponent: 1.0,
        };
        assert!(wide < i16::MAX);
        assert_eq!(
            output(&config(), [Some(125), None, None], narrow).thumb_x,
            i16::MAX
        );
    }

    fn wheel_button(usage: u16) -> Control {
        Control::Button(wd_core::input::ButtonId { page: 9, usage })
    }

    #[test]
    fn a_report_becomes_control_bits() {
        assert_eq!(control_bits(0b101, None), 0b101);
        assert_eq!(control_bits(0, Some(2)), HAT_RIGHT << HAT_SHIFT);
        assert_eq!(control_bits(0, Some(1)), (HAT_UP | HAT_RIGHT) << HAT_SHIFT);
        assert_eq!(control_bits(0, Some(8)), 0);
        assert_eq!(control_bits(0, Some(15)), 0);
    }

    #[test]
    fn a_number_too_high_cannot_look_like_the_d_pad() {
        assert_eq!(control_bits(1u64 << 63, None), 0);
    }

    #[test]
    fn bound_controls_find_their_bits() {
        assert_eq!(bit_for(wheel_button(1)), Some(1));
        assert_eq!(bit_for(wheel_button(25)), Some(1 << 24));
        assert_eq!(bit_for(wheel_button(0)), None);
        assert_eq!(bit_for(wheel_button(61)), None);
        assert_eq!(bit_for(Control::Hat(4)), Some(HAT_DOWN << HAT_SHIFT));
        assert_eq!(bit_for(Control::Hat(1)), None);
    }

    #[test]
    fn a_held_control_does_not_replay_when_the_wheel_returns() {
        let (_, waiting) = gate_controls(0b1, 0, 0, false);
        assert_eq!(waiting, 0b1);
        let (counted, waiting) = gate_controls(0b1, 0, waiting, true);
        assert_eq!(counted, 0);
        assert_eq!(waiting, 0b1);
        let (counted, waiting) = gate_controls(0, 0, waiting, true);
        assert_eq!(counted, 0);
        assert_eq!(waiting, 0);
        let (counted, _) = gate_controls(0b1, 0b1, waiting, true);
        assert_eq!(counted, 0b1);
    }

    #[test]
    fn nothing_counts_while_the_wheel_may_not_apply() {
        assert_eq!(gate_controls(0b11, 0b11, 0, false).0, 0);
    }

    #[test]
    fn a_short_press_counts_once() {
        assert_eq!(gate_controls(0, 0b100, 0, true).0, 0b100);
        assert_eq!(gate_controls(0, 0, 0, true).0, 0);
    }

    #[test]
    fn controls_press_their_gamepad_buttons() {
        let pairs = [(0b01, 0x1000), (0b10, 0x4000), (0, 0x0004)];
        assert_eq!(route_flags(0b01, pairs.into_iter()), 0x1000);
        assert_eq!(route_flags(0b11, pairs.into_iter()), 0x5000);
        assert_eq!(route_flags(0, pairs.into_iter()), 0);
    }

    #[test]
    fn an_unbound_function_presses_nothing() {
        assert_eq!(route_flags(u64::MAX, [(0, 0x0004)].into_iter()), 0);
    }

    #[test]
    fn a_valid_steering_speed_is_read_with_surrounding_space() {
        assert_eq!(
            parse_steering_speed(
                " 4
"
            ),
            Some(4.0)
        );
        assert_eq!(parse_steering_speed("1"), Some(1.0));
        assert_eq!(parse_steering_speed("10.0"), Some(10.0));
    }

    #[test]
    fn an_invalid_steering_speed_is_refused() {
        for text in ["", "fast", "0.5", "10.5", "-3", "NaN", "inf", "4 5"] {
            assert_eq!(parse_steering_speed(text), None, "{text:?}");
        }
    }

    #[test]
    fn controls_with_no_reading_are_neutral() {
        assert_eq!(
            output(&config(), [None, None, None], neutral()),
            Output::default()
        );
    }

    #[test]
    fn a_pressed_reading_gives_full_output() {
        let result = output(&config(), [Some(200), Some(0), Some(255)], neutral());
        assert_eq!(result.thumb_x, i16::MAX);
        assert_eq!(result.right_trigger, u8::MAX);
        assert_eq!(result.left_trigger, 0);
    }

    #[test]
    fn compensation_keeps_centre_and_full_lock() {
        assert_eq!(compensate(0.0), 0.0);
        assert_eq!(compensate(1.0), 1.0);
        assert_eq!(compensate(-1.0), -1.0);
    }

    #[test]
    fn small_turns_start_at_the_edge_of_the_dead_range() {
        assert!(compensate(0.001) >= STICK_DEAD_END);
        assert!(compensate(0.001) < STICK_DEAD_END + 0.01);
    }

    #[test]
    fn compensation_rises_steadily_and_is_symmetric() {
        let mut previous = 0.0;
        for step in 1..=100 {
            let intent = f64::from(step) / 100.0;
            let stick = compensate(intent);
            assert!(stick > previous);
            assert_eq!(compensate(-intent), -stick);
            previous = stick;
        }
    }

    #[test]
    fn stick_values_cover_the_range_and_centre_is_zero() {
        assert_eq!(to_thumb(0.0), 0);
        assert_eq!(to_thumb(1.0), i16::MAX);
        assert_eq!(to_thumb(-1.0), -i16::MAX);
    }

    #[test]
    fn out_of_range_intent_is_limited() {
        assert_eq!(to_thumb(5.0), i16::MAX);
        assert_eq!(to_trigger(-1.0), 0);
        assert_eq!(to_trigger(5.0), u8::MAX);
    }

    #[test]
    fn wheel_only_applies_while_driving() {
        assert!(may_apply(true, true, true, false, 0));
        assert!(!may_apply(true, false, true, false, 0));
    }

    #[test]
    fn losing_focus_stops_the_wheel() {
        assert!(!may_apply(true, true, false, false, 0));
    }

    #[test]
    fn a_press_made_while_closed_never_counts_when_it_opens() {
        // The poll that sees a context change is closed, however the press arrived.
        let (counted, waiting) = gate_controls(0b1, 0b1, 0, false);
        assert_eq!(counted, 0);
        assert_eq!(gate_controls(0b1, 0, waiting, true).0, 0);
    }

    #[test]
    fn pause_and_disconnect_stop_the_wheel() {
        assert!(!may_apply(true, true, true, true, 0));
        assert!(!may_apply(false, true, true, false, 0));
    }

    #[test]
    fn a_stale_script_stops_the_wheel() {
        assert!(may_apply(true, true, true, false, HEARTBEAT_LIMIT_US));
        assert!(!may_apply(true, true, true, false, HEARTBEAT_LIMIT_US + 1));
    }

    #[test]
    fn a_gamepad_works_when_the_wheel_is_at_rest() {
        let pad = Output {
            thumb_x: 900,
            left_trigger: 10,
            right_trigger: 20,
        };
        assert_eq!(merge(pad, Output::default()), pad);
    }

    #[test]
    fn a_moved_wheel_takes_over_the_stick() {
        let pad = Output {
            thumb_x: 900,
            left_trigger: 0,
            right_trigger: 0,
        };
        let wheel = Output {
            thumb_x: -100,
            left_trigger: 5,
            right_trigger: 0,
        };
        assert_eq!(merge(pad, wheel), wheel);
    }
}

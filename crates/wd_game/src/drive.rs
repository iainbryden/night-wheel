//! Turns calibrated wheel values into gamepad values, and decides when they may apply.
//! Free of Windows and game types so it can be tested without either.

use wd_core::config::Config;

/// With no update from the game's script for this long, the wheel stops driving.
pub const HEARTBEAT_LIMIT_US: u64 = 500_000;

/// The gamepad values the wheel controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Output {
    pub thumb_x: i16,
    pub left_trigger: u8,
    pub right_trigger: u8,
}

/// Steering intent, -1.0..=1.0, to a stick value. Centre is exactly 0.
fn to_thumb(intent: f64) -> i16 {
    (intent.clamp(-1.0, 1.0) * f64::from(i16::MAX)).round() as i16
}

/// Pedal intent, 0.0..=1.0, to a trigger value.
fn to_trigger(intent: f64) -> u8 {
    (intent.clamp(0.0, 1.0) * f64::from(u8::MAX)).round() as u8
}

/// Gamepad values for raw steering, throttle and brake readings. The wheel reports nothing until
/// something moves, so a control with no reading yet is neutral: a raw default could otherwise
/// read as a fully pressed pedal.
pub fn output(config: &Config, [steering, throttle, brake]: [Option<i32>; 3]) -> Output {
    Output {
        thumb_x: steering.map_or(0, |raw| to_thumb(config.steering.normalize(raw))),
        right_trigger: throttle.map_or(0, |raw| to_trigger(config.throttle.normalize(raw))),
        left_trigger: brake.map_or(0, |raw| to_trigger(config.brake.normalize(raw))),
    }
}

/// The same stick and triggers also walk and fire on foot, so the wheel only applies while
/// the player is driving, the game is not paused, and the script's updates are still fresh.
pub fn may_apply(connected: bool, driving: bool, paused: bool, heartbeat_age_us: u64) -> bool {
    connected && driving && !paused && heartbeat_age_us <= HEARTBEAT_LIMIT_US
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
        }
    }

    #[test]
    fn controls_with_no_reading_are_neutral() {
        assert_eq!(output(&config(), [None, None, None]), Output::default());
    }

    #[test]
    fn a_pressed_reading_gives_full_output() {
        let result = output(&config(), [Some(200), Some(0), Some(255)]);
        assert_eq!(result.thumb_x, i16::MAX);
        assert_eq!(result.right_trigger, u8::MAX);
        assert_eq!(result.left_trigger, 0);
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
        assert!(may_apply(true, true, false, 0));
        assert!(!may_apply(true, false, false, 0));
    }

    #[test]
    fn pause_and_disconnect_stop_the_wheel() {
        assert!(!may_apply(true, true, true, 0));
        assert!(!may_apply(false, true, false, 0));
    }

    #[test]
    fn a_stale_script_stops_the_wheel() {
        assert!(may_apply(true, true, false, HEARTBEAT_LIMIT_US));
        assert!(!may_apply(true, true, false, HEARTBEAT_LIMIT_US + 1));
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

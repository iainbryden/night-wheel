//! Converts calibrated raw controls into driver intent.
//!
//! Raw polarity is not assumed. A steering axis may rise or fall from left to
//! right, and a pedal's rest value may be above or below its full value.
//!
//! The validation limits below only reject nonsense. They are provisional, and
//! are not measured driving values or recommended settings.

use std::fmt;

const MAX_DEADZONE: f64 = 0.95;
const EXPONENT_RANGE: (f64, f64) = (0.1, 10.0);
const SENSITIVITY_RANGE: (f64, f64) = (0.01, 10.0);

/// Why a calibration was rejected.
#[derive(Debug, Clone, PartialEq)]
pub enum CalibrationError {
    /// A span between two calibration points has zero length.
    ZeroSpan(&'static str),
    /// The steering centre is not strictly between the two ends.
    CenterOutsideRange,
    /// A number is NaN or infinite.
    NotFinite(&'static str),
    /// A number is outside its accepted limits.
    OutOfRange(&'static str),
}

impl fmt::Display for CalibrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroSpan(what) => write!(f, "{what} has no usable travel"),
            Self::CenterOutsideRange => {
                write!(f, "steering centre must lie between left and right")
            }
            Self::NotFinite(what) => write!(f, "{what} must be a finite number"),
            Self::OutOfRange(what) => write!(f, "{what} is out of range"),
        }
    }
}

impl std::error::Error for CalibrationError {}

fn check_range(
    value: f64,
    (min, max): (f64, f64),
    name: &'static str,
) -> Result<(), CalibrationError> {
    if !value.is_finite() {
        return Err(CalibrationError::NotFinite(name));
    }
    if value < min || value > max {
        return Err(CalibrationError::OutOfRange(name));
    }
    Ok(())
}

/// Fraction of the way from `from` to `to`, clamped to 0.0..=1.0.
fn travel(raw: i32, from: i32, to: i32) -> f64 {
    let span = f64::from(to) - f64::from(from);
    ((f64::from(raw) - f64::from(from)) / span).clamp(0.0, 1.0)
}

/// Steering axis calibration and response.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteeringCalibration {
    /// Raw value at full left lock.
    pub left: i32,
    /// Raw value at centre.
    pub center: i32,
    /// Raw value at full right lock.
    pub right: i32,
    /// Fraction of each side's travel ignored around centre.
    pub deadzone: f64,
    /// Response curve exponent. 1.0 is linear.
    pub exponent: f64,
    /// Output multiplier applied after the curve. The result is still limited to -1.0..=1.0.
    pub sensitivity: f64,
    /// Swaps the output direction.
    pub inverted: bool,
}

impl SteeringCalibration {
    /// Checks every field.
    pub fn validate(&self) -> Result<(), CalibrationError> {
        if self.left == self.center || self.right == self.center {
            return Err(CalibrationError::ZeroSpan("steering"));
        }
        let rising = self.left < self.center && self.center < self.right;
        let falling = self.left > self.center && self.center > self.right;
        if !rising && !falling {
            return Err(CalibrationError::CenterOutsideRange);
        }
        check_range(self.deadzone, (0.0, MAX_DEADZONE), "steering deadzone")?;
        check_range(self.exponent, EXPONENT_RANGE, "steering exponent")?;
        check_range(self.sensitivity, SENSITIVITY_RANGE, "steering sensitivity")?;
        Ok(())
    }

    /// Converts a raw value to steering intent: -1.0 is full left, 1.0 is full right.
    ///
    /// The calibration must have passed [`Self::validate`].
    pub fn normalize(&self, raw: i32) -> f64 {
        self.respond(raw, 1.0, self.exponent)
    }

    /// Like [`Self::normalize`], with a range and curve chosen in the game. Full lock arrives at
    /// `1 / range_scale` of the wheel's travel, and the curve then shapes that whole span.
    pub fn respond(&self, raw: i32, range_scale: f64, exponent: f64) -> f64 {
        let rising = self.right > self.left;
        let toward_right = if rising {
            raw >= self.center
        } else {
            raw <= self.center
        };
        let (end, side) = if toward_right {
            (self.right, 1.0)
        } else {
            (self.left, -1.0)
        };
        let moved = (travel(raw, self.center, end) * range_scale).min(1.0);
        let shaped = if moved <= self.deadzone {
            0.0
        } else {
            ((moved - self.deadzone) / (1.0 - self.deadzone)).powf(exponent)
        };
        let direction = if self.inverted { -side } else { side };
        (shaped * self.sensitivity * direction).clamp(-1.0, 1.0)
    }
}

/// Pedal axis calibration and response.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PedalCalibration {
    /// Raw value with the pedal released.
    pub rest: i32,
    /// Raw value with the pedal fully pressed.
    pub full: i32,
    /// Fraction of travel ignored at the released end.
    pub rest_deadzone: f64,
    /// Fraction of travel ignored at the pressed end.
    pub full_deadzone: f64,
    /// Response curve exponent. 1.0 is linear.
    pub exponent: f64,
    /// Swaps the output so that released reads 1.0 and pressed reads 0.0.
    pub inverted: bool,
}

impl PedalCalibration {
    /// Checks every field.
    pub fn validate(&self) -> Result<(), CalibrationError> {
        if self.rest == self.full {
            return Err(CalibrationError::ZeroSpan("pedal"));
        }
        check_range(
            self.rest_deadzone,
            (0.0, MAX_DEADZONE),
            "pedal rest deadzone",
        )?;
        check_range(
            self.full_deadzone,
            (0.0, MAX_DEADZONE),
            "pedal full deadzone",
        )?;
        if self.rest_deadzone + self.full_deadzone >= 1.0 {
            return Err(CalibrationError::ZeroSpan("pedal usable range"));
        }
        check_range(self.exponent, EXPONENT_RANGE, "pedal exponent")?;
        Ok(())
    }

    /// Converts a raw value to pedal intent: 0.0 is released, 1.0 is fully pressed.
    ///
    /// The calibration must have passed [`Self::validate`].
    pub fn normalize(&self, raw: i32) -> f64 {
        let moved = travel(raw, self.rest, self.full);
        let usable = 1.0 - self.rest_deadzone - self.full_deadzone;
        let shaped = ((moved - self.rest_deadzone) / usable)
            .clamp(0.0, 1.0)
            .powf(self.exponent);
        if self.inverted { 1.0 - shaped } else { shaped }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-9;

    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < EPSILON,
            "{actual} is not {expected}"
        );
    }

    fn steering() -> SteeringCalibration {
        SteeringCalibration {
            left: 0,
            center: 100,
            right: 300,
            deadzone: 0.0,
            exponent: 1.0,
            sensitivity: 1.0,
            inverted: false,
        }
    }

    fn pedal() -> PedalCalibration {
        PedalCalibration {
            rest: 1000,
            full: 0,
            rest_deadzone: 0.0,
            full_deadzone: 0.0,
            exponent: 1.0,
            inverted: false,
        }
    }

    #[test]
    fn steering_hits_center_and_both_ends() {
        let s = steering();
        s.validate().unwrap();
        close(s.normalize(0), -1.0);
        close(s.normalize(100), 0.0);
        close(s.normalize(300), 1.0);
    }

    #[test]
    fn steering_scales_each_side_separately_for_an_off_center_wheel() {
        let s = steering();
        close(s.normalize(50), -0.5);
        close(s.normalize(200), 0.5);
    }

    #[test]
    fn steering_works_when_raw_values_fall_from_left_to_right() {
        let s = SteeringCalibration {
            left: 300,
            center: 100,
            right: 0,
            ..steering()
        };
        s.validate().unwrap();
        close(s.normalize(300), -1.0);
        close(s.normalize(100), 0.0);
        close(s.normalize(0), 1.0);
        close(s.normalize(200), -0.5);
    }

    #[test]
    fn steering_clamps_values_beyond_calibration() {
        let s = steering();
        close(s.normalize(-50), -1.0);
        close(s.normalize(1000), 1.0);
    }

    #[test]
    fn steering_handles_the_full_raw_range_without_overflow() {
        let s = SteeringCalibration {
            left: i32::MIN,
            center: 0,
            right: i32::MAX,
            ..steering()
        };
        s.validate().unwrap();
        close(s.normalize(i32::MIN), -1.0);
        close(s.normalize(i32::MAX), 1.0);
    }

    #[test]
    fn steering_deadzone_zeroes_center_and_still_reaches_full() {
        let s = SteeringCalibration {
            deadzone: 0.5,
            ..steering()
        };
        close(s.normalize(200), 0.0);
        close(s.normalize(250), 0.5);
        close(s.normalize(300), 1.0);
        close(s.normalize(0), -1.0);
    }

    #[test]
    fn steering_exponent_shapes_midrange_only() {
        let s = SteeringCalibration {
            exponent: 2.0,
            ..steering()
        };
        close(s.normalize(200), 0.25);
        close(s.normalize(50), -0.25);
        close(s.normalize(300), 1.0);
    }

    #[test]
    fn a_narrower_range_reaches_full_lock_sooner_and_stays_limited() {
        let steering = steering();
        let quarter = steering.center + (steering.right - steering.center) / 4;
        assert!((steering.respond(quarter, 4.0, 1.0) - 1.0).abs() < 0.001);
        assert_eq!(steering.respond(steering.right, 4.0, 1.0), 1.0);
        assert!((steering.respond(quarter, 2.0, 1.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn the_curve_softens_centre_but_keeps_full_lock() {
        let steering = steering();
        let quarter = steering.center + (steering.right - steering.center) / 4;
        assert!((steering.respond(quarter, 1.0, 2.0) - 0.0625).abs() < 0.001);
        assert_eq!(steering.respond(steering.right, 1.0, 2.0), 1.0);
        assert_eq!(steering.respond(steering.center, 4.0, 2.0), 0.0);
    }

    #[test]
    fn steering_sensitivity_scales_and_output_stays_limited() {
        let s = SteeringCalibration {
            sensitivity: 2.0,
            ..steering()
        };
        close(s.normalize(150), 0.5);
        close(s.normalize(300), 1.0);
        close(s.normalize(0), -1.0);
    }

    #[test]
    fn steering_inversion_swaps_direction() {
        let s = SteeringCalibration {
            inverted: true,
            ..steering()
        };
        close(s.normalize(0), 1.0);
        close(s.normalize(300), -1.0);
    }

    #[test]
    fn steering_rejects_zero_span() {
        let same_left = SteeringCalibration {
            left: 100,
            ..steering()
        };
        let same_right = SteeringCalibration {
            right: 100,
            ..steering()
        };
        assert_eq!(
            same_left.validate(),
            Err(CalibrationError::ZeroSpan("steering"))
        );
        assert_eq!(
            same_right.validate(),
            Err(CalibrationError::ZeroSpan("steering"))
        );
    }

    #[test]
    fn steering_rejects_center_outside_range() {
        let s = SteeringCalibration {
            center: 500,
            ..steering()
        };
        assert_eq!(s.validate(), Err(CalibrationError::CenterOutsideRange));
    }

    #[test]
    fn steering_rejects_non_finite_and_out_of_range_numbers() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let s = SteeringCalibration {
                exponent: bad,
                ..steering()
            };
            assert!(matches!(s.validate(), Err(CalibrationError::NotFinite(_))));
        }
        let s = SteeringCalibration {
            deadzone: -0.1,
            ..steering()
        };
        assert!(matches!(s.validate(), Err(CalibrationError::OutOfRange(_))));
        let s = SteeringCalibration {
            sensitivity: 0.0,
            ..steering()
        };
        assert!(matches!(s.validate(), Err(CalibrationError::OutOfRange(_))));
    }

    #[test]
    fn pedal_handles_falling_raw_values() {
        let p = pedal();
        p.validate().unwrap();
        close(p.normalize(1000), 0.0);
        close(p.normalize(500), 0.5);
        close(p.normalize(0), 1.0);
    }

    #[test]
    fn pedal_handles_rising_raw_values() {
        let p = PedalCalibration {
            rest: 0,
            full: 1000,
            ..pedal()
        };
        close(p.normalize(0), 0.0);
        close(p.normalize(250), 0.25);
        close(p.normalize(1000), 1.0);
    }

    #[test]
    fn pedal_clamps_values_beyond_calibration() {
        let p = pedal();
        close(p.normalize(5000), 0.0);
        close(p.normalize(-5000), 1.0);
    }

    #[test]
    fn pedal_usable_travel_reaches_both_ends_early() {
        let p = PedalCalibration {
            rest: 0,
            full: 1000,
            rest_deadzone: 0.1,
            full_deadzone: 0.1,
            ..pedal()
        };
        close(p.normalize(100), 0.0);
        close(p.normalize(500), 0.5);
        close(p.normalize(900), 1.0);
    }

    #[test]
    fn pedal_exponent_and_inversion_apply() {
        let curved = PedalCalibration {
            rest: 0,
            full: 100,
            exponent: 2.0,
            ..pedal()
        };
        close(curved.normalize(50), 0.25);
        let inverted = PedalCalibration {
            rest: 0,
            full: 100,
            inverted: true,
            ..pedal()
        };
        close(inverted.normalize(0), 1.0);
        close(inverted.normalize(100), 0.0);
    }

    #[test]
    fn pedal_rejects_zero_span_and_no_usable_range() {
        let flat = PedalCalibration {
            rest: 5,
            full: 5,
            ..pedal()
        };
        assert_eq!(flat.validate(), Err(CalibrationError::ZeroSpan("pedal")));
        let no_range = PedalCalibration {
            rest_deadzone: 0.5,
            full_deadzone: 0.5,
            ..pedal()
        };
        assert_eq!(
            no_range.validate(),
            Err(CalibrationError::ZeroSpan("pedal usable range"))
        );
    }

    #[test]
    fn pedal_rejects_non_finite_numbers() {
        let p = PedalCalibration {
            rest_deadzone: f64::NAN,
            ..pedal()
        };
        assert!(matches!(p.validate(), Err(CalibrationError::NotFinite(_))));
    }
}

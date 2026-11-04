//! The guided calibration sequence.
//!
//! Each control is identified by asking the user to move it, so the sequence
//! assumes nothing about which axis is which on any device.

use crate::calibration::{CalibrationError, PedalCalibration, SteeringCalibration};
use crate::input::{AxisId, AxisState, Sample};
use crate::listen::{AxisDetector, MIN_AXIS_MOVEMENT};
use std::collections::BTreeSet;
use std::fmt;

/// Where the sequence has reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    FindSteering,
    SteeringCenter,
    SteeringLeft,
    SteeringRight,
    ThrottleFull,
    ThrottleRest,
    BrakeFull,
    BrakeRest,
    Done,
}

impl Step {
    /// What to tell the user.
    pub fn instruction(self) -> &'static str {
        match self {
            Self::FindSteering => "Turn the wheel a good way in either direction.",
            Self::SteeringCenter => "Centre the wheel, then choose Capture.",
            Self::SteeringLeft => "Turn the wheel fully left and hold it, then choose Capture.",
            Self::SteeringRight => "Turn the wheel fully right and hold it, then choose Capture.",
            Self::ThrottleFull => "Press the throttle fully and hold it, then choose Capture.",
            Self::ThrottleRest => "Release the throttle completely, then choose Capture.",
            Self::BrakeFull => "Press the brake fully and hold it, then choose Capture.",
            Self::BrakeRest => "Release the brake completely, then choose Capture.",
            Self::Done => "Calibration complete.",
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::FindSteering | Self::SteeringCenter => Self::FindSteering,
            Self::SteeringLeft => Self::SteeringCenter,
            Self::SteeringRight => Self::SteeringLeft,
            Self::ThrottleFull => Self::SteeringRight,
            Self::ThrottleRest => Self::ThrottleFull,
            Self::BrakeFull => Self::ThrottleRest,
            Self::BrakeRest => Self::BrakeFull,
            Self::Done => Self::BrakeRest,
        }
    }
}

/// Why a capture was refused. The sequence stays on the same step.
#[derive(Debug, Clone, PartialEq)]
pub enum CaptureError {
    /// Nothing has been detected or reported for this step yet.
    NoInput,
    /// The value is too close to the matching captured value.
    TooLittleTravel,
    /// A value does not fit the calibration range.
    OutOfRange,
    /// The captured values do not form a valid calibration.
    Invalid(CalibrationError),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoInput => write!(f, "No movement detected yet. Move the control first."),
            Self::TooLittleTravel => write!(f, "That is too close to the last position."),
            Self::OutOfRange => write!(f, "The device reported a value that can't be used."),
            Self::Invalid(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CaptureError {}

/// The three axes in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxisMap {
    pub steering: AxisId,
    pub throttle: AxisId,
    pub brake: AxisId,
}

/// The finished result, with no response shaping applied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Calibrated {
    pub axes: AxisMap,
    pub steering: SteeringCalibration,
    pub throttle: PedalCalibration,
    pub brake: PedalCalibration,
}

/// The sequence's progress.
#[derive(Debug, Clone)]
pub struct Wizard {
    step: Step,
    detector: Option<AxisDetector>,
    detected: Option<AxisId>,
    steering_axis: Option<AxisId>,
    center: i32,
    left: i32,
    right: i32,
    throttle_axis: Option<AxisId>,
    throttle_full: i32,
    throttle_rest: i32,
    brake_axis: Option<AxisId>,
    brake_full: i32,
    brake_rest: i32,
}

impl Default for Wizard {
    fn default() -> Self {
        Self::new()
    }
}

fn enough_travel(a: i32, b: i32, axis: &AxisState) -> bool {
    let Some(span) = axis.span() else {
        return false;
    };
    (f64::from(a) - f64::from(b)).abs() / span >= MIN_AXIS_MOVEMENT
}

impl Wizard {
    /// Starts at the first step.
    pub fn new() -> Self {
        Self {
            step: Step::FindSteering,
            detector: None,
            detected: None,
            steering_axis: None,
            center: 0,
            left: 0,
            right: 0,
            throttle_axis: None,
            throttle_full: 0,
            throttle_rest: 0,
            brake_axis: None,
            brake_full: 0,
            brake_rest: 0,
        }
    }

    /// Current step.
    pub fn step(&self) -> Step {
        self.step
    }

    /// The axis this step is watching, when known.
    pub fn current_axis(&self) -> Option<AxisId> {
        match self.step {
            Step::FindSteering | Step::ThrottleFull | Step::BrakeFull => self.detected,
            Step::SteeringCenter | Step::SteeringLeft | Step::SteeringRight => self.steering_axis,
            Step::ThrottleRest => self.throttle_axis,
            Step::BrakeRest => self.brake_axis,
            Step::Done => None,
        }
    }

    fn used_axes(&self) -> BTreeSet<AxisId> {
        [self.steering_axis, self.throttle_axis, self.brake_axis]
            .into_iter()
            .flatten()
            .collect()
    }

    fn enter(&mut self, step: Step) {
        self.step = step;
        self.detector = None;
        self.detected = None;
    }

    /// Feeds in the latest state. Detection only runs on steps that need it.
    pub fn observe(&mut self, sample: &Sample) {
        if !matches!(
            self.step,
            Step::FindSteering | Step::ThrottleFull | Step::BrakeFull
        ) {
            return;
        }
        let used = self.used_axes();
        let detector = self.detector.get_or_insert_with(|| AxisDetector::new(used));
        if let Some(axis) = detector.observe(sample) {
            self.detected = Some(axis);
            if self.step == Step::FindSteering {
                self.steering_axis = Some(axis);
                self.enter(Step::SteeringCenter);
            }
        }
    }

    /// True when the current step takes a Capture.
    pub fn needs_capture(&self) -> bool {
        !matches!(self.step, Step::FindSteering | Step::Done)
    }

    /// Records the current value and moves on.
    pub fn capture(&mut self, sample: &Sample) -> Result<(), CaptureError> {
        let id = self.current_axis().ok_or(CaptureError::NoInput)?;
        let axis = sample.axis(id).ok_or(CaptureError::NoInput)?;
        let value = i32::try_from(axis.value).map_err(|_| CaptureError::OutOfRange)?;
        match self.step {
            Step::FindSteering | Step::Done => return Err(CaptureError::NoInput),
            Step::SteeringCenter => {
                self.center = value;
                self.enter(Step::SteeringLeft);
            }
            Step::SteeringLeft => {
                if !enough_travel(value, self.center, axis) {
                    return Err(CaptureError::TooLittleTravel);
                }
                self.left = value;
                self.enter(Step::SteeringRight);
            }
            Step::SteeringRight => {
                if !enough_travel(value, self.center, axis) {
                    return Err(CaptureError::TooLittleTravel);
                }
                let trial = neutral_steering(self.left, self.center, value);
                trial.validate().map_err(CaptureError::Invalid)?;
                self.right = value;
                self.enter(Step::ThrottleFull);
            }
            Step::ThrottleFull => {
                self.throttle_axis = Some(id);
                self.throttle_full = value;
                self.enter(Step::ThrottleRest);
            }
            Step::ThrottleRest => {
                if !enough_travel(value, self.throttle_full, axis) {
                    return Err(CaptureError::TooLittleTravel);
                }
                self.throttle_rest = value;
                self.enter(Step::BrakeFull);
            }
            Step::BrakeFull => {
                self.brake_axis = Some(id);
                self.brake_full = value;
                self.enter(Step::BrakeRest);
            }
            Step::BrakeRest => {
                if !enough_travel(value, self.brake_full, axis) {
                    return Err(CaptureError::TooLittleTravel);
                }
                self.brake_rest = value;
                self.enter(Step::Done);
            }
        }
        Ok(())
    }

    /// Returns to the previous step, undoing what it recorded.
    pub fn back(&mut self) {
        let previous = self.step.previous();
        match previous {
            Step::FindSteering => self.steering_axis = None,
            Step::ThrottleFull => self.throttle_axis = None,
            Step::BrakeFull => self.brake_axis = None,
            _ => {}
        }
        self.enter(previous);
    }

    /// The finished calibration, once complete.
    pub fn result(&self) -> Option<Calibrated> {
        if self.step != Step::Done {
            return None;
        }
        Some(Calibrated {
            axes: AxisMap {
                steering: self.steering_axis?,
                throttle: self.throttle_axis?,
                brake: self.brake_axis?,
            },
            steering: neutral_steering(self.left, self.center, self.right),
            throttle: neutral_pedal(self.throttle_rest, self.throttle_full),
            brake: neutral_pedal(self.brake_rest, self.brake_full),
        })
    }
}

fn neutral_steering(left: i32, center: i32, right: i32) -> SteeringCalibration {
    SteeringCalibration {
        left,
        center,
        right,
        deadzone: 0.0,
        exponent: 1.0,
        sensitivity: 1.0,
        inverted: false,
    }
}

fn neutral_pedal(rest: i32, full: i32) -> PedalCalibration {
    PedalCalibration {
        rest,
        full,
        rest_deadzone: 0.0,
        full_deadzone: 0.0,
        exponent: 1.0,
        inverted: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHEEL: u16 = 1;
    const GAS: u16 = 2;
    const BRAKE: u16 = 3;

    fn id(usage: u16) -> AxisId {
        AxisId { page: 1, usage }
    }

    fn state(wheel: i64, gas: i64, brake: i64) -> Sample {
        let axis = |usage, value| AxisState {
            id: id(usage),
            value,
            logical_min: 0,
            logical_max: 1000,
        };
        Sample {
            axes: vec![axis(WHEEL, wheel), axis(GAS, gas), axis(BRAKE, brake)],
            ..Sample::default()
        }
    }

    fn at_throttle() -> Wizard {
        let mut wizard = Wizard::new();
        wizard.observe(&state(500, 1000, 1000));
        wizard.observe(&state(800, 1000, 1000));
        wizard.capture(&state(500, 1000, 1000)).unwrap();
        wizard.capture(&state(0, 1000, 1000)).unwrap();
        wizard.capture(&state(1000, 1000, 1000)).unwrap();
        wizard
    }

    #[test]
    fn full_sequence_produces_a_valid_calibration() {
        let mut wizard = at_throttle();
        assert_eq!(wizard.step(), Step::ThrottleFull);

        wizard.observe(&state(500, 1000, 1000));
        wizard.observe(&state(500, 0, 1000));
        wizard.capture(&state(500, 0, 1000)).unwrap();
        wizard.capture(&state(500, 1000, 1000)).unwrap();

        wizard.observe(&state(500, 1000, 1000));
        wizard.observe(&state(500, 1000, 100));
        wizard.capture(&state(500, 1000, 100)).unwrap();
        wizard.capture(&state(500, 1000, 1000)).unwrap();

        assert_eq!(wizard.step(), Step::Done);
        let done = wizard.result().unwrap();
        assert_eq!(done.axes.steering, id(WHEEL));
        assert_eq!(done.axes.throttle, id(GAS));
        assert_eq!(done.axes.brake, id(BRAKE));
        done.steering.validate().unwrap();
        done.throttle.validate().unwrap();
        done.brake.validate().unwrap();
        assert_eq!(done.steering.normalize(0), -1.0);
        assert_eq!(done.steering.normalize(1000), 1.0);
        assert_eq!(done.throttle.normalize(0), 1.0);
        assert_eq!(done.brake.normalize(1000), 0.0);
    }

    #[test]
    fn no_result_before_the_end() {
        assert_eq!(at_throttle().result(), None);
    }

    #[test]
    fn steering_step_advances_on_movement_without_capture() {
        let mut wizard = Wizard::new();
        assert!(!wizard.needs_capture());
        wizard.observe(&state(500, 1000, 1000));
        assert_eq!(wizard.step(), Step::FindSteering);
        wizard.observe(&state(700, 1000, 1000));
        assert_eq!(wizard.step(), Step::SteeringCenter);
        assert!(wizard.needs_capture());
    }

    #[test]
    fn capture_without_detection_is_refused() {
        let mut wizard = at_throttle();
        wizard.observe(&state(500, 1000, 1000));
        assert_eq!(
            wizard.capture(&state(500, 1000, 1000)),
            Err(CaptureError::NoInput)
        );
        assert_eq!(wizard.step(), Step::ThrottleFull);
    }

    #[test]
    fn capture_with_no_device_data_is_refused() {
        let mut wizard = Wizard::new();
        wizard.observe(&state(500, 1000, 1000));
        wizard.observe(&state(800, 1000, 1000));
        assert_eq!(
            wizard.capture(&Sample::default()),
            Err(CaptureError::NoInput)
        );
    }

    #[test]
    fn unmoved_positions_are_refused() {
        let mut wizard = Wizard::new();
        wizard.observe(&state(500, 1000, 1000));
        wizard.observe(&state(800, 1000, 1000));
        wizard.capture(&state(500, 1000, 1000)).unwrap();
        assert_eq!(
            wizard.capture(&state(510, 1000, 1000)),
            Err(CaptureError::TooLittleTravel)
        );
        assert_eq!(wizard.step(), Step::SteeringLeft);
    }

    #[test]
    fn left_and_right_on_the_same_side_are_refused() {
        let mut wizard = Wizard::new();
        wizard.observe(&state(500, 1000, 1000));
        wizard.observe(&state(800, 1000, 1000));
        wizard.capture(&state(500, 1000, 1000)).unwrap();
        wizard.capture(&state(0, 1000, 1000)).unwrap();
        assert_eq!(
            wizard.capture(&state(100, 1000, 1000)),
            Err(CaptureError::Invalid(CalibrationError::CenterOutsideRange))
        );
        assert_eq!(wizard.step(), Step::SteeringRight);
    }

    #[test]
    fn a_pedal_that_did_not_release_is_refused() {
        let mut wizard = at_throttle();
        wizard.observe(&state(500, 1000, 1000));
        wizard.observe(&state(500, 0, 1000));
        wizard.capture(&state(500, 0, 1000)).unwrap();
        assert_eq!(
            wizard.capture(&state(500, 20, 1000)),
            Err(CaptureError::TooLittleTravel)
        );
    }

    #[test]
    fn brake_cannot_reuse_the_throttle_axis() {
        let mut wizard = at_throttle();
        wizard.observe(&state(500, 1000, 1000));
        wizard.observe(&state(500, 0, 1000));
        wizard.capture(&state(500, 0, 1000)).unwrap();
        wizard.capture(&state(500, 1000, 1000)).unwrap();
        wizard.observe(&state(500, 1000, 1000));
        wizard.observe(&state(500, 0, 1000));
        assert_eq!(wizard.current_axis(), None);
    }

    #[test]
    fn pedals_cannot_reuse_the_steering_axis() {
        let mut wizard = at_throttle();
        wizard.observe(&state(500, 1000, 1000));
        wizard.observe(&state(0, 1000, 1000));
        assert_eq!(wizard.current_axis(), None);
    }

    #[test]
    fn back_undoes_a_step_and_frees_its_axis() {
        let mut wizard = at_throttle();
        wizard.observe(&state(500, 1000, 1000));
        wizard.observe(&state(500, 0, 1000));
        wizard.capture(&state(500, 0, 1000)).unwrap();
        assert_eq!(wizard.step(), Step::ThrottleRest);
        wizard.back();
        assert_eq!(wizard.step(), Step::ThrottleFull);
        wizard.observe(&state(500, 1000, 1000));
        wizard.observe(&state(500, 0, 1000));
        assert_eq!(wizard.current_axis(), Some(id(GAS)));
    }

    #[test]
    fn back_from_the_start_stays_at_the_start() {
        let mut wizard = Wizard::new();
        wizard.back();
        assert_eq!(wizard.step(), Step::FindSteering);
    }

    #[test]
    fn value_that_does_not_fit_is_refused() {
        let mut wizard = Wizard::new();
        wizard.observe(&state(500, 1000, 1000));
        wizard.observe(&state(800, 1000, 1000));
        assert_eq!(
            wizard.capture(&state(i64::MAX, 1000, 1000)),
            Err(CaptureError::OutOfRange)
        );
    }
}

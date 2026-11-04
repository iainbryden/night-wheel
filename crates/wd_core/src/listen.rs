//! Detecting an intentional control while setting up. Nothing here reaches the game.

use crate::input::{AxisId, Control, Sample};
use std::collections::{BTreeMap, BTreeSet};

/// Share of an axis's reported range it must move to count as intentional.
///
/// Provisional. One idle run moved the steering axis by about 0.4% of its range,
/// so this sits well above that. To be reviewed against the wheel check.
pub const MIN_AXIS_MOVEMENT: f64 = 0.10;

/// Time allowed for a Listen before it gives up, in milliseconds.
pub const LISTEN_TIMEOUT_MS: u64 = 10_000;

/// State of a Listen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListenOutcome {
    Waiting,
    Detected(Control),
    TimedOut,
}

/// Waits for a fresh press. Controls held when listening began are ignored until released.
#[derive(Debug, Clone)]
pub struct ButtonListener {
    deadline_ms: u64,
    blocked: BTreeSet<Control>,
}

impl ButtonListener {
    /// Starts listening. `sample` is `None` when no device is connected.
    pub fn new(now_ms: u64, sample: Option<&Sample>, timeout_ms: u64) -> Self {
        Self {
            deadline_ms: now_ms.saturating_add(timeout_ms),
            blocked: sample.map(Sample::held).unwrap_or_default(),
        }
    }

    /// Takes the latest state.
    ///
    /// A missing sample proves nothing about release, so blocked controls stay blocked.
    pub fn update(&mut self, now_ms: u64, sample: Option<&Sample>) -> ListenOutcome {
        if let Some(sample) = sample {
            let held = sample.held();
            self.blocked.retain(|control| held.contains(control));
            if let Some(&control) = held.difference(&self.blocked).next() {
                return ListenOutcome::Detected(control);
            }
        }
        if now_ms >= self.deadline_ms {
            ListenOutcome::TimedOut
        } else {
            ListenOutcome::Waiting
        }
    }
}

/// Finds the axis the user is moving, by comparing with where each axis started.
#[derive(Debug, Clone)]
pub struct AxisDetector {
    baseline: BTreeMap<AxisId, i64>,
    excluded: BTreeSet<AxisId>,
}

impl AxisDetector {
    /// Starts detecting. Axes in `excluded` are never reported.
    pub fn new(excluded: BTreeSet<AxisId>) -> Self {
        Self {
            baseline: BTreeMap::new(),
            excluded,
        }
    }

    /// Returns the axis that has moved furthest, once it has moved far enough.
    pub fn observe(&mut self, sample: &Sample) -> Option<AxisId> {
        let mut best: Option<(AxisId, f64)> = None;
        for axis in &sample.axes {
            if self.excluded.contains(&axis.id) {
                continue;
            }
            let Some(span) = axis.span() else {
                continue;
            };
            let start = *self.baseline.entry(axis.id).or_insert(axis.value);
            let moved = (axis.value - start).abs() as f64 / span;
            if moved >= MIN_AXIS_MOVEMENT && best.is_none_or(|(_, top)| moved > top) {
                best = Some((axis.id, moved));
            }
        }
        best.map(|(id, _)| id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{AxisState, ButtonId};

    const TIMEOUT: u64 = 1000;

    fn button(usage: u16) -> ButtonId {
        ButtonId { page: 9, usage }
    }

    fn holding(usages: &[u16]) -> Sample {
        Sample {
            buttons: usages.iter().map(|usage| button(*usage)).collect(),
            ..Sample::default()
        }
    }

    fn axes(values: &[(u16, i64)]) -> Sample {
        Sample {
            axes: values
                .iter()
                .map(|(usage, value)| AxisState {
                    id: AxisId {
                        page: 1,
                        usage: *usage,
                    },
                    value: *value,
                    logical_min: 0,
                    logical_max: 1000,
                })
                .collect(),
            ..Sample::default()
        }
    }

    fn id(usage: u16) -> AxisId {
        AxisId { page: 1, usage }
    }

    #[test]
    fn fresh_press_is_detected() {
        let mut listener = ButtonListener::new(0, Some(&holding(&[])), TIMEOUT);
        assert_eq!(
            listener.update(1, Some(&holding(&[]))),
            ListenOutcome::Waiting
        );
        assert_eq!(
            listener.update(2, Some(&holding(&[5]))),
            ListenOutcome::Detected(Control::Button(button(5)))
        );
    }

    #[test]
    fn control_held_before_listening_is_ignored_until_released() {
        let mut listener = ButtonListener::new(0, Some(&holding(&[5])), TIMEOUT);
        assert_eq!(
            listener.update(1, Some(&holding(&[5]))),
            ListenOutcome::Waiting
        );
        assert_eq!(
            listener.update(2, Some(&holding(&[]))),
            ListenOutcome::Waiting
        );
        assert_eq!(
            listener.update(3, Some(&holding(&[5]))),
            ListenOutcome::Detected(Control::Button(button(5)))
        );
    }

    #[test]
    fn another_control_is_detected_while_one_stays_held() {
        let mut listener = ButtonListener::new(0, Some(&holding(&[5])), TIMEOUT);
        assert_eq!(
            listener.update(1, Some(&holding(&[5, 6]))),
            ListenOutcome::Detected(Control::Button(button(6)))
        );
    }

    #[test]
    fn hat_position_is_detected() {
        let mut listener = ButtonListener::new(0, Some(&Sample::default()), TIMEOUT);
        let sample = Sample {
            hat: Some(4),
            ..Sample::default()
        };
        assert_eq!(
            listener.update(1, Some(&sample)),
            ListenOutcome::Detected(Control::Hat(4))
        );
    }

    #[test]
    fn listening_times_out() {
        let mut listener = ButtonListener::new(100, Some(&holding(&[])), TIMEOUT);
        assert_eq!(
            listener.update(1099, Some(&holding(&[]))),
            ListenOutcome::Waiting
        );
        assert_eq!(
            listener.update(1100, Some(&holding(&[]))),
            ListenOutcome::TimedOut
        );
    }

    #[test]
    fn disconnect_does_not_count_as_release() {
        let mut listener = ButtonListener::new(0, Some(&holding(&[5])), TIMEOUT);
        assert_eq!(listener.update(1, None), ListenOutcome::Waiting);
        assert_eq!(
            listener.update(2, Some(&holding(&[5]))),
            ListenOutcome::Waiting
        );
    }

    #[test]
    fn listening_with_no_device_still_times_out() {
        let mut listener = ButtonListener::new(0, None, TIMEOUT);
        assert_eq!(listener.update(TIMEOUT, None), ListenOutcome::TimedOut);
    }

    #[test]
    fn axis_noise_is_not_detected() {
        let mut detector = AxisDetector::new(BTreeSet::new());
        assert_eq!(detector.observe(&axes(&[(1, 500)])), None);
        assert_eq!(detector.observe(&axes(&[(1, 505)])), None);
        assert_eq!(detector.observe(&axes(&[(1, 499)])), None);
    }

    #[test]
    fn deliberate_axis_movement_is_detected_in_either_direction() {
        let mut up = AxisDetector::new(BTreeSet::new());
        up.observe(&axes(&[(1, 500)]));
        assert_eq!(up.observe(&axes(&[(1, 700)])), Some(id(1)));
        let mut down = AxisDetector::new(BTreeSet::new());
        down.observe(&axes(&[(1, 500)]));
        assert_eq!(down.observe(&axes(&[(1, 300)])), Some(id(1)));
    }

    #[test]
    fn the_axis_moved_furthest_wins() {
        let mut detector = AxisDetector::new(BTreeSet::new());
        detector.observe(&axes(&[(1, 500), (2, 500)]));
        assert_eq!(detector.observe(&axes(&[(1, 650), (2, 900)])), Some(id(2)));
    }

    #[test]
    fn excluded_axes_are_never_reported() {
        let mut detector = AxisDetector::new(BTreeSet::from([id(1)]));
        detector.observe(&axes(&[(1, 0), (2, 500)]));
        assert_eq!(detector.observe(&axes(&[(1, 1000), (2, 500)])), None);
    }

    #[test]
    fn axis_with_no_usable_range_is_skipped() {
        let mut detector = AxisDetector::new(BTreeSet::new());
        let flat = |value| Sample {
            axes: vec![AxisState {
                id: id(1),
                value,
                logical_min: 0,
                logical_max: 0,
            }],
            ..Sample::default()
        };
        detector.observe(&flat(0));
        assert_eq!(detector.observe(&flat(100)), None);
    }
}

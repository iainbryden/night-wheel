//! A device-independent picture of a controller's current state.

use crate::hid_value::logical_fraction;
use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

/// HID usage page for generic desktop controls.
pub const GENERIC_DESKTOP_PAGE: u16 = 0x01;
/// HID usage for a hat switch.
pub const HAT_USAGE: u16 = 0x39;

/// Identifies an axis by what the device reports. Says nothing about which physical control it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AxisId {
    pub page: u16,
    pub usage: u16,
}

impl fmt::Display for AxisId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04X}:{:04X}", self.page, self.usage)
    }
}

fn parse_hex_pair(text: &str) -> Option<(u16, u16)> {
    let (page, usage) = text.split_once(':')?;
    Some((
        u16::from_str_radix(page, 16).ok()?,
        u16::from_str_radix(usage, 16).ok()?,
    ))
}

impl FromStr for AxisId {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, ()> {
        let (page, usage) = parse_hex_pair(text).ok_or(())?;
        Ok(Self { page, usage })
    }
}

/// Identifies a button by what the device reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ButtonId {
    pub page: u16,
    pub usage: u16,
}

/// A control that can be assigned to an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Control {
    Button(ButtonId),
    /// A hat switch position, counted from the bottom of its reported range.
    Hat(u16),
}

impl Control {
    /// Name for screens. These are the device's own numbers, because labels on the
    /// wheel are not assumed to match them.
    pub fn label(&self) -> String {
        match self {
            Self::Button(button) => format!("Button {}", button.usage),
            Self::Hat(position) => format!("D-pad position {position}"),
        }
    }
}

impl fmt::Display for Control {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Button(button) => write!(f, "button:{:04X}:{:04X}", button.page, button.usage),
            Self::Hat(position) => write!(f, "hat:{position}"),
        }
    }
}

impl FromStr for Control {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, ()> {
        if let Some(rest) = text.strip_prefix("button:") {
            let (page, usage) = parse_hex_pair(rest).ok_or(())?;
            return Ok(Self::Button(ButtonId { page, usage }));
        }
        if let Some(rest) = text.strip_prefix("hat:") {
            return rest.parse().map(Self::Hat).map_err(|_| ());
        }
        Err(())
    }
}

/// One axis's current value and the range the device reports for it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisState {
    pub id: AxisId,
    pub value: i64,
    pub logical_min: i64,
    pub logical_max: i64,
}

impl AxisState {
    /// Position within the reported range.
    pub fn fraction(&self) -> Option<f64> {
        logical_fraction(self.value, self.logical_min, self.logical_max)
    }

    /// Size of the reported range, when it is usable.
    pub fn span(&self) -> Option<f64> {
        (self.logical_max > self.logical_min).then(|| (self.logical_max - self.logical_min) as f64)
    }
}

/// Everything known about a controller at one moment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sample {
    pub axes: Vec<AxisState>,
    pub buttons: BTreeSet<ButtonId>,
    pub hat: Option<u16>,
}

impl Sample {
    /// Looks up an axis.
    pub fn axis(&self, id: AxisId) -> Option<&AxisState> {
        self.axes.iter().find(|axis| axis.id == id)
    }

    /// Every assignable control currently held.
    pub fn held(&self) -> BTreeSet<Control> {
        let mut held: BTreeSet<Control> =
            self.buttons.iter().copied().map(Control::Button).collect();
        if let Some(position) = self.hat {
            held.insert(Control::Hat(position));
        }
        held
    }
}

/// A set of held buttons as bits, with usage 1 in the lowest bit. Usages outside 1 to 64 are left
/// out. Allocates nothing, so it can run for every wheel report.
pub fn button_mask(usages: &[u16]) -> u64 {
    usages.iter().fold(0, |mask, &usage| {
        if (1..=64).contains(&usage) {
            mask | 1u64 << (usage - 1)
        } else {
            mask
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn button_mask_sets_one_bit_for_each_usage() {
        assert_eq!(super::button_mask(&[]), 0);
        assert_eq!(super::button_mask(&[1]), 1);
        assert_eq!(super::button_mask(&[1, 3]), 0b101);
        assert_eq!(super::button_mask(&[64]), 1u64 << 63);
    }

    #[test]
    fn button_mask_leaves_out_usages_it_cannot_hold() {
        assert_eq!(super::button_mask(&[0, 65, 500]), 0);
    }

    use super::*;

    #[test]
    fn axis_id_text_round_trips() {
        let id = AxisId {
            page: 1,
            usage: 0x30,
        };
        assert_eq!(id.to_string(), "0001:0030");
        assert_eq!("0001:0030".parse(), Ok(id));
    }

    #[test]
    fn control_text_round_trips() {
        for control in [
            Control::Button(ButtonId { page: 9, usage: 12 }),
            Control::Hat(3),
        ] {
            assert_eq!(control.to_string().parse(), Ok(control));
        }
    }

    #[test]
    fn malformed_text_is_rejected() {
        for text in [
            "",
            "button:",
            "button:9",
            "button:zz:1",
            "hat:",
            "hat:x",
            "key:1",
        ] {
            assert_eq!(text.parse::<Control>(), Err(()), "{text}");
        }
        assert_eq!("0001".parse::<AxisId>(), Err(()));
    }

    #[test]
    fn held_includes_buttons_and_hat() {
        let button = ButtonId { page: 9, usage: 1 };
        let sample = Sample {
            axes: Vec::new(),
            buttons: BTreeSet::from([button]),
            hat: Some(2),
        };
        assert_eq!(
            sample.held(),
            BTreeSet::from([Control::Button(button), Control::Hat(2)])
        );
    }

    #[test]
    fn unusable_range_has_no_span() {
        let axis = AxisState {
            id: AxisId { page: 1, usage: 1 },
            value: 0,
            logical_min: 5,
            logical_max: 5,
        };
        assert_eq!(axis.span(), None);
        assert_eq!(axis.fraction(), None);
    }
}

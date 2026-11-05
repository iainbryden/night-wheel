//! Reads what a HID game controller reports about itself and decodes its reports.
//!
//! Nothing here assumes which control is which. Controls are named only by
//! their HID usage; matching them to the physical wheel and pedals is done by
//! observation.

use std::collections::BTreeSet;
use std::mem::size_of;
use wd_core::hid_value::{decode_logical, logical_fraction};
use wd_core::input::{
    AxisId, AxisState, ButtonId, GENERIC_DESKTOP_PAGE, HAT_USAGE, Sample, button_mask,
};
use windows::Win32::Devices::HumanInterfaceDevice::{
    HIDP_BUTTON_CAPS, HIDP_CAPS, HIDP_STATUS_SUCCESS, HIDP_VALUE_CAPS, HidP_GetButtonCaps,
    HidP_GetCaps, HidP_GetUsageValue, HidP_GetUsages, HidP_GetValueCaps, HidP_Input,
    PHIDP_PREPARSED_DATA,
};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::UI::Input::{
    GetRawInputDeviceInfoW, RAW_INPUT_DEVICE_INFO_COMMAND, RID_DEVICE_INFO, RIDI_DEVICEINFO,
    RIDI_PREPARSEDDATA, RIM_TYPEHID,
};

const JOYSTICK_USAGE: u16 = 0x04;
const GAMEPAD_USAGE: u16 = 0x05;
const MULTI_AXIS_USAGE: u16 = 0x08;

/// Identity observed on the target PC; see SRC-014 in evidence/SOURCES.csv.
const OBSERVED_TARGET: (u32, u32) = (0x046D, 0xC24F);

/// Upper limit on buttons reported as held in one report page.
const MAX_USAGES: usize = 256;

/// The HID usage page for numbered buttons.
const BUTTON_PAGE: u16 = 0x09;

/// One value control, such as an axis or hat switch.
#[derive(Debug)]
pub struct ValueControl {
    pub page: u16,
    pub usage: u16,
    pub link: u16,
    pub bit_size: u16,
    pub logical_min: i64,
    pub logical_max: i64,
    pub current: Option<i64>,
    pub lowest: Option<i64>,
    pub highest: Option<i64>,
}

impl ValueControl {
    pub fn name(&self) -> String {
        usage_name(self.page, self.usage)
    }

    /// Position within the reported range, when both are known.
    pub fn fraction(&self) -> Option<f64> {
        logical_fraction(self.current?, self.logical_min, self.logical_max)
    }
}

/// A connected game controller.
#[derive(Debug)]
pub struct Device {
    pub handle: isize,
    pub vendor: u32,
    pub product: u32,
    pub version: u32,
    pub top_page: u16,
    pub top_usage: u16,
    pub values: Vec<ValueControl>,
    /// (usage page, link collection) pairs that carry buttons.
    button_pages: Vec<(u16, u16)>,
    pub button_count: usize,
    pub pressed: BTreeSet<(u16, u16)>,
    /// Held numbered buttons as bits, kept current by [`Device::apply_values`].
    pub held_buttons: u64,
    pub reports: u64,
    pub input_report_bytes: u16,
    pub output_report_bytes: u16,
    pub feature_report_bytes: u16,
    preparsed: Vec<u64>,
}

/// A change noticed while decoding a report.
#[derive(Debug, PartialEq)]
pub enum Change {
    Value {
        name: String,
        value: i64,
    },
    Button {
        page: u16,
        usage: u16,
        pressed: bool,
    },
}

impl Device {
    /// Reads a device's identity and capabilities.
    ///
    /// Returns `Ok(None)` when the device is not a game controller.
    pub fn open(handle: HANDLE) -> Result<Option<Self>, String> {
        let info = device_info(handle)?;
        if info.dwType != RIM_TYPEHID {
            return Ok(None);
        }
        // SAFETY: dwType was just checked, so `hid` is the active union member.
        let hid = unsafe { info.Anonymous.hid };
        if hid.usUsagePage != GENERIC_DESKTOP_PAGE
            || ![JOYSTICK_USAGE, GAMEPAD_USAGE, MULTI_AXIS_USAGE].contains(&hid.usUsage)
        {
            return Ok(None);
        }

        let preparsed = preparsed_data(handle)?;
        let parsed = PHIDP_PREPARSED_DATA(preparsed.as_ptr() as isize);
        let mut caps = HIDP_CAPS::default();
        // SAFETY: `parsed` points at preparsed data owned by `preparsed`, and `caps` is writable.
        let status = unsafe { HidP_GetCaps(parsed, &mut caps) };
        if status != HIDP_STATUS_SUCCESS {
            return Err(format!("HidP_GetCaps failed: {:#x}", status.0));
        }

        let values = read_value_caps(parsed, caps.NumberInputValueCaps)?;
        let (button_pages, button_count) = read_button_caps(parsed, caps.NumberInputButtonCaps)?;

        Ok(Some(Self {
            handle: handle.0 as isize,
            vendor: hid.dwVendorId,
            product: hid.dwProductId,
            version: hid.dwVersionNumber,
            top_page: hid.usUsagePage,
            top_usage: hid.usUsage,
            values,
            button_pages,
            button_count,
            pressed: BTreeSet::new(),
            held_buttons: 0,
            reports: 0,
            input_report_bytes: caps.InputReportByteLength,
            output_report_bytes: caps.OutputReportByteLength,
            feature_report_bytes: caps.FeatureReportByteLength,
            preparsed,
        }))
    }

    /// Current state in device-independent form.
    ///
    /// Only generic desktop controls are included. A hat value outside its reported
    /// range is treated as not pressed.
    pub fn sample(&self) -> Sample {
        let mut sample = Sample {
            buttons: self
                .pressed
                .iter()
                .map(|&(page, usage)| ButtonId { page, usage })
                .collect(),
            ..Sample::default()
        };
        for control in &self.values {
            let Some(value) = control.current else {
                continue;
            };
            if control.page != GENERIC_DESKTOP_PAGE {
                continue;
            }
            if control.usage == HAT_USAGE {
                if (control.logical_min..=control.logical_max).contains(&value) {
                    sample.hat = u16::try_from(value - control.logical_min).ok();
                }
                continue;
            }
            sample.axes.push(AxisState {
                id: AxisId {
                    page: control.page,
                    usage: control.usage,
                },
                value,
                logical_min: control.logical_min,
                logical_max: control.logical_max,
            });
        }
        sample
    }

    /// True when this is the wheel observed on the target PC.
    pub fn is_observed_target(&self) -> bool {
        (self.vendor, self.product) == OBSERVED_TARGET
    }

    /// Short identity for display.
    pub fn label(&self) -> String {
        let target = if self.is_observed_target() {
            " (observed G29)"
        } else {
            ""
        };
        format!(
            "VID_{:04X} PID_{:04X} rev {:04X}{target}",
            self.vendor, self.product, self.version
        )
    }

    /// The current value of a control, when it has reported one.
    pub fn value(&self, page: u16, usage: u16) -> Option<i64> {
        self.values
            .iter()
            .find(|control| control.page == page && control.usage == usage)
            .and_then(|control| control.current)
    }

    /// Decodes value controls and held buttons and allocates nothing, for the low-latency path.
    pub fn apply_values(&mut self, report: &mut [u8]) {
        self.reports += 1;
        let parsed = PHIDP_PREPARSED_DATA(self.preparsed.as_ptr() as isize);
        for control in &mut self.values {
            decode_value(control, parsed, report);
        }
        let mut held = 0;
        let mut decoded_any = false;
        for &(page, link) in &self.button_pages {
            if page != BUTTON_PAGE {
                continue;
            }
            let mut usages = [0u16; MAX_USAGES];
            let mut length = MAX_USAGES as u32;
            // SAFETY: `usages` holds `length` items, and the other pointers are valid for the call.
            let status = unsafe {
                HidP_GetUsages(
                    HidP_Input,
                    page,
                    Some(link),
                    usages.as_mut_ptr(),
                    &mut length,
                    parsed,
                    report,
                )
            };
            if status == HIDP_STATUS_SUCCESS {
                decoded_any = true;
                held |= button_mask(&usages[..length as usize]);
            }
        }
        // A report that carries no buttons says nothing about them, so the last state stands.
        if decoded_any {
            self.held_buttons = held;
        }
    }

    /// Decodes one input report and returns what changed.
    pub fn apply_report(&mut self, report: &mut [u8]) -> Vec<Change> {
        self.reports += 1;
        let parsed = PHIDP_PREPARSED_DATA(self.preparsed.as_ptr() as isize);
        let mut changes = Vec::new();

        for control in &mut self.values {
            let before = control.current;
            if let Some(value) = decode_value(control, parsed, report)
                && before != Some(value)
            {
                changes.push(Change::Value {
                    name: usage_name(control.page, control.usage),
                    value,
                });
            }
        }

        let mut held = BTreeSet::new();
        let mut decoded_any = false;
        for &(page, link) in &self.button_pages {
            let mut usages = [0u16; MAX_USAGES];
            let mut length = MAX_USAGES as u32;
            // SAFETY: `usages` holds `length` items, and the other pointers are valid for the call.
            let status = unsafe {
                HidP_GetUsages(
                    HidP_Input,
                    page,
                    Some(link),
                    usages.as_mut_ptr(),
                    &mut length,
                    parsed,
                    report,
                )
            };
            if status != HIDP_STATUS_SUCCESS {
                continue;
            }
            decoded_any = true;
            for &usage in &usages[..length as usize] {
                held.insert((page, usage));
            }
        }
        if decoded_any {
            for &(page, usage) in held.difference(&self.pressed) {
                changes.push(Change::Button {
                    page,
                    usage,
                    pressed: true,
                });
            }
            for &(page, usage) in self.pressed.difference(&held) {
                changes.push(Change::Button {
                    page,
                    usage,
                    pressed: false,
                });
            }
            self.pressed = held;
        }
        changes
    }
}

/// Updates one control from a report. Returns its value when the report carried it.
fn decode_value(
    control: &mut ValueControl,
    parsed: PHIDP_PREPARSED_DATA,
    report: &mut [u8],
) -> Option<i64> {
    let mut raw = 0u32;
    // SAFETY: `parsed` and `report` are valid for the duration of the call.
    let status = unsafe {
        HidP_GetUsageValue(
            HidP_Input,
            control.page,
            Some(control.link),
            control.usage,
            &mut raw,
            parsed,
            report,
        )
    };
    // A device with several report IDs only carries some controls per report.
    if status != HIDP_STATUS_SUCCESS {
        return None;
    }
    let min = i32::try_from(control.logical_min).unwrap_or(0);
    let value = decode_logical(raw, control.bit_size, min)?;
    control.current = Some(value);
    control.lowest = Some(control.lowest.map_or(value, |low| low.min(value)));
    control.highest = Some(control.highest.map_or(value, |high| high.max(value)));
    Some(value)
}

fn device_info(handle: HANDLE) -> Result<RID_DEVICE_INFO, String> {
    let mut info = RID_DEVICE_INFO {
        cbSize: size_of::<RID_DEVICE_INFO>() as u32,
        ..Default::default()
    };
    let mut size = size_of::<RID_DEVICE_INFO>() as u32;
    // SAFETY: `info` is writable for `size` bytes, and its cbSize is set as documented.
    let copied = unsafe {
        GetRawInputDeviceInfoW(
            Some(handle),
            RIDI_DEVICEINFO,
            Some((&raw mut info).cast()),
            &mut size,
        )
    };
    if copied == u32::MAX {
        return Err("could not read device information".to_owned());
    }
    Ok(info)
}

fn preparsed_data(handle: HANDLE) -> Result<Vec<u64>, String> {
    let command: RAW_INPUT_DEVICE_INFO_COMMAND = RIDI_PREPARSEDDATA;
    let mut size = 0u32;
    // SAFETY: a null buffer asks only for the required size.
    let probe = unsafe { GetRawInputDeviceInfoW(Some(handle), command, None, &mut size) };
    if probe != 0 || size == 0 {
        return Err("could not size device description".to_owned());
    }
    // u64 storage keeps the opaque data aligned.
    let mut buffer = vec![0u64; (size as usize).div_ceil(size_of::<u64>())];
    // SAFETY: `buffer` holds at least `size` bytes.
    let copied = unsafe {
        GetRawInputDeviceInfoW(
            Some(handle),
            command,
            Some(buffer.as_mut_ptr().cast()),
            &mut size,
        )
    };
    if copied == u32::MAX {
        return Err("could not read device description".to_owned());
    }
    Ok(buffer)
}

fn read_value_caps(parsed: PHIDP_PREPARSED_DATA, count: u16) -> Result<Vec<ValueControl>, String> {
    let mut controls = Vec::new();
    if count == 0 {
        return Ok(controls);
    }
    let mut caps = vec![HIDP_VALUE_CAPS::default(); usize::from(count)];
    let mut length = count;
    // SAFETY: `caps` holds `length` items, and `parsed` stays valid for the call.
    let status = unsafe { HidP_GetValueCaps(HidP_Input, caps.as_mut_ptr(), &mut length, parsed) };
    if status != HIDP_STATUS_SUCCESS {
        return Err(format!("HidP_GetValueCaps failed: {:#x}", status.0));
    }
    for cap in &caps[..usize::from(length)] {
        // SAFETY: both union members start with the same usage fields; IsRange selects which.
        let (first, last) = unsafe {
            if cap.IsRange {
                (cap.Anonymous.Range.UsageMin, cap.Anonymous.Range.UsageMax)
            } else {
                (cap.Anonymous.NotRange.Usage, cap.Anonymous.NotRange.Usage)
            }
        };
        for usage in first..=last {
            controls.push(ValueControl {
                page: cap.UsagePage,
                usage,
                link: cap.LinkCollection,
                bit_size: cap.BitSize,
                logical_min: i64::from(cap.LogicalMin),
                logical_max: i64::from(cap.LogicalMax),
                current: None,
                lowest: None,
                highest: None,
            });
        }
    }
    Ok(controls)
}

fn read_button_caps(
    parsed: PHIDP_PREPARSED_DATA,
    count: u16,
) -> Result<(Vec<(u16, u16)>, usize), String> {
    if count == 0 {
        return Ok((Vec::new(), 0));
    }
    let mut caps = vec![HIDP_BUTTON_CAPS::default(); usize::from(count)];
    let mut length = count;
    // SAFETY: `caps` holds `length` items, and `parsed` stays valid for the call.
    let status = unsafe { HidP_GetButtonCaps(HidP_Input, caps.as_mut_ptr(), &mut length, parsed) };
    if status != HIDP_STATUS_SUCCESS {
        return Err(format!("HidP_GetButtonCaps failed: {:#x}", status.0));
    }
    let mut pages = BTreeSet::new();
    let mut total = 0usize;
    for cap in &caps[..usize::from(length)] {
        pages.insert((cap.UsagePage, cap.LinkCollection));
        // SAFETY: both union members start with the same usage fields; IsRange selects which.
        let span = unsafe {
            if cap.IsRange {
                usize::from(cap.Anonymous.Range.UsageMax)
                    .saturating_sub(usize::from(cap.Anonymous.Range.UsageMin))
                    + 1
            } else {
                1
            }
        };
        total += span;
    }
    Ok((pages.into_iter().collect(), total))
}

/// Readable name for a HID usage. Describes the report only, not the physical control.
pub fn usage_name(page: u16, usage: u16) -> String {
    if page == GENERIC_DESKTOP_PAGE {
        let known = match usage {
            0x30 => Some("X"),
            0x31 => Some("Y"),
            0x32 => Some("Z"),
            0x33 => Some("Rx"),
            0x34 => Some("Ry"),
            0x35 => Some("Rz"),
            0x36 => Some("Slider"),
            0x37 => Some("Dial"),
            0x38 => Some("Wheel"),
            0x39 => Some("Hat"),
            _ => None,
        };
        if let Some(name) = known {
            return name.to_owned();
        }
    }
    format!("{page:04X}:{usage:04X}")
}

//! Runs the Raw Input message loop and turns its messages into events.

use crate::device::{Change, Device};
use std::collections::BTreeMap;
use std::mem::size_of;
use std::time::Instant;
use windows::Win32::Foundation::{HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceList, HRAWINPUT, RAWHID, RAWINPUT, RAWINPUTDEVICE,
    RAWINPUTDEVICELIST, RAWINPUTHEADER, RID_INPUT, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RIM_TYPEHID,
    RegisterRawInputDevices,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_MESSAGE,
    KillTimer, MSG, RegisterClassW, SetTimer, WINDOW_EX_STYLE, WINDOW_STYLE, WM_INPUT,
    WM_INPUT_DEVICE_CHANGE, WM_TIMER, WNDCLASSW,
};
use windows::core::w;

const GIDC_ARRIVAL: usize = 1;
const GIDC_REMOVAL: usize = 2;
const TIMER_ID: usize = 1;

/// Connected devices, keyed by their system handle.
pub type Devices = BTreeMap<isize, Device>;

/// Something that happened in the message loop.
pub enum Event<'a> {
    Connected(&'a Device),
    Disconnected(&'a Device),
    Report {
        device: &'a Device,
        /// Always empty under [`run_values_only`].
        changes: &'a [Change],
        /// When this thread began handling the report.
        received: Instant,
    },
    /// Fires at the rate given to [`run`], whether or not anything changed.
    Tick(&'a Devices),
}

/// Whether the message loop should keep going.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Stop,
}

/// Lists game controllers without registering for input.
pub fn enumerate() -> Result<Devices, String> {
    let entry_size = size_of::<RAWINPUTDEVICELIST>() as u32;
    let mut count = 0u32;
    // SAFETY: a null list asks only for the device count.
    let probe = unsafe { GetRawInputDeviceList(None, &mut count, entry_size) };
    if probe == u32::MAX {
        return Err("could not count input devices".to_owned());
    }
    let mut list = vec![RAWINPUTDEVICELIST::default(); count as usize];
    // SAFETY: `list` holds `count` entries.
    let found = unsafe { GetRawInputDeviceList(Some(list.as_mut_ptr()), &mut count, entry_size) };
    if found == u32::MAX {
        return Err("could not list input devices".to_owned());
    }
    let mut devices = BTreeMap::new();
    for entry in list.iter().take(found as usize) {
        if entry.dwType != RIM_TYPEHID {
            continue;
        }
        match Device::open(entry.hDevice) {
            Ok(Some(device)) => {
                devices.insert(device.handle, device);
            }
            Ok(None) => {}
            Err(problem) => eprintln!("warning: skipped a device: {problem}"),
        }
    }
    Ok(devices)
}

/// Runs on the calling thread until `handler` returns [`Flow::Stop`], then returns the
/// final device state. Call this only once per process, because it registers a window class.
pub fn run(tick_ms: u32, mut handler: impl FnMut(Event<'_>) -> Flow) -> Result<Devices, String> {
    run_with(Decode::Full, tick_ms, &mut handler)
}

/// Like [`run`], but decodes only value controls and allocates nothing per report.
/// Reports are handled as they arrive; the tick does not delay them.
pub fn run_values_only(
    tick_ms: u32,
    mut handler: impl FnMut(Event<'_>) -> Flow,
) -> Result<Devices, String> {
    run_with(Decode::ValuesOnly, tick_ms, &mut handler)
}

#[derive(Clone, Copy)]
enum Decode {
    Full,
    ValuesOnly,
}

fn run_with(
    decode: Decode,
    tick_ms: u32,
    handler: &mut impl FnMut(Event<'_>) -> Flow,
) -> Result<Devices, String> {
    let window = create_window()?;
    let result = message_loop(window, decode, tick_ms, handler);
    // SAFETY: `window` was created above on this thread and is not used after this.
    let _ = unsafe { DestroyWindow(window) };
    result
}

fn message_loop(
    window: HWND,
    decode: Decode,
    tick_ms: u32,
    handler: &mut impl FnMut(Event<'_>) -> Flow,
) -> Result<Devices, String> {
    let mut devices = enumerate()?;
    register_for_input(window)?;
    for device in devices.values() {
        handler(Event::Connected(device));
    }
    // SAFETY: `window` is valid and belongs to this thread.
    if unsafe { SetTimer(Some(window), TIMER_ID, tick_ms, None) } == 0 {
        return Err("could not start the timer".to_owned());
    }

    let mut buffer = vec![0u64; 512];
    let mut message = MSG::default();
    loop {
        // SAFETY: `message` is writable; a null filter window receives this thread's messages.
        let got = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if got.0 <= 0 {
            break;
        }
        match message.message {
            WM_INPUT => {
                handle_input(message.lParam, decode, &mut buffer, &mut devices, handler)?;
            }
            WM_INPUT_DEVICE_CHANGE => {
                handle_device_change(message.wParam, message.lParam, &mut devices, handler);
            }
            WM_TIMER => {
                if handler(Event::Tick(&devices)) == Flow::Stop {
                    break;
                }
            }
            // SAFETY: `message` was just filled in by GetMessageW.
            _ => unsafe {
                DispatchMessageW(&message);
            },
        }
    }
    // SAFETY: the timer was started on this window above.
    let _ = unsafe { KillTimer(Some(window), TIMER_ID) };
    Ok(devices)
}

fn handle_input(
    lparam: LPARAM,
    decode: Decode,
    buffer: &mut Vec<u64>,
    devices: &mut Devices,
    handler: &mut impl FnMut(Event<'_>) -> Flow,
) -> Result<(), String> {
    let received = Instant::now();
    let handle = HRAWINPUT(lparam.0 as *mut _);
    let header_size = size_of::<RAWINPUTHEADER>() as u32;
    let mut size = 0u32;
    // SAFETY: a null buffer asks only for the required size.
    let probe = unsafe { GetRawInputData(handle, RID_INPUT, None, &mut size, header_size) };
    if probe == u32::MAX {
        return Err("could not size an input report".to_owned());
    }
    let needed = (size as usize).div_ceil(size_of::<u64>());
    if buffer.len() < needed {
        buffer.resize(needed, 0);
    }
    // SAFETY: `buffer` holds at least `size` bytes.
    let copied = unsafe {
        GetRawInputData(
            handle,
            RID_INPUT,
            Some(buffer.as_mut_ptr().cast()),
            &mut size,
            header_size,
        )
    };
    if copied == u32::MAX {
        return Err("could not read an input report".to_owned());
    }

    // SAFETY: the system wrote a RAWINPUT into `buffer`, whose u64 storage satisfies alignment.
    let raw = unsafe { &*buffer.as_ptr().cast::<RAWINPUT>() };
    if raw.header.dwType != RIM_TYPEHID.0 {
        return Ok(());
    }
    let key = raw.header.hDevice.0 as isize;
    if !devices.contains_key(&key)
        && let Ok(Some(device)) = Device::open(raw.header.hDevice)
    {
        handler(Event::Connected(&device));
        devices.insert(key, device);
    }
    let Some(device) = devices.get_mut(&key) else {
        return Ok(());
    };

    // SAFETY: dwType is HID, so `hid` is the active union member.
    let hid = unsafe { raw.data.hid };
    let report_size = hid.dwSizeHid as usize;
    let total = report_size.saturating_mul(hid.dwCount as usize);
    let offset = std::mem::offset_of!(RAWINPUT, data) + std::mem::offset_of!(RAWHID, bRawData);
    if report_size == 0 || offset.saturating_add(total) > copied as usize {
        return Ok(());
    }
    // SAFETY: the range was checked against the byte count the system copied into `buffer`.
    let bytes = unsafe {
        std::slice::from_raw_parts_mut(buffer.as_mut_ptr().cast::<u8>().add(offset), total)
    };
    for report in bytes.chunks_exact_mut(report_size) {
        let changes = match decode {
            Decode::Full => device.apply_report(report),
            Decode::ValuesOnly => {
                device.apply_values(report);
                Vec::new()
            }
        };
        handler(Event::Report {
            device,
            changes: &changes,
            received,
        });
    }
    Ok(())
}

fn handle_device_change(
    wparam: WPARAM,
    lparam: LPARAM,
    devices: &mut Devices,
    handler: &mut impl FnMut(Event<'_>) -> Flow,
) {
    let handle = HANDLE(lparam.0 as *mut _);
    let key = lparam.0;
    match wparam.0 {
        GIDC_ARRIVAL => {
            if !devices.contains_key(&key)
                && let Ok(Some(device)) = Device::open(handle)
            {
                handler(Event::Connected(&device));
                devices.insert(key, device);
            }
        }
        GIDC_REMOVAL => {
            if let Some(device) = devices.remove(&key) {
                handler(Event::Disconnected(&device));
            }
        }
        _ => {}
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: the arguments are passed through unchanged from the system.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

fn create_window() -> Result<HWND, String> {
    // SAFETY: a null name returns this executable's module handle.
    let module = unsafe { GetModuleHandleW(None) }
        .map_err(|error| format!("could not get the module handle: {error}"))?;
    let instance = HINSTANCE(module.0);
    let class_name = w!("wd_input_window");
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        lpszClassName: class_name,
        ..Default::default()
    };
    // SAFETY: `class` is fully initialised and its name is a static string.
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err("could not register the input window class".to_owned());
    }
    // SAFETY: the class was registered above; a message-only window is never shown.
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("wd_input"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance),
            None,
        )
    }
    .map_err(|error| format!("could not create the input window: {error}"))
}

fn register_for_input(window: HWND) -> Result<(), String> {
    let flags = RIDEV_INPUTSINK | RIDEV_DEVNOTIFY;
    let requests = [0x04u16, 0x05, 0x08].map(|usage| RAWINPUTDEVICE {
        usUsagePage: 0x01,
        usUsage: usage,
        dwFlags: flags,
        hwndTarget: window,
    });
    // SAFETY: `requests` is valid for the call, and the size matches its element type.
    unsafe { RegisterRawInputDevices(&requests, size_of::<RAWINPUTDEVICE>() as u32) }
        .map_err(|error| format!("could not register for input: {error}"))
}

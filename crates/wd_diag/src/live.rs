//! Device listing and the live display.

use crate::device::{Change, Device};
use std::collections::BTreeMap;
use std::io::{IsTerminal, Write};
use std::mem::size_of;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Console::{
    CONSOLE_MODE, ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode, GetStdHandle,
    STD_OUTPUT_HANDLE, SetConsoleMode,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceList, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE,
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
const REFRESH_TIMER: usize = 1;
const REFRESH_MS: u32 = 100;
const BAR_WIDTH: usize = 24;

/// Lists game controllers and their controls.
pub fn list() -> Result<(), String> {
    let devices = enumerate()?;
    println!("Read-only listing. Nothing is sent to any device.\n");
    if devices.is_empty() {
        println!("No game controllers found.");
        return Ok(());
    }
    for device in devices.values() {
        println!("{}", device.label());
        println!(
            "  Top-level usage {:04X}:{:04X}",
            device.top_page, device.top_usage
        );
        println!(
            "  Report sizes in bytes: input {}, output {}, feature {}",
            device.input_report_bytes, device.output_report_bytes, device.feature_report_bytes
        );
        println!("  Buttons: {}", device.button_count);
        println!("  Values: {}", device.values.len());
        for control in &device.values {
            println!(
                "    {:<10} {:>2} bits, reported range {}..={}",
                control.name(),
                control.bit_size,
                control.logical_min,
                control.logical_max
            );
        }
    }
    Ok(())
}

/// Shows live controls until the time limit, or until Ctrl+C when there is none.
pub fn run(seconds: Option<u32>, plain: bool) -> Result<(), String> {
    let plain = plain || !std::io::stdout().is_terminal();
    if !plain {
        enable_terminal_control();
    }
    let window = create_window()?;
    let result = message_loop(window, seconds, plain);
    // SAFETY: `window` was created above on this thread and is not used after this.
    let _ = unsafe { DestroyWindow(window) };
    result
}

fn message_loop(window: HWND, seconds: Option<u32>, plain: bool) -> Result<(), String> {
    let mut devices = enumerate()?;
    register_for_input(window)?;
    // SAFETY: `window` is valid and belongs to this thread.
    if unsafe { SetTimer(Some(window), REFRESH_TIMER, REFRESH_MS, None) } == 0 {
        return Err("could not start the refresh timer".to_owned());
    }
    let deadline = seconds.map(|s| Instant::now() + Duration::from_secs(u64::from(s)));
    if plain {
        println!("Read-only. Nothing is sent to any device.");
        for device in devices.values() {
            println!("connected: {}", device.label());
        }
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
                handle_input(message.lParam, &mut buffer, &mut devices, plain)?;
            }
            WM_INPUT_DEVICE_CHANGE => {
                handle_device_change(message.wParam, message.lParam, &mut devices, plain);
            }
            WM_TIMER => {
                if !plain {
                    draw(&devices);
                }
                if deadline.is_some_and(|end| Instant::now() >= end) {
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
    let _ = unsafe { KillTimer(Some(window), REFRESH_TIMER) };

    if plain {
        print_summary(&devices);
    }
    Ok(())
}

fn print_summary(devices: &BTreeMap<isize, Device>) {
    println!("summary:");
    for device in devices.values() {
        println!("  {}: {} reports", device.label(), device.reports);
        for control in &device.values {
            if let (Some(low), Some(high)) = (control.lowest, control.highest) {
                println!("    {:<10} seen {low}..={high}", control.name());
            }
        }
    }
}

fn handle_input(
    lparam: LPARAM,
    buffer: &mut Vec<u64>,
    devices: &mut BTreeMap<isize, Device>,
    plain: bool,
) -> Result<(), String> {
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
        devices.insert(key, device);
    }
    let Some(device) = devices.get_mut(&key) else {
        return Ok(());
    };

    // SAFETY: dwType is HID, so `hid` is the active union member.
    let hid = unsafe { raw.data.hid };
    let report_size = hid.dwSizeHid as usize;
    let count = hid.dwCount as usize;
    let total = report_size.saturating_mul(count);
    let offset = std::mem::offset_of!(RAWINPUT, data)
        + std::mem::offset_of!(windows::Win32::UI::Input::RAWHID, bRawData);
    if report_size == 0 || offset.saturating_add(total) > copied as usize {
        return Ok(());
    }
    // SAFETY: the range was checked against the byte count the system copied into `buffer`.
    let bytes = unsafe {
        std::slice::from_raw_parts_mut(buffer.as_mut_ptr().cast::<u8>().add(offset), total)
    };
    for report in bytes.chunks_exact_mut(report_size) {
        let changes = device.apply_report(report);
        if plain {
            print_changes(device, &changes);
        }
    }
    Ok(())
}

fn print_changes(device: &Device, changes: &[Change]) {
    for change in changes {
        match change {
            Change::Value { name, value } => {
                println!(
                    "{:04X}:{:04X} {name} = {value}",
                    device.vendor, device.product
                );
            }
            Change::Button {
                page,
                usage,
                pressed,
            } => {
                let state = if *pressed { "pressed" } else { "released" };
                println!(
                    "{:04X}:{:04X} button {page:04X}:{usage} {state}",
                    device.vendor, device.product
                );
            }
        }
    }
}

fn handle_device_change(
    wparam: WPARAM,
    lparam: LPARAM,
    devices: &mut BTreeMap<isize, Device>,
    plain: bool,
) {
    let handle = HANDLE(lparam.0 as *mut _);
    let key = lparam.0;
    match wparam.0 {
        GIDC_ARRIVAL => {
            if let Ok(Some(device)) = Device::open(handle) {
                if plain && !devices.contains_key(&key) {
                    println!("connected: {}", device.label());
                }
                devices.entry(key).or_insert(device);
            }
        }
        GIDC_REMOVAL => {
            if let Some(device) = devices.remove(&key)
                && plain
            {
                println!("disconnected: {}", device.label());
            }
        }
        _ => {}
    }
}

fn enumerate() -> Result<BTreeMap<isize, Device>, String> {
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
    let class_name = w!("wd_diag_input_window");
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
            w!("wd_diag"),
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

fn enable_terminal_control() {
    // SAFETY: only queries and adjusts this process's own console output mode.
    unsafe {
        if let Ok(handle) = GetStdHandle(STD_OUTPUT_HANDLE) {
            let mut mode = CONSOLE_MODE::default();
            if GetConsoleMode(handle, &mut mode).is_ok() {
                let _ = SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
            }
        }
    }
}

fn bar(fraction: Option<f64>) -> String {
    let Some(fraction) = fraction else {
        return format!("[{}]", "?".repeat(BAR_WIDTH));
    };
    let filled = (fraction * BAR_WIDTH as f64).round() as usize;
    format!(
        "[{}{}]",
        "#".repeat(filled.min(BAR_WIDTH)),
        ".".repeat(BAR_WIDTH.saturating_sub(filled))
    )
}

fn draw(devices: &BTreeMap<isize, Device>) {
    let mut out = String::from("\x1b[H\x1b[J");
    out.push_str("Read-only wheel diagnostic. Nothing is sent to any device. Ctrl+C to quit.\n\n");
    if devices.is_empty() {
        out.push_str("No game controller connected.\n");
    }
    for device in devices.values() {
        out.push_str(&format!(
            "{}  connected, {} reports\n",
            device.label(),
            device.reports
        ));
        for control in &device.values {
            let current = control
                .current
                .map_or_else(|| "no data".to_owned(), |value| value.to_string());
            let seen = match (control.lowest, control.highest) {
                (Some(low), Some(high)) => format!("seen {low}..={high}"),
                _ => String::new(),
            };
            out.push_str(&format!(
                "  {:<10} {} {current:>8}  {seen}\n",
                control.name(),
                bar(control.fraction())
            ));
        }
        let pressed: Vec<String> = device
            .pressed
            .iter()
            .map(|(_, usage)| usage.to_string())
            .collect();
        out.push_str(&format!(
            "  Buttons held: {}\n\n",
            if pressed.is_empty() {
                "none".to_owned()
            } else {
                pressed.join(" ")
            }
        ));
    }
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(out.as_bytes());
    let _ = stdout.flush();
}

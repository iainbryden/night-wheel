//! Device listing and the live display.

use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};
use wd_device::device::{Change, Device};
use wd_device::pump::{self, Devices, Event, Flow};
use windows::Win32::System::Console::{
    CONSOLE_MODE, ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode, GetStdHandle,
    STD_OUTPUT_HANDLE, SetConsoleMode,
};

const REFRESH_MS: u32 = 100;
const BAR_WIDTH: usize = 24;

/// Lists game controllers and their controls.
pub fn list() -> Result<(), String> {
    let devices = pump::enumerate()?;
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
    if plain {
        println!("Read-only. Nothing is sent to any device.");
    } else {
        enable_terminal_control();
    }
    let deadline = seconds.map(|s| Instant::now() + Duration::from_secs(u64::from(s)));
    let devices = pump::run(REFRESH_MS, |event| {
        match event {
            Event::Connected(device) if plain => println!("connected: {}", device.label()),
            Event::Disconnected(device) if plain => println!("disconnected: {}", device.label()),
            Event::Report { device, changes } if plain => print_changes(device, changes),
            Event::Tick(devices) => {
                if !plain {
                    draw(devices);
                }
                if deadline.is_some_and(|end| Instant::now() >= end) {
                    return Flow::Stop;
                }
            }
            _ => {}
        }
        Flow::Continue
    })?;
    if plain {
        print_summary(&devices);
    }
    Ok(())
}

fn print_summary(devices: &Devices) {
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

fn draw(devices: &Devices) {
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

//! Reads the wheel on its own thread as reports arrive. Read-only. Nothing on the per-report
//! path locks, allocates or touches a file.

use crate::{log_file, shared};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use wd_core::input::AxisId;
use wd_core::wizard::AxisMap;
use wd_device::device::Device;
use wd_device::pump::{Event, Flow, run_values_only};

/// Only used to notice a stop request. Reports do not wait for it.
const STOP_CHECK_MS: u32 = 250;
const STOP_WAIT: Duration = Duration::from_millis(1500);
const STOP_POLL: Duration = Duration::from_millis(20);

/// Shown before calibration: HID X, which is not assumed to be steering for driving.
const DISPLAY_AXIS: AxisId = AxisId {
    page: 0x01,
    usage: 0x30,
};

const HAT: AxisId = AxisId {
    page: 0x01,
    usage: 0x39,
};

fn reading(device: &Device, axis: AxisId) -> Option<i64> {
    device.value(axis.page, axis.usage)
}

fn readings(device: &Device, map: Option<AxisMap>) -> [Option<i64>; 3] {
    match map {
        Some(map) => [
            reading(device, map.steering),
            reading(device, map.throttle),
            reading(device, map.brake),
        ],
        None => [reading(device, DISPLAY_AXIS), None, None],
    }
}

pub struct WheelReader {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl WheelReader {
    pub fn start(map: Option<AxisMap>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            let result = run_values_only(STOP_CHECK_MS, |event| {
                match event {
                    Event::Report {
                        device, received, ..
                    } => {
                        if device.is_observed_target() {
                            shared::record_report(
                                readings(device, map),
                                device.held_buttons,
                                reading(device, HAT),
                                received,
                            );
                        }
                    }
                    Event::Connected(device) => {
                        if device.is_observed_target() {
                            shared::wheel_connected();
                        }
                        log_file::send(format!("controller connected {}", device.label()));
                    }
                    Event::Disconnected(device) => {
                        if device.is_observed_target() {
                            shared::wheel_lost();
                        }
                        log_file::send(format!("controller disconnected {}", device.label()));
                    }
                    Event::Notice(text) => {
                        log_file::send(format!("wheel reader: {text}"));
                    }
                    Event::Tick(_) => {
                        if thread_stop.load(Ordering::Relaxed) {
                            return Flow::Stop;
                        }
                    }
                }
                Flow::Continue
            });
            // Whatever ended the reading, nothing may keep applying its last values.
            shared::wheel_lost();
            if let Err(problem) = result {
                log_file::send(format!("wheel reading stopped: {problem}"));
            }
        });
        Self { stop, thread }
    }

    /// Asks the thread to stop and waits briefly. It never blocks game shutdown indefinitely.
    /// Returns whether the thread has ended.
    pub fn stop(self) -> bool {
        self.stop.store(true, Ordering::Relaxed);
        let started = Instant::now();
        while !self.thread.is_finished() && started.elapsed() < STOP_WAIT {
            std::thread::sleep(STOP_POLL);
        }
        if self.thread.is_finished() {
            let _ = self.thread.join();
            true
        } else {
            log_file::send("wheel thread did not stop in time".to_owned());
            false
        }
    }
}

//! Reads the wheel on its own thread as reports arrive. Read-only. Nothing on the per-report
//! path locks, allocates or touches a file.

use crate::{log_file, shared};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use wd_device::pump::{Event, Flow, run_values_only};

/// Only used to notice a stop request. Reports do not wait for it.
const STOP_CHECK_MS: u32 = 250;
const STOP_WAIT: Duration = Duration::from_millis(1500);
const STOP_POLL: Duration = Duration::from_millis(20);

const GENERIC_DESKTOP_PAGE: u16 = 0x01;
/// HID X. Taken as steering for this test; individual controls are still unconfirmed.
const X_USAGE: u16 = 0x30;

pub struct WheelReader {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl WheelReader {
    pub fn start() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            let result = run_values_only(STOP_CHECK_MS, |event| {
                match event {
                    Event::Report {
                        device, received, ..
                    } => {
                        if device.is_observed_target() {
                            let steering = device.value(GENERIC_DESKTOP_PAGE, X_USAGE);
                            shared::record_report(steering, received);
                        }
                    }
                    Event::Connected(device) => {
                        log_file::send(format!("controller connected {}", device.label()));
                    }
                    Event::Disconnected(device) => {
                        log_file::send(format!("controller disconnected {}", device.label()));
                    }
                    Event::Tick(_) => {
                        if thread_stop.load(Ordering::Relaxed) {
                            return Flow::Stop;
                        }
                    }
                }
                Flow::Continue
            });
            if let Err(problem) = result {
                log_file::send(format!("wheel reading stopped: {problem}"));
            }
        });
        Self { stop, thread }
    }

    /// Asks the thread to stop and waits briefly. It never blocks game shutdown indefinitely.
    pub fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        let started = Instant::now();
        while !self.thread.is_finished() && started.elapsed() < STOP_WAIT {
            std::thread::sleep(STOP_POLL);
        }
        if self.thread.is_finished() {
            let _ = self.thread.join();
        } else {
            log_file::send("wheel thread did not stop in time".to_owned());
        }
    }
}

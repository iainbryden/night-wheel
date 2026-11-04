//! Reads devices on a background thread and shares their latest state.

use crate::pump::{self, Devices, Event, Flow};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use wd_core::input::Sample;

const TICK_MS: u32 = 20;

/// The latest known state of one device.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceView {
    pub label: String,
    pub vendor: u32,
    pub product: u32,
    pub is_observed_target: bool,
    pub reports: u64,
    pub sample: Sample,
}

#[derive(Default)]
struct Shared {
    devices: Vec<DeviceView>,
    error: Option<String>,
}

/// Owns the background reader. Dropping it stops the thread.
pub struct Monitor {
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

fn views(devices: &Devices) -> Vec<DeviceView> {
    devices
        .values()
        .map(|device| DeviceView {
            label: device.label(),
            vendor: device.vendor,
            product: device.product,
            is_observed_target: device.is_observed_target(),
            reports: device.reports,
            sample: device.sample(),
        })
        .collect()
}

impl Monitor {
    /// Starts reading. `on_change` runs on the background thread whenever state changes.
    pub fn start(on_change: impl Fn() + Send + 'static) -> Self {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_shared = Arc::clone(&shared);
        let thread_stop = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            let result = pump::run(TICK_MS, |event| {
                if let Event::Tick(devices) = event {
                    let latest = views(devices);
                    let mut state = thread_shared.lock().unwrap_or_else(PoisonError::into_inner);
                    if state.devices != latest {
                        state.devices = latest;
                        drop(state);
                        on_change();
                    }
                    if thread_stop.load(Ordering::Relaxed) {
                        return Flow::Stop;
                    }
                }
                Flow::Continue
            });
            if let Err(error) = result {
                let mut state = thread_shared.lock().unwrap_or_else(PoisonError::into_inner);
                state.error = Some(error);
                state.devices.clear();
                drop(state);
                on_change();
            }
        });
        Self {
            shared,
            stop,
            thread: Some(thread),
        }
    }

    /// The latest state of every connected game controller.
    pub fn devices(&self) -> Vec<DeviceView> {
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .devices
            .clone()
    }

    /// Why reading stopped, if it failed.
    pub fn error(&self) -> Option<String> {
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .error
            .clone()
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

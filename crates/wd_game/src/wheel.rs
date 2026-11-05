//! Counts wheel reports on a background thread and logs a summary once a second. Read-only.

use crate::log_file;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use wd_device::pump::{Event, Flow, run};

const SUMMARY_MS: u32 = 1000;
const STOP_WAIT: Duration = Duration::from_millis(1500);
const STOP_POLL: Duration = Duration::from_millis(20);

pub struct WheelReader {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

fn summary(reports: u64, paused: bool) -> Option<String> {
    (reports > 0).then(|| format!("wheel reports={reports} paused={paused}"))
}

impl WheelReader {
    pub fn start(paused: &'static AtomicBool) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            let mut reports = 0u64;
            let result = run(SUMMARY_MS, |event| {
                match event {
                    Event::Connected(device) => {
                        log_file::write(&format!("controller connected {}", device.label()));
                    }
                    Event::Disconnected(device) => {
                        log_file::write(&format!("controller disconnected {}", device.label()));
                    }
                    Event::Report { device, .. } => {
                        if device.is_observed_target() {
                            reports += 1;
                        }
                    }
                    Event::Tick(_) => {
                        if let Some(line) = summary(reports, paused.load(Ordering::Relaxed)) {
                            log_file::write(&line);
                        }
                        reports = 0;
                        if thread_stop.load(Ordering::Relaxed) {
                            return Flow::Stop;
                        }
                    }
                }
                Flow::Continue
            });
            if let Err(problem) = result {
                log_file::write(&format!("wheel reading stopped: {problem}"));
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
            log_file::write("wheel thread did not stop in time");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::summary;

    #[test]
    fn quiet_seconds_write_nothing() {
        assert_eq!(summary(0, false), None);
    }

    #[test]
    fn a_summary_records_count_and_pause_state() {
        assert_eq!(
            summary(57, true).as_deref(),
            Some("wheel reports=57 paused=true")
        );
    }
}

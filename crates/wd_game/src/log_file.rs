//! Writes the local log on its own thread, so no file write ever runs on the game's thread or the
//! wheel reader. Failing to log must never disturb the game. Each launch starts a new file and keeps
//! the previous one, and a file stops growing at a size limit. Nothing is ever sent anywhere.

use crate::shared;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use wd_core::config;

const SUMMARY_EVERY: Duration = Duration::from_secs(1);
/// The longest the thread sleeps before noticing a stop request.
const STOP_CHECK: Duration = Duration::from_millis(100);
const STOP_WAIT: Duration = Duration::from_millis(1500);
const STOP_POLL: Duration = Duration::from_millis(20);
const MAX_LOG_BYTES: u64 = 20 * 1024 * 1024;

static SENDER: OnceLock<Sender<String>> = OnceLock::new();

pub struct Logger {
    stop: &'static AtomicBool,
    thread: JoinHandle<()>,
}

fn path() -> Option<PathBuf> {
    Some(config::settings_folder()?.join("night-wheel.log"))
}

/// Starts this launch's file, keeping the last one beside it.
fn open() -> Option<File> {
    let path = path()?;
    fs::create_dir_all(path.parent()?).ok()?;
    let _ = fs::rename(&path, path.with_extension("old.log"));
    OpenOptions::new().create(true).append(true).open(path).ok()
}

/// Whether a line still fits under the size limit.
fn has_room(written: u64, line: u64) -> bool {
    written.saturating_add(line) <= MAX_LOG_BYTES
}

/// Keeps one entry on one line, so a stray value cannot forge another entry.
fn format_line(millis: u128, message: &str) -> String {
    let clean: String = message
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    format!("{millis} {clean}\n")
}

fn write_line(file: &mut Option<File>, written: &mut u64, message: &str) {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    if let Some(file) = file {
        let line = format_line(millis, message);
        if has_room(*written, line.len() as u64) && file.write_all(line.as_bytes()).is_ok() {
            *written += line.len() as u64;
        }
    }
}

/// Queues a message. This allocates, so it is only for occasional events.
pub fn send(message: String) {
    if let Some(sender) = SENDER.get() {
        let _ = sender.send(message);
    }
}

impl Logger {
    pub fn start(stop: &'static AtomicBool) -> Self {
        let (sender, receiver) = mpsc::channel();
        let _ = SENDER.set(sender);
        let thread = std::thread::spawn(move || {
            let mut file = open();
            let mut written = 0;
            let mut next = Instant::now() + SUMMARY_EVERY;
            while !stop.load(Ordering::Relaxed) {
                let wait = next
                    .saturating_duration_since(Instant::now())
                    .min(STOP_CHECK);
                match receiver.recv_timeout(wait) {
                    Ok(message) => write_line(&mut file, &mut written, &message),
                    Err(RecvTimeoutError::Timeout) => {
                        if Instant::now() < next {
                            continue;
                        }
                        if let Some(line) = shared::stats_line() {
                            write_line(&mut file, &mut written, &line);
                        }
                        next = Instant::now() + SUMMARY_EVERY;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
            while let Ok(message) = receiver.try_recv() {
                write_line(&mut file, &mut written, &message);
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
        let finished = self.thread.is_finished();
        if finished {
            let _ = self.thread.join();
        }
        finished
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_LOG_BYTES, format_line, has_room};

    #[test]
    fn the_log_stops_at_its_limit() {
        assert!(has_room(0, 10));
        assert!(has_room(MAX_LOG_BYTES - 10, 10));
        assert!(!has_room(MAX_LOG_BYTES - 9, 10));
        assert!(!has_room(u64::MAX, 1));
    }

    #[test]
    fn a_line_has_time_and_message() {
        assert_eq!(format_line(12, "plugin loaded"), "12 plugin loaded\n");
    }

    #[test]
    fn line_breaks_cannot_create_extra_entries() {
        assert_eq!(format_line(1, "a\nb\r"), "1 a b \n");
    }
}

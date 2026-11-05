//! Writes test results on its own thread, so no file write ever runs on the game's thread
//! or the wheel reader. Failing to log must never disturb the game.

use crate::shared;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SUMMARY_EVERY: Duration = Duration::from_secs(1);
const STOP_WAIT: Duration = Duration::from_millis(1500);
const STOP_POLL: Duration = Duration::from_millis(20);

static SENDER: OnceLock<Sender<String>> = OnceLock::new();

pub struct Logger {
    stop: &'static AtomicBool,
    thread: JoinHandle<()>,
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join("WheelDrive").join("load-test.log"))
}

fn open() -> Option<File> {
    let path = path()?;
    fs::create_dir_all(path.parent()?).ok()?;
    OpenOptions::new().create(true).append(true).open(path).ok()
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

fn write_line(file: &mut Option<File>, message: &str) {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    if let Some(file) = file {
        let _ = file.write_all(format_line(millis, message).as_bytes());
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
            let mut next = Instant::now() + SUMMARY_EVERY;
            loop {
                let wait = next.saturating_duration_since(Instant::now());
                match receiver.recv_timeout(wait) {
                    Ok(message) => write_line(&mut file, &message),
                    Err(RecvTimeoutError::Timeout) => {
                        if let Some(line) = shared::stats_line() {
                            write_line(&mut file, &line);
                        }
                        next = Instant::now() + SUMMARY_EVERY;
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
            while let Ok(message) = receiver.try_recv() {
                write_line(&mut file, &message);
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format_line;

    #[test]
    fn a_line_has_time_and_message() {
        assert_eq!(format_line(12, "plugin loaded"), "12 plugin loaded\n");
    }

    #[test]
    fn line_breaks_cannot_create_extra_entries() {
        assert_eq!(format_line(1, "a\nb\r"), "1 a b \n");
    }
}

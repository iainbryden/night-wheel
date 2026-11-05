//! Game test plugin. It reads the wheel, gives scripts its value for an on-screen test during
//! pause, and measures timing. It sends nothing to the wheel and calls no game function.

// The loader entry points are compiled out of test builds, which leaves their callees unused.
#![cfg_attr(test, allow(dead_code, unused_imports))]

mod latency;
mod log_file;
mod shared;
mod wheel;

use red4ext_rs::{
    Exportable, GlobalExport, Plugin, SdkEnv, SemVer, U16CStr, export_plugin_symbols, exports,
    global, log, wcstr,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

static LOGGER_STOP: AtomicBool = AtomicBool::new(false);
static THREADS: Mutex<Option<(wheel::WheelReader, log_file::Logger)>> = Mutex::new(None);

/// Added to the value a script sends, so a reply proves the plugin handled the call.
const PING_OFFSET: i32 = 1;

struct LoadTest;

impl Plugin for LoadTest {
    const AUTHOR: &'static U16CStr = wcstr!("Iain Bryden");
    const NAME: &'static U16CStr = wcstr!("WheelDriveLoadTest");
    const VERSION: SemVer = SemVer::new(0, 2, 0);

    fn exports() -> impl Exportable {
        exports![
            GlobalExport(global!(c"WheelDriveTest_Ping", ping)),
            GlobalExport(global!(c"WheelDriveTest_Event", event)),
            GlobalExport(global!(c"WheelDriveTest_Read", read)),
            GlobalExport(global!(c"WheelDriveTest_Key", key)),
        ]
    }

    fn on_load(env: &SdkEnv) {
        log::info!(env, "WheelDriveLoadTest loaded");
        let logger = log_file::Logger::start(&LOGGER_STOP);
        log_file::send("plugin loaded, version 0.2.0".to_owned());
        let reader = wheel::WheelReader::start();
        *THREADS.lock().unwrap_or_else(PoisonError::into_inner) = Some((reader, logger));
    }

    fn on_unload(_env: &SdkEnv) {
        log_file::send("plugin unloading".to_owned());
        let threads = THREADS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some((reader, logger)) = threads {
            reader.stop();
            logger.stop();
        }
    }
}

export_plugin_symbols!(LoadTest);

fn ping(value: i32) -> i32 {
    let reply = value.wrapping_add(PING_OFFSET);
    log_file::send(format!("script ping {value}, reply {reply}"));
    reply
}

fn event(name: String) {
    let paused = pause_after(&name, shared::PAUSED.load(Ordering::Relaxed));
    shared::PAUSED.store(paused, Ordering::Relaxed);
    log_file::send(format!("script event {name} paused={paused}"));
}

fn read() -> i32 {
    shared::read_steering()
}

fn key() -> i32 {
    shared::record_key() as i32
}

/// The pause state once a script event has been seen.
fn pause_after(event: &str, current: bool) -> bool {
    match event {
        "session-pause" => true,
        "session-resume" | "session-end" => false,
        _ => current,
    }
}

#[cfg(test)]
mod tests {
    use super::pause_after;

    #[test]
    fn pause_follows_session_events() {
        assert!(pause_after("session-pause", false));
        assert!(!pause_after("session-resume", true));
    }

    #[test]
    fn ending_a_session_clears_pause() {
        assert!(!pause_after("session-end", true));
    }

    #[test]
    fn other_events_leave_pause_alone() {
        assert!(pause_after("key-press", true));
        assert!(!pause_after("key-press", false));
    }
}

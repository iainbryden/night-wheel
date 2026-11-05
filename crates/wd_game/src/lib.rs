//! First game load test. It answers whether the plugin loads, whether a script can call it,
//! and whether wheel reports and pause events arrive. It writes only to its own log file,
//! sends nothing to the wheel, and calls no game function.

// The loader entry points are compiled out of test builds, which leaves their callees unused.
#![cfg_attr(test, allow(dead_code, unused_imports))]

mod log_file;
mod wheel;

use red4ext_rs::{
    Exportable, GlobalExport, Plugin, SdkEnv, SemVer, U16CStr, export_plugin_symbols, exports,
    global, log, wcstr,
};
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::{AtomicBool, Ordering};

static PAUSED: AtomicBool = AtomicBool::new(false);
static WHEEL: Mutex<Option<wheel::WheelReader>> = Mutex::new(None);

/// Added to the value a script sends, so a reply proves the plugin handled the call.
const PING_OFFSET: i32 = 1;

struct LoadTest;

impl Plugin for LoadTest {
    const AUTHOR: &'static U16CStr = wcstr!("Iain Bryden");
    const NAME: &'static U16CStr = wcstr!("WheelDriveLoadTest");
    const VERSION: SemVer = SemVer::new(0, 1, 0);

    fn exports() -> impl Exportable {
        exports![
            GlobalExport(global!(c"WheelDriveTest_Ping", ping)),
            GlobalExport(global!(c"WheelDriveTest_Event", event)),
        ]
    }

    fn on_load(env: &SdkEnv) {
        log::info!(env, "WheelDriveLoadTest loaded");
        log_file::write("plugin loaded");
        let reader = wheel::WheelReader::start(&PAUSED);
        *WHEEL.lock().unwrap_or_else(PoisonError::into_inner) = Some(reader);
    }

    fn on_unload(_env: &SdkEnv) {
        log_file::write("plugin unloading");
        if let Some(reader) = WHEEL.lock().unwrap_or_else(PoisonError::into_inner).take() {
            reader.stop();
        }
    }
}

export_plugin_symbols!(LoadTest);

fn ping(value: i32) -> i32 {
    let reply = value.wrapping_add(PING_OFFSET);
    log_file::write(&format!("script ping {value}, reply {reply}"));
    reply
}

fn event(name: String) {
    let paused = pause_after(&name, PAUSED.load(Ordering::Relaxed));
    PAUSED.store(paused, Ordering::Relaxed);
    log_file::write(&format!("script event {name} paused={paused}"));
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

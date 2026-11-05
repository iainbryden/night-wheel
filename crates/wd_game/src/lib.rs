//! Game test plugin. It reads the wheel, answers the game's gamepad polls with calibrated wheel
//! values while the player is driving, and measures timing. It sends nothing to the wheel.

// The loader entry points are compiled out of test builds, which leaves their callees unused.
#![cfg_attr(test, allow(dead_code, unused_imports))]

mod drive;
mod latency;
mod log_file;
mod shared;
mod wheel;
mod xinput_hook;

use red4ext_rs::{
    Exportable, GlobalExport, Plugin, SdkEnv, SemVer, U16CStr, export_plugin_symbols, exports,
    global, log, wcstr,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use wd_core::config;

static LOGGER_STOP: AtomicBool = AtomicBool::new(false);
static THREADS: Mutex<Option<(wheel::WheelReader, log_file::Logger)>> = Mutex::new(None);

/// Added to the value a script sends, so a reply proves the plugin handled the call.
const PING_OFFSET: i32 = 1;

struct LoadTest;

impl Plugin for LoadTest {
    const AUTHOR: &'static U16CStr = wcstr!("Iain Bryden");
    const NAME: &'static U16CStr = wcstr!("WheelDriveLoadTest");
    const VERSION: SemVer = SemVer::new(0, 8, 0);

    fn exports() -> impl Exportable {
        exports![
            GlobalExport(global!(c"WheelDriveTest_Ping", ping)),
            GlobalExport(global!(c"WheelDriveTest_Event", event)),
            GlobalExport(global!(c"WheelDriveTest_Read", read)),
            GlobalExport(global!(c"WheelDriveTest_Key", key)),
            GlobalExport(global!(c"WheelDriveTest_SetDriving", set_driving)),
            GlobalExport(global!(c"WheelDriveTest_SteeringAction", steering_action)),
            GlobalExport(global!(c"WheelDriveTest_SteeringSpeed", steering_speed)),
        ]
    }

    fn on_load(env: &SdkEnv) {
        log::info!(env, "WheelDriveLoadTest loaded");
        let logger = log_file::Logger::start(&LOGGER_STOP);
        log_file::send("plugin loaded, version 0.8.0".to_owned());
        load_profile();
        load_steering_speed();
        xinput_hook::attach(env);
        let map = shared::PROFILE.get().map(|profile| profile.axes);
        let reader = wheel::WheelReader::start(map);
        *THREADS.lock().unwrap_or_else(PoisonError::into_inner) = Some((reader, logger));
    }

    fn on_unload(env: &SdkEnv) {
        log_file::send(shared::curve_line());
        log_file::send("plugin unloading".to_owned());
        xinput_hook::detach(env);
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

/// Without usable settings the wheel never drives; the game keeps its normal input.
fn load_profile() {
    let Some(base) = std::env::var_os("LOCALAPPDATA") else {
        log_file::send("no settings folder, so the wheel will not drive".to_owned());
        return;
    };
    let path = PathBuf::from(base).join("WheelDrive").join("profile.cfg");
    match config::load(&path) {
        Ok(loaded) => {
            log_file::send(format!(
                "settings loaded: steering {} throttle {} brake {}",
                loaded.config.axes.steering, loaded.config.axes.throttle, loaded.config.axes.brake
            ));
            let _ = shared::PROFILE.set(loaded.config);
        }
        Err(problem) => {
            log_file::send(format!(
                "no usable settings, so the wheel will not drive: {problem}"
            ));
        }
    }
}

/// Read before the game asks for it. A missing or unusable file keeps the proven default.
fn load_steering_speed() {
    let path = std::env::var_os("LOCALAPPDATA")
        .map(|base| PathBuf::from(base).join("WheelDrive").join("steering-speed.txt"));
    let text = path.and_then(|path| std::fs::read_to_string(path).ok());
    let chosen = text.as_deref().and_then(drive::parse_steering_speed);
    let value = chosen.unwrap_or(drive::DEFAULT_STEERING_SPEED);
    shared::set_steering_speed(value);
    let source = match (&text, chosen) {
        (_, Some(_)) => "from file",
        (Some(_), None) => "file not usable, so the default",
        (None, None) => "no file, so the default",
    };
    log_file::send(format!("steering speed x{value}, {source}"));
}

fn steering_speed() -> f32 {
    shared::steering_speed()
}

fn set_driving(driving: bool) {
    shared::set_driving(driving);
}

fn steering_action(value: f32) {
    shared::record_steering_action(value);
}

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

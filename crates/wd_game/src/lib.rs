//! Game test plugin. It reads the wheel, answers the game's gamepad polls with calibrated wheel
//! values while the player is driving, and measures timing. It sends nothing to the wheel.

// The loader entry points are compiled out of test builds, which leaves their callees unused.
#![cfg_attr(test, allow(dead_code, unused_imports))]

mod drive;
mod latency;
mod log_file;
mod settings;
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
use wd_core::config::{self, Config};

static LOGGER_STOP: AtomicBool = AtomicBool::new(false);
static THREADS: Mutex<Option<(wheel::WheelReader, log_file::Logger)>> = Mutex::new(None);
/// The settings, with their bindings, as the settings screen edits them.
static SETTINGS: Mutex<Option<Config>> = Mutex::new(None);
static SCREEN: Mutex<Option<settings::Screen>> = Mutex::new(None);

/// Added to the value a script sends, so a reply proves the plugin handled the call.
const PING_OFFSET: i32 = 1;

struct LoadTest;

impl Plugin for LoadTest {
    const AUTHOR: &'static U16CStr = wcstr!("Iain Bryden");
    const NAME: &'static U16CStr = wcstr!("WheelDriveLoadTest");
    const VERSION: SemVer = SemVer::new(0, 17, 0);

    fn exports() -> impl Exportable {
        exports![
            GlobalExport(global!(c"WheelDriveTest_Ping", ping)),
            GlobalExport(global!(c"WheelDriveTest_Event", event)),
            GlobalExport(global!(c"WheelDriveTest_Read", read)),
            GlobalExport(global!(c"WheelDriveTest_Key", key)),
            GlobalExport(global!(c"WheelDriveTest_SetDriving", set_driving)),
            GlobalExport(global!(c"WheelDriveTest_SteeringAction", steering_action)),
            GlobalExport(global!(c"WheelDriveTest_SteeringSpeed", steering_speed)),
            GlobalExport(global!(c"WheelDriveTest_ScreenOpen", screen_open)),
            GlobalExport(global!(c"WheelDriveTest_ScreenClose", screen_close)),
            GlobalExport(global!(c"WheelDriveTest_ScreenCommand", screen_command)),
            GlobalExport(global!(c"WheelDriveTest_ScreenTick", screen_tick)),
            GlobalExport(global!(c"WheelDriveTest_ScreenRowCount", screen_row_count)),
            GlobalExport(global!(c"WheelDriveTest_ScreenRowLabel", screen_row_label)),
            GlobalExport(global!(c"WheelDriveTest_ScreenRowValue", screen_row_value)),
            GlobalExport(global!(c"WheelDriveTest_ScreenSelected", screen_selected)),
            GlobalExport(global!(c"WheelDriveTest_ScreenSelect", screen_select)),
            GlobalExport(global!(c"WheelDriveTest_ScreenRowLit", screen_row_lit)),
            GlobalExport(global!(c"WheelDriveTest_ScreenSection", screen_section)),
            GlobalExport(global!(
                c"WheelDriveTest_TakeScriptActions",
                take_script_actions
            )),
            GlobalExport(global!(c"WheelDriveTest_ScreenColumn", screen_column)),
            GlobalExport(global!(c"WheelDriveTest_ScreenBoostRow", screen_boost_row)),
            GlobalExport(global!(c"WheelDriveTest_BoostPercent", boost_percent)),
            GlobalExport(global!(c"WheelDriveTest_BoostAdjust", boost_adjust)),
            GlobalExport(global!(c"WheelDriveTest_BoostWanted", boost_wanted)),
            GlobalExport(global!(c"WheelDriveTest_ScriptHeld", script_held)),
            GlobalExport(global!(c"WheelDriveTest_Throttle", throttle)),
            GlobalExport(global!(c"WheelDriveTest_Brake", brake)),
            GlobalExport(global!(c"WheelDriveTest_SetAutopilot", set_autopilot)),
            GlobalExport(global!(c"WheelDriveTest_ScreenStatus", screen_status)),
            GlobalExport(global!(c"WheelDriveTest_ScreenHints", screen_hints)),
        ]
    }

    fn on_load(env: &SdkEnv) {
        log::info!(env, "WheelDriveLoadTest loaded");
        let logger = log_file::Logger::start(&LOGGER_STOP);
        log_file::send("plugin loaded, version 0.17.0".to_owned());
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

fn profile_path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join("WheelDrive").join("profile.cfg"))
}

/// Without usable settings the wheel never drives; the game keeps its normal input.
fn load_profile() {
    let Some(path) = profile_path() else {
        log_file::send("no settings folder, so the wheel will not drive".to_owned());
        return;
    };
    match config::load(&path) {
        Ok(loaded) => {
            log_file::send(format!(
                "settings loaded: steering {} throttle {} brake {}",
                loaded.config.axes.steering, loaded.config.axes.throttle, loaded.config.axes.brake
            ));
            shared::set_row_bits(&loaded.config.bindings);
            *SETTINGS.lock().unwrap_or_else(PoisonError::into_inner) = Some(loaded.config.clone());
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
    let path = std::env::var_os("LOCALAPPDATA").map(|base| {
        PathBuf::from(base)
            .join("WheelDrive")
            .join("steering-speed.txt")
    });
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

fn screen_open() {
    shared::controls_for_screen();
    *SCREEN.lock().unwrap_or_else(PoisonError::into_inner) = Some(settings::Screen::new());
}

fn screen_close() {
    *SCREEN.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

/// Publishes and saves changed bindings. A failed save says so on the screen, and the bindings
/// still apply until the game closes.
fn bindings_changed(config: &Config, screen: &mut settings::Screen) {
    shared::set_row_bits(&config.bindings);
    let saved = profile_path()
        .ok_or_else(|| "no settings folder".to_owned())
        .and_then(|path| config::save(&path, config).map_err(|problem| problem.to_string()));
    match saved {
        Ok(()) => {
            log_file::send(format!(
                "bindings saved: {}",
                config
                    .bindings
                    .iter()
                    .map(|(action, control)| format!("{}={control}", action.key()))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        Err(problem) => {
            screen.set_message("COULD NOT SAVE".to_owned());
            log_file::send(format!("bindings not saved: {problem}"));
        }
    }
}

fn screen_command(code: i32) {
    let Some(command) = settings::Command::from_code(code) else {
        return;
    };
    let mut screen = SCREEN.lock().unwrap_or_else(PoisonError::into_inner);
    let mut settings = SETTINGS.lock().unwrap_or_else(PoisonError::into_inner);
    let (Some(screen), Some(config)) = (screen.as_mut(), settings.as_mut()) else {
        return;
    };
    let held = shared::controls_held();
    if screen.command(command, shared::now_ms(), held, &mut config.bindings) {
        bindings_changed(config, screen);
    }
}

/// Runs the screen for one refresh. False means it should close.
fn screen_tick() -> bool {
    let mut screen = SCREEN.lock().unwrap_or_else(PoisonError::into_inner);
    let mut settings = SETTINGS.lock().unwrap_or_else(PoisonError::into_inner);
    let (Some(screen), Some(config)) = (screen.as_mut(), settings.as_mut()) else {
        return true;
    };
    let (held, pressed) = shared::controls_for_screen();
    if screen.wheel(shared::now_ms(), held, pressed, &mut config.bindings) {
        bindings_changed(config, screen);
    }
    !screen.done()
}

fn screen_row_count() -> i32 {
    settings::ROWS.len() as i32
}

fn screen_row_label(row: i32) -> String {
    settings::Screen::row_label(usize::try_from(row).unwrap_or(usize::MAX))
}

/// Runs `read` with the screen and its settings, or gives the default when either is missing.
fn with_screen<T: Default>(read: impl FnOnce(&settings::Screen, &Config) -> T) -> T {
    let screen = SCREEN.lock().unwrap_or_else(PoisonError::into_inner);
    let settings = SETTINGS.lock().unwrap_or_else(PoisonError::into_inner);
    match (screen.as_ref(), settings.as_ref()) {
        (Some(screen), Some(config)) => read(screen, config),
        _ => T::default(),
    }
}

fn screen_row_value(row: i32) -> String {
    with_screen(|screen, config| {
        screen.row_value(usize::try_from(row).unwrap_or(usize::MAX), &config.bindings)
    })
}

fn screen_selected() -> i32 {
    with_screen(|screen, _| screen.selected() as i32)
}

fn screen_select(row: i32) {
    let mut screen = SCREEN.lock().unwrap_or_else(PoisonError::into_inner);
    if let (Some(screen), Ok(row)) = (screen.as_mut(), usize::try_from(row)) {
        screen.select(row);
    }
}

fn screen_section(row: i32) -> String {
    settings::section_at(usize::try_from(row).unwrap_or(usize::MAX)).to_owned()
}

fn screen_column(row: i32) -> i32 {
    settings::column_of(usize::try_from(row).unwrap_or(usize::MAX)) as i32
}

fn screen_boost_row() -> i32 {
    settings::BOOST_ROW as i32
}

fn boost_percent() -> i32 {
    SETTINGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map_or(config::DEFAULT_BOOST_PERCENT, |config| config.boost_percent) as i32
}

/// Moves the boost strength in steps of 5, within the allowed range, and saves it.
fn boost_adjust(steps: i32) {
    let mut settings = SETTINGS.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(config) = settings.as_mut() else {
        return;
    };
    let range = config::BOOST_PERCENT_RANGE;
    let moved = (i64::from(config.boost_percent) + i64::from(steps) * 5)
        .clamp(i64::from(*range.start()), i64::from(*range.end())) as u32;
    if moved == config.boost_percent {
        return;
    }
    config.boost_percent = moved;
    let saved = profile_path()
        .ok_or_else(|| "no settings folder".to_owned())
        .and_then(|path| config::save(&path, config).map_err(|problem| problem.to_string()));
    match saved {
        Ok(()) => log_file::send(format!("boost strength saved: {moved}%")),
        Err(problem) => log_file::send(format!("boost strength not saved: {problem}")),
    }
}

fn script_held() -> i32 {
    shared::script_held()
}

fn throttle() -> f32 {
    shared::pedal(0)
}

fn brake() -> f32 {
    shared::pedal(1)
}

fn boost_wanted() -> bool {
    shared::boost_wanted()
}

fn set_autopilot(on: bool) {
    shared::set_autopilot(on);
}

fn take_script_actions() -> i32 {
    shared::take_script_actions()
}

fn screen_row_lit(row: i32) -> bool {
    let now = shared::now_ms();
    with_screen(|screen, _| screen.row_lit(usize::try_from(row).unwrap_or(usize::MAX), now))
}

fn screen_status() -> String {
    if SETTINGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_none()
    {
        return "NO CALIBRATION FILE, SO BINDING IS NOT AVAILABLE".to_owned();
    }
    with_screen(|screen, _| screen.status())
}

fn screen_hints() -> String {
    with_screen(|screen, _| screen.hints().to_owned())
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

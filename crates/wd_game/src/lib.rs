//! The game plugin. It reads the wheel, answers the game's gamepad polls with calibrated wheel
//! values while the player is driving, runs the settings page and measures timing. It sends nothing
//! to the wheel.

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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use wd_core::config::{self, Config};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN, GetModuleHandleExW,
};
use windows::core::PCWSTR;

static LOGGER_STOP: AtomicBool = AtomicBool::new(false);
static THREADS: Mutex<Option<(wheel::WheelReader, log_file::Logger)>> = Mutex::new(None);
/// The settings, with their bindings, as the settings screen edits them.
static SETTINGS: Mutex<Option<Config>> = Mutex::new(None);
static SCREEN: Mutex<Option<settings::Screen>> = Mutex::new(None);

const VERSION_MAJOR: u8 = 0;
const VERSION_MINOR: u16 = 22;
const VERSION_PATCH: u32 = 0;

struct NightWheel;

impl Plugin for NightWheel {
    const AUTHOR: &'static U16CStr = wcstr!("Iain Bryden");
    const NAME: &'static U16CStr = wcstr!("NightWheel");
    const VERSION: SemVer = SemVer::new(VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH);

    fn exports() -> impl Exportable {
        exports![
            GlobalExport(global!(c"NightWheel_Event", event)),
            GlobalExport(global!(c"NightWheel_SetDriving", set_driving)),
            GlobalExport(global!(c"NightWheel_SteeringAction", steering_action)),
            GlobalExport(global!(c"NightWheel_SteeringSpeed", steering_speed)),
            GlobalExport(global!(c"NightWheel_ScreenOpen", screen_open)),
            GlobalExport(global!(c"NightWheel_ScreenClose", screen_close)),
            GlobalExport(global!(c"NightWheel_ScreenCommand", screen_command)),
            GlobalExport(global!(c"NightWheel_ScreenTick", screen_tick)),
            GlobalExport(global!(c"NightWheel_ScreenRowCount", screen_row_count)),
            GlobalExport(global!(c"NightWheel_ScreenRowLabel", screen_row_label)),
            GlobalExport(global!(c"NightWheel_ScreenRowValue", screen_row_value)),
            GlobalExport(global!(c"NightWheel_ScreenSelected", screen_selected)),
            GlobalExport(global!(c"NightWheel_ScreenSelect", screen_select)),
            GlobalExport(global!(c"NightWheel_ScreenRowLit", screen_row_lit)),
            GlobalExport(global!(c"NightWheel_ScreenSection", screen_section)),
            GlobalExport(global!(
                c"NightWheel_TakeScriptActions",
                take_script_actions
            )),
            GlobalExport(global!(c"NightWheel_ScreenColumn", screen_column)),
            GlobalExport(global!(c"NightWheel_ScreenBoostRow", screen_boost_row)),
            GlobalExport(global!(c"NightWheel_BoostPercent", boost_percent)),
            GlobalExport(global!(c"NightWheel_BoostAdjust", boost_adjust)),
            GlobalExport(global!(c"NightWheel_BoostWanted", boost_wanted)),
            GlobalExport(global!(c"NightWheel_BoostHeld", boost_held)),
            GlobalExport(global!(c"NightWheel_ScriptHeld", script_held)),
            GlobalExport(global!(c"NightWheel_Throttle", throttle)),
            GlobalExport(global!(c"NightWheel_Brake", brake)),
            GlobalExport(global!(c"NightWheel_SetAutopilot", set_autopilot)),
            GlobalExport(global!(c"NightWheel_ScreenStatus", screen_status)),
            GlobalExport(global!(c"NightWheel_SliderCount", slider_count)),
            GlobalExport(global!(c"NightWheel_SliderLabel", slider_label)),
            GlobalExport(global!(c"NightWheel_SliderValue", slider_value)),
            GlobalExport(global!(c"NightWheel_SliderFraction", slider_fraction)),
            GlobalExport(global!(c"NightWheel_SliderAdjust", slider_adjust)),
            GlobalExport(global!(c"NightWheel_ScreenHints", screen_hints)),
        ]
    }

    fn on_load(env: &SdkEnv) {
        log::info!(env, "NightWheel loaded");
        let adopted = config::adopt_old_settings();
        let logger = log_file::Logger::start(&LOGGER_STOP);
        log_file::send(format!(
            "plugin loaded, version {}",
            env!("CARGO_PKG_VERSION")
        ));
        if adopted > 0 {
            log_file::send(format!(
                "copied {adopted} files from the old settings folder"
            ));
        }
        load_profile();
        load_steering_speed();
        xinput_hook::attach(env);
        let map = shared::PROFILE.get().map(|profile| profile.axes);
        let reader = wheel::WheelReader::start(map);
        *THREADS.lock().unwrap_or_else(PoisonError::into_inner) = Some((reader, logger));
    }

    fn on_unload(env: &SdkEnv) {
        log_file::send("plugin unloading".to_owned());
        xinput_hook::detach(env);
        let threads = THREADS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some((reader, logger)) = threads {
            let reader_ended = reader.stop();
            let logger_ended = logger.stop();
            if !(reader_ended && logger_ended) {
                keep_library_loaded();
            }
        }
    }
}

/// A thread that outlives its wait would crash if this library were unmapped under it, so the
/// library stays loaded until the process ends. Waiting forever could hang the game's shutdown.
fn keep_library_loaded() {
    let address = keep_library_loaded as *const () as *const u16;
    let mut module = HMODULE::default();
    // SAFETY: the flags make Windows read `address` as a location in this library, not as text,
    // and `module` is writable for the call.
    let _ = unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            PCWSTR(address),
            &mut module,
        )
    };
}

export_plugin_symbols!(NightWheel);

/// Without usable settings the wheel never drives; the game keeps its normal input.
fn load_profile() {
    let Some(path) = config::profile_path() else {
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
            shared::set_tuning(drive::Tuning::of(&loaded.config));
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

/// Read before the game asks for it. A choice in the settings wins. Otherwise a text file from an
/// earlier version is adopted, so the next save keeps it, and otherwise the proven default applies.
fn load_steering_speed() {
    let mut settings = SETTINGS.lock().unwrap_or_else(PoisonError::into_inner);
    let saved = settings.as_ref().and_then(|config| config.steering_speed);
    let (value, source) = if let Some(saved) = saved {
        (saved as f32, "from settings")
    } else {
        let path = config::settings_folder().map(|folder| folder.join("steering-speed.txt"));
        let text = path.and_then(|path| std::fs::read_to_string(path).ok());
        match text.as_deref().and_then(drive::parse_steering_speed) {
            Some(chosen) => {
                if let Some(config) = settings.as_mut() {
                    config.steering_speed = Some(f64::from(chosen));
                }
                (chosen, "from the old text file")
            }
            None => (drive::DEFAULT_STEERING_SPEED, "the default"),
        }
    };
    shared::set_steering_speed(value);
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
    let saved = config::profile_path()
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
    let saved = config::profile_path()
        .ok_or_else(|| "no settings folder".to_owned())
        .and_then(|path| config::save(&path, config).map_err(|problem| problem.to_string()));
    match saved {
        Ok(()) => log_file::send(format!("boost strength saved: {moved}%")),
        Err(problem) => log_file::send(format!("boost strength not saved: {problem}")),
    }
}

fn slider_count() -> i32 {
    settings::SLIDER_COUNT
}

fn slider_label(index: i32) -> String {
    settings::slider_label(index).to_owned()
}

fn slider_value(index: i32) -> String {
    SETTINGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map_or_else(String::new, |config| settings::slider_value(index, config))
}

fn slider_fraction(index: i32) -> f32 {
    SETTINGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map_or(0.0, |config| {
            settings::slider_fraction(index, config) as f32
        })
}

/// Moves a steering slider, applies range and curve at once, and saves.
fn slider_adjust(index: i32, steps: i32) {
    let mut settings = SETTINGS.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(config) = settings.as_mut() else {
        return;
    };
    if !settings::slider_adjust(index, steps, config) {
        return;
    }
    shared::set_tuning(drive::Tuning::of(config));
    let saved = config::profile_path()
        .ok_or_else(|| "no settings folder".to_owned())
        .and_then(|path| config::save(&path, config).map_err(|problem| problem.to_string()));
    match saved {
        Ok(()) => log_file::send(format!(
            "steering saved: range {} curve {} speed {:?}",
            config.steering_range, config.steering.exponent, config.steering_speed
        )),
        Err(problem) => log_file::send(format!("steering not saved: {problem}")),
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

fn boost_held() -> bool {
    shared::boost_held()
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
    shared::set_driving(driving, xinput_hook::game_has_focus());
}

fn steering_action(_value: f32) {
    shared::record_steering_action();
}

fn event(name: String) {
    let paused = pause_after(&name, shared::PAUSED.load(Ordering::Relaxed));
    if shared::PAUSED.swap(paused, Ordering::Relaxed) != paused {
        shared::context_changed();
    }
    log_file::send(format!("script event {name} paused={paused}"));
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
    use super::{VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH, pause_after};

    #[test]
    fn the_plugin_version_matches_the_package() {
        assert_eq!(
            format!("{VERSION_MAJOR}.{VERSION_MINOR}.{VERSION_PATCH}"),
            env!("CARGO_PKG_VERSION")
        );
    }

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

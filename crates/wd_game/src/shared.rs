//! State shared between the wheel reader, the game's thread and the logger. Everything on
//! the wheel-to-game path is a plain atomic: no locks and no allocation.

use crate::drive;
use crate::latency::{Histogram, Summary};
use crate::settings::{self, ROWS, SCRIPT_BOOST, SCRIPT_SHIFT};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;
use wd_core::assignment::Bindings;
use wd_core::config::Config;

/// A read this soon after a report counts as "wheel moving" for value age.
const MOVING_WINDOW_US: u64 = 50_000;
/// Marks that no report has arrived yet.
const NEVER: u64 = u64::MAX;

static START: OnceLock<Instant> = OnceLock::new();

pub static PAUSED: AtomicBool = AtomicBool::new(false);
pub static WHEEL_CONNECTED: AtomicBool = AtomicBool::new(false);
/// Validated settings, when a usable file exists. Without them the wheel never drives.
pub static PROFILE: OnceLock<Config> = OnceLock::new();
static DRIVING: AtomicBool = AtomicBool::new(false);
static FOCUSED: AtomicBool = AtomicBool::new(false);
/// Counts changes of context: pause, focus, driving, connection, or a gap in the game's updates.
/// A press made before a change must never act after it, even if no poll saw the gate closed.
static CONTEXT: AtomicU32 = AtomicU32::new(0);
static CONTEXT_SEEN: AtomicU32 = AtomicU32::new(0);
static HEARTBEAT_US: AtomicU64 = AtomicU64::new(0);
/// Marks an axis with no reading since the wheel last connected.
const NO_READING: i32 = i32::MIN;
static AXES: [AtomicI32; 3] = [const { AtomicI32::new(NO_READING) }; 3];
static LAST_POLL_US: AtomicU64 = AtomicU64::new(NEVER);
static APPLIED: AtomicU32 = AtomicU32::new(0);
static LAST_REPORT_US: AtomicU64 = AtomicU64::new(NEVER);
static REPORTS: AtomicU32 = AtomicU32::new(0);
/// The controls held now, as control bits.
static CONTROLS: AtomicU64 = AtomicU64::new(0);
/// Presses since the game's last poll, and since the settings screen last looked. A dial click can
/// start and end between two polls, so each press is kept until it has been seen.
static PRESSED_FOR_POLL: AtomicU64 = AtomicU64::new(0);
static PRESSED_FOR_SCREEN: AtomicU64 = AtomicU64::new(0);
/// Controls that must be released before they count again.
static WAITING: AtomicU64 = AtomicU64::new(0);
/// The controls that counted at the last poll, to find fresh presses.
static LAST_COUNTED: AtomicU64 = AtomicU64::new(0);
/// Fresh presses of functions the game script carries out, until it takes them.
static SCRIPT_PENDING: AtomicU32 = AtomicU32::new(0);
/// Whether the boost control counts right now, and whether the brake allows it.
static BOOST_HELD: AtomicBool = AtomicBool::new(false);
static BRAKE_CLEAR: AtomicBool = AtomicBool::new(true);
/// The script functions whose controls count right now.
static SCRIPT_HELD: AtomicU32 = AtomicU32::new(0);
/// The latest throttle and brake values sent to the game, 0 to 255.
static PEDALS: [AtomicU32; 2] = [const { AtomicU32::new(0) }; 2];
/// True while the game's own driving is on, when the wheel must not steer or use the pedals.
static AUTOPILOT: AtomicBool = AtomicBool::new(false);
/// Functions the wheel has pressed since the last stats line.
static FIRED: AtomicU32 = AtomicU32::new(0);
/// The control bit bound to each settings row, or zero.
static ROW_BITS: [AtomicU64; ROWS.len()] = [const { AtomicU64::new(0) }; ROWS.len()];
/// The steering speed multiplier for this launch, set once before the game asks for it.
static STEERING_SPEED: OnceLock<f32> = OnceLock::new();

static DECODE: Histogram = Histogram::new();
static POLL_INTERVAL: Histogram = Histogram::new();
static POLL_AGE: Histogram = Histogram::new();
static ACTION_DELAY: Histogram = Histogram::new();

static LAST_THUMB: AtomicI32 = AtomicI32::new(0);
static THUMB_CHANGED_US: AtomicU64 = AtomicU64::new(NEVER);
static THUMB_CHANGE_SEEN_US: AtomicU64 = AtomicU64::new(NEVER);

fn micros_since_start(instant: Instant) -> u64 {
    let start = *START.get_or_init(Instant::now);
    instant.saturating_duration_since(start).as_micros() as u64
}

/// Called by the wheel reader for each wheel report.
pub fn record_report(axes: [Option<i64>; 3], buttons: u64, hat: Option<i64>, received: Instant) {
    let now = Instant::now();
    let controls = drive::control_bits(buttons, hat);
    let pressed = controls & !CONTROLS.swap(controls, Ordering::Relaxed);
    if pressed != 0 {
        PRESSED_FOR_POLL.fetch_or(pressed, Ordering::Relaxed);
        PRESSED_FOR_SCREEN.fetch_or(pressed, Ordering::Relaxed);
    }
    for (slot, value) in AXES.iter().zip(axes) {
        if let Some(value) = value {
            slot.store(value as i32, Ordering::Relaxed);
        }
    }
    LAST_REPORT_US.store(micros_since_start(now), Ordering::Relaxed);
    REPORTS.fetch_add(1, Ordering::Relaxed);
    DECODE.record(now.saturating_duration_since(received).as_micros() as u64);
}

/// Called when the wheel's context changes. Forgets functions waiting for the script.
pub fn context_changed() {
    CONTEXT.fetch_add(1, Ordering::Relaxed);
    SCRIPT_PENDING.store(0, Ordering::Relaxed);
}

/// Called on the game's thread each frame, with whether the player is driving and the game has
/// focus.
pub fn set_driving(driving: bool, focused: bool) {
    let now = micros_since_start(Instant::now());
    let previous = HEARTBEAT_US.swap(now, Ordering::Relaxed);
    let gap = now.saturating_sub(previous) > drive::HEARTBEAT_LIMIT_US;
    let was_driving = DRIVING.swap(driving, Ordering::Relaxed);
    let was_focused = FOCUSED.swap(focused, Ordering::Relaxed);
    if gap || was_driving != driving || was_focused != focused {
        context_changed();
    }
}

/// The latest raw steering, throttle and brake readings, where there are any.
pub fn raw_axes() -> [Option<i32>; 3] {
    [0, 1, 2].map(|axis| {
        let raw = AXES[axis].load(Ordering::Relaxed);
        (raw != NO_READING).then_some(raw)
    })
}

/// Called when the wheel appears. Anything already held must be released before it counts.
pub fn wheel_connected() {
    WHEEL_CONNECTED.store(true, Ordering::Relaxed);
    context_changed();
}

/// Called when the wheel leaves or can no longer be read. Forgets everything that came from it, so
/// nothing stays pressed and old readings can't apply after it returns.
pub fn wheel_lost() {
    WHEEL_CONNECTED.store(false, Ordering::Relaxed);
    for axis in &AXES {
        axis.store(NO_READING, Ordering::Relaxed);
    }
    CONTROLS.store(0, Ordering::Relaxed);
    PRESSED_FOR_POLL.store(0, Ordering::Relaxed);
    PRESSED_FOR_SCREEN.store(0, Ordering::Relaxed);
    BOOST_HELD.store(false, Ordering::Relaxed);
    SCRIPT_HELD.store(0, Ordering::Relaxed);
    set_pedals(0, 0);
    context_changed();
}

/// Whether the wheel may control the game at this moment.
pub fn wheel_may_apply() -> bool {
    let now = micros_since_start(Instant::now());
    drive::may_apply(
        WHEEL_CONNECTED.load(Ordering::Relaxed),
        DRIVING.load(Ordering::Relaxed),
        FOCUSED.load(Ordering::Relaxed),
        PAUSED.load(Ordering::Relaxed),
        now.saturating_sub(HEARTBEAT_US.load(Ordering::Relaxed)),
    )
}

/// Called each time the game polls for gamepad state.
pub fn record_poll() {
    let now = micros_since_start(Instant::now());
    let previous = LAST_POLL_US.swap(now, Ordering::Relaxed);
    if previous != NEVER {
        let interval = now.saturating_sub(previous);
        POLL_INTERVAL.record(interval);
        if interval > drive::HEARTBEAT_LIMIT_US {
            context_changed();
        }
    }
    let reported = LAST_REPORT_US.load(Ordering::Relaxed);
    if reported != NEVER {
        let age = now.saturating_sub(reported);
        if age <= MOVING_WINDOW_US {
            POLL_AGE.record(age);
        }
    }
}

/// Called when wheel values were given to the game.
pub fn record_applied(thumb: i16) {
    APPLIED.fetch_add(1, Ordering::Relaxed);
    if LAST_THUMB.swap(i32::from(thumb), Ordering::Relaxed) != i32::from(thumb) {
        THUMB_CHANGED_US.store(micros_since_start(Instant::now()), Ordering::Relaxed);
    }
}

/// Called when the game reports its steering action: how long after the stick last changed.
pub fn record_steering_action() {
    let changed = THUMB_CHANGED_US.load(Ordering::Relaxed);
    if changed != NEVER && THUMB_CHANGE_SEEN_US.swap(changed, Ordering::Relaxed) != changed {
        let now = micros_since_start(Instant::now());
        ACTION_DELAY.record(now.saturating_sub(changed));
    }
}

/// Called at each game poll. The gamepad buttons the wheel is pressing now. Atomics only.
pub fn pad_buttons(may_apply: bool) -> u16 {
    let held = CONTROLS.load(Ordering::Relaxed);
    let pressed = PRESSED_FOR_POLL.swap(0, Ordering::Relaxed);
    let waiting = WAITING.load(Ordering::Relaxed);
    let context = CONTEXT.load(Ordering::Relaxed);
    let changed = CONTEXT_SEEN.swap(context, Ordering::Relaxed) != context;
    let (counted, waiting) = drive::gate_controls(held, pressed, waiting, may_apply && !changed);
    WAITING.store(waiting, Ordering::Relaxed);
    let routes = |controls| {
        drive::route_flags(
            controls,
            ROW_BITS
                .iter()
                .zip(ROWS)
                .map(|(bit, (_, route))| (bit.load(Ordering::Relaxed), route)),
        )
    };
    let flags = routes(counted);
    BOOST_HELD.store(flags & SCRIPT_BOOST != 0, Ordering::Relaxed);
    SCRIPT_HELD.store(flags >> SCRIPT_SHIFT, Ordering::Relaxed);
    if flags != 0 {
        FIRED.fetch_or(flags, Ordering::Relaxed);
    }
    let fresh = counted & !LAST_COUNTED.swap(counted, Ordering::Relaxed);
    if fresh != 0 {
        let scripted = routes(fresh) >> SCRIPT_SHIFT;
        if scripted != 0 {
            SCRIPT_PENDING.fetch_or(scripted, Ordering::Relaxed);
        }
    }
    (flags & 0xFFFF) as u16
}

/// Whether what the last poll published still holds. The script reads it between polls, so it is
/// checked again here: the wheel may have left, or the game may have stopped polling.
fn script_inputs_live() -> bool {
    let polled = LAST_POLL_US.load(Ordering::Relaxed);
    wheel_may_apply()
        && polled != NEVER
        && micros_since_start(Instant::now()).saturating_sub(polled) <= drive::HEARTBEAT_LIMIT_US
}

/// Whether boost should push the car now. Touching the brake ends it.
pub fn boost_wanted() -> bool {
    script_inputs_live()
        && BOOST_HELD.load(Ordering::Relaxed)
        && BRAKE_CLEAR.load(Ordering::Relaxed)
}

pub fn script_held() -> i32 {
    if script_inputs_live() {
        SCRIPT_HELD.load(Ordering::Relaxed) as i32
    } else {
        0
    }
}

pub fn set_pedals(throttle: u8, brake: u8) {
    PEDALS[0].store(u32::from(throttle), Ordering::Relaxed);
    PEDALS[1].store(u32::from(brake), Ordering::Relaxed);
}

/// Throttle, then brake, from 0 to 1.
pub fn pedal(index: usize) -> f32 {
    if script_inputs_live() {
        PEDALS[index].load(Ordering::Relaxed) as f32 / 255.0
    } else {
        0.0
    }
}

pub fn set_brake_clear(clear: bool) {
    BRAKE_CLEAR.store(clear, Ordering::Relaxed);
}

pub fn set_autopilot(on: bool) {
    AUTOPILOT.store(on, Ordering::Relaxed);
}

pub fn autopilot() -> bool {
    AUTOPILOT.load(Ordering::Relaxed)
}

/// Called by the game script each frame while driving. The functions it should carry out now.
pub fn take_script_actions() -> i32 {
    SCRIPT_PENDING.swap(0, Ordering::Relaxed) as i32
}

/// Publishes bindings to the game's poll.
pub fn set_row_bits(bindings: &Bindings) {
    for (slot, (action, _)) in ROW_BITS.iter().zip(ROWS) {
        let bit = bindings.get(action).and_then(drive::bit_for).unwrap_or(0);
        slot.store(bit, Ordering::Relaxed);
    }
}

/// The controls held now, and those pressed since the settings screen last looked.
pub fn controls_for_screen() -> (u64, u64) {
    (
        CONTROLS.load(Ordering::Relaxed),
        PRESSED_FOR_SCREEN.swap(0, Ordering::Relaxed),
    )
}

pub fn controls_held() -> u64 {
    CONTROLS.load(Ordering::Relaxed)
}

pub fn now_ms() -> u64 {
    micros_since_start(Instant::now()) / 1000
}

pub fn set_steering_speed(value: f32) {
    let _ = STEERING_SPEED.set(value);
}

pub fn steering_speed() -> f32 {
    STEERING_SPEED
        .get()
        .copied()
        .unwrap_or(drive::DEFAULT_STEERING_SPEED)
}

fn format_summary(name: &str, summary: Summary) -> String {
    format!(
        "{name}[n={} p50<={} p99<={} max={}]",
        summary.count, summary.p50_us, summary.p99_us, summary.max_us
    )
}

/// One second's measurements, or nothing when the second was idle. Times are microseconds.
pub fn stats_line() -> Option<String> {
    let reports = REPORTS.swap(0, Ordering::Relaxed);
    let decode = DECODE.drain();
    let poll = POLL_INTERVAL.drain();
    let poll_age = POLL_AGE.drain();
    let applied = APPLIED.swap(0, Ordering::Relaxed);
    let action_delay = ACTION_DELAY.drain();
    if reports == 0 && poll.count == 0 {
        return None;
    }
    Some(format!(
        "stats paused={} driving={} connected={} reports={reports} applied={applied} fired={} {} {} {} {}",
        PAUSED.load(Ordering::Relaxed),
        DRIVING.load(Ordering::Relaxed),
        WHEEL_CONNECTED.load(Ordering::Relaxed),
        settings::fired_names(FIRED.swap(0, Ordering::Relaxed)),
        format_summary("decode", decode),
        format_summary("poll_interval", poll),
        format_summary("poll_age", poll_age),
        format_summary("action_delay", action_delay),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::latency::Summary;

    /// One test, because these share process-wide state.
    #[test]
    fn nothing_from_the_wheel_outlasts_it() {
        wheel_connected();
        set_driving(true, true);
        record_poll();
        BOOST_HELD.store(true, Ordering::Relaxed);
        SCRIPT_HELD.store(128, Ordering::Relaxed);
        SCRIPT_PENDING.store(1, Ordering::Relaxed);
        set_pedals(255, 0);
        assert!(boost_wanted());
        assert_eq!(script_held(), 128);
        assert_eq!(pedal(0), 1.0);

        // Losing focus hides it without clearing it.
        set_driving(true, false);
        assert!(!boost_wanted());
        assert_eq!(script_held(), 0);
        assert_eq!(pedal(0), 0.0);
        assert_eq!(take_script_actions(), 0);

        set_driving(true, true);
        wheel_lost();
        assert!(!WHEEL_CONNECTED.load(Ordering::Relaxed));
        assert_eq!(raw_axes(), [None, None, None]);
        assert!(!BOOST_HELD.load(Ordering::Relaxed));
        assert_eq!(SCRIPT_HELD.load(Ordering::Relaxed), 0);
        assert_eq!(PEDALS[0].load(Ordering::Relaxed), 0);

        // A press waiting when the context changes never acts.
        wheel_connected();
        set_driving(true, true);
        record_poll();
        CONTROLS.store(0b1, Ordering::Relaxed);
        PRESSED_FOR_POLL.store(0b1, Ordering::Relaxed);
        ROW_BITS[0].store(0b1, Ordering::Relaxed);
        assert_eq!(pad_buttons(true), 0);
        assert_eq!(pad_buttons(true), 0, "still held, so it needs a release");
        CONTROLS.store(0, Ordering::Relaxed);
        assert_eq!(pad_buttons(true), 0);
        CONTROLS.store(0b1, Ordering::Relaxed);
        assert_ne!(pad_buttons(true), 0, "a fresh press counts");
        ROW_BITS[0].store(0, Ordering::Relaxed);
        CONTROLS.store(0, Ordering::Relaxed);
    }

    #[test]
    fn a_summary_lists_all_its_numbers() {
        let summary = Summary {
            count: 3,
            p50_us: 8,
            p99_us: 64,
            max_us: 50,
        };
        assert_eq!(
            format_summary("decode", summary),
            "decode[n=3 p50<=8 p99<=64 max=50]"
        );
    }
}

//! State shared between the wheel reader, the game's thread and the logger. Everything on
//! the wheel-to-game path is a plain atomic: no locks and no allocation.

use crate::drive;
use crate::latency::{Histogram, Summary};
use crate::settings::{self, ROWS, SCRIPT_BOOST, SCRIPT_HOVER, SCRIPT_SHIFT};
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
static HEARTBEAT_US: AtomicU64 = AtomicU64::new(0);
/// Marks an axis with no reading since the wheel last connected.
const NO_READING: i32 = i32::MIN;
static AXES: [AtomicI32; 3] = [const { AtomicI32::new(NO_READING) }; 3];
static LAST_POLL_US: AtomicU64 = AtomicU64::new(NEVER);
static APPLIED: AtomicU32 = AtomicU32::new(0);
static LAST_REPORT_US: AtomicU64 = AtomicU64::new(NEVER);
static LAST_READ_US: AtomicU64 = AtomicU64::new(NEVER);
static REPORTS: AtomicU32 = AtomicU32::new(0);
static KEYS: AtomicU32 = AtomicU32::new(0);
static TOTAL_KEYS: AtomicU32 = AtomicU32::new(0);
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
static HOVER_HELD: AtomicBool = AtomicBool::new(false);
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
static READ_INTERVAL: Histogram = Histogram::new();
static VALUE_AGE: Histogram = Histogram::new();
static POLL_INTERVAL: Histogram = Histogram::new();
static POLL_AGE: Histogram = Histogram::new();
static ACTION_DELAY: Histogram = Histogram::new();

/// Number of stick ranges in the stick-to-action table.
const CURVE_BUCKETS: usize = 41;
/// Marks a table entry with no observation.
const NO_VALUE: u32 = u32::MAX;
static CURVE: [AtomicU32; CURVE_BUCKETS] = [const { AtomicU32::new(NO_VALUE) }; CURVE_BUCKETS];
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

/// Called on the game's thread each time it reads the wheel. Returns the steering value.
pub fn read_steering() -> i32 {
    let now = micros_since_start(Instant::now());
    let previous = LAST_READ_US.swap(now, Ordering::Relaxed);
    if previous != NEVER {
        READ_INTERVAL.record(now.saturating_sub(previous));
    }
    let reported = LAST_REPORT_US.load(Ordering::Relaxed);
    if reported != NEVER {
        let age = now.saturating_sub(reported);
        if age <= MOVING_WINDOW_US {
            VALUE_AGE.record(age);
        }
    }
    raw_axes()[0].unwrap_or(0)
}

/// Called on the game's thread each frame, with whether the player is driving.
pub fn set_driving(driving: bool) {
    DRIVING.store(driving, Ordering::Relaxed);
    HEARTBEAT_US.store(micros_since_start(Instant::now()), Ordering::Relaxed);
}

/// The latest raw steering, throttle and brake readings, where there are any.
pub fn raw_axes() -> [Option<i32>; 3] {
    [0, 1, 2].map(|axis| {
        let raw = AXES[axis].load(Ordering::Relaxed);
        (raw != NO_READING).then_some(raw)
    })
}

/// Forgets old readings when the wheel leaves, so they can't apply after it returns.
pub fn clear_axes() {
    for axis in &AXES {
        axis.store(NO_READING, Ordering::Relaxed);
    }
    CONTROLS.store(0, Ordering::Relaxed);
    PRESSED_FOR_POLL.store(0, Ordering::Relaxed);
    PRESSED_FOR_SCREEN.store(0, Ordering::Relaxed);
}

/// Whether the wheel may control the game at this moment.
pub fn wheel_may_apply() -> bool {
    let now = micros_since_start(Instant::now());
    drive::may_apply(
        WHEEL_CONNECTED.load(Ordering::Relaxed),
        DRIVING.load(Ordering::Relaxed),
        PAUSED.load(Ordering::Relaxed),
        now.saturating_sub(HEARTBEAT_US.load(Ordering::Relaxed)),
    )
}

/// Called each time the game polls for gamepad state.
pub fn record_poll() {
    let now = micros_since_start(Instant::now());
    let previous = LAST_POLL_US.swap(now, Ordering::Relaxed);
    if previous != NEVER {
        POLL_INTERVAL.record(now.saturating_sub(previous));
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

fn curve_bucket(thumb: i32) -> usize {
    let offset = (thumb + 32768).clamp(0, 65535) as usize;
    offset * CURVE_BUCKETS / 65536
}

/// Called when the game reports its steering action: what the stick value became, and how long
/// after the stick last changed.
pub fn record_steering_action(value: f32) {
    let thumb = LAST_THUMB.load(Ordering::Relaxed);
    CURVE[curve_bucket(thumb)].store(value.to_bits(), Ordering::Relaxed);
    let changed = THUMB_CHANGED_US.load(Ordering::Relaxed);
    if changed != NEVER && THUMB_CHANGE_SEEN_US.swap(changed, Ordering::Relaxed) != changed {
        let now = micros_since_start(Instant::now());
        ACTION_DELAY.record(now.saturating_sub(changed));
    }
}

fn format_curve(values: [Option<f32>; CURVE_BUCKETS]) -> String {
    let mut line = String::from("steering curve (stick range centre -> game action):");
    for (bucket, value) in values.iter().enumerate() {
        if let Some(value) = value {
            let centre = (bucket as f32 + 0.5) / CURVE_BUCKETS as f32 * 2.0 - 1.0;
            line.push_str(&format!(" {centre:.2}->{value:.3}"));
        }
    }
    line
}

/// The stick-to-action table, for the log.
pub fn curve_line() -> String {
    let mut values = [None; CURVE_BUCKETS];
    for (slot, entry) in values.iter_mut().zip(&CURVE) {
        let bits = entry.load(Ordering::Relaxed);
        if bits != NO_VALUE {
            *slot = Some(f32::from_bits(bits));
        }
    }
    format_curve(values)
}

/// Called at each game poll. The gamepad buttons the wheel is pressing now. Atomics only.
pub fn pad_buttons(may_apply: bool) -> u16 {
    let held = CONTROLS.load(Ordering::Relaxed);
    let pressed = PRESSED_FOR_POLL.swap(0, Ordering::Relaxed);
    let waiting = WAITING.load(Ordering::Relaxed);
    let (counted, waiting) = drive::gate_controls(held, pressed, waiting, may_apply);
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
    HOVER_HELD.store(flags & SCRIPT_HOVER != 0, Ordering::Relaxed);
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

/// Whether boost should push the car now. Touching the brake ends it.
pub fn boost_wanted() -> bool {
    BOOST_HELD.load(Ordering::Relaxed) && BRAKE_CLEAR.load(Ordering::Relaxed)
}

pub fn hover_wanted() -> bool {
    HOVER_HELD.load(Ordering::Relaxed)
}

pub fn set_pedals(throttle: u8, brake: u8) {
    PEDALS[0].store(u32::from(throttle), Ordering::Relaxed);
    PEDALS[1].store(u32::from(brake), Ordering::Relaxed);
}

/// Throttle, then brake, from 0 to 1.
pub fn pedal(index: usize) -> f32 {
    PEDALS[index].load(Ordering::Relaxed) as f32 / 255.0
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

/// Called on the game's thread for each key press. Returns the total so far.
pub fn record_key() -> u32 {
    KEYS.fetch_add(1, Ordering::Relaxed);
    TOTAL_KEYS.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
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
    let keys = KEYS.swap(0, Ordering::Relaxed);
    let decode = DECODE.drain();
    let interval = READ_INTERVAL.drain();
    let age = VALUE_AGE.drain();
    let poll = POLL_INTERVAL.drain();
    let poll_age = POLL_AGE.drain();
    let applied = APPLIED.swap(0, Ordering::Relaxed);
    let action_delay = ACTION_DELAY.drain();
    if reports == 0 && keys == 0 && interval.count == 0 && poll.count == 0 {
        return None;
    }
    Some(format!(
        "stats paused={} driving={} connected={} reports={reports} keys={keys} applied={applied} fired={} {} {} {} {} {} {}",
        PAUSED.load(Ordering::Relaxed),
        DRIVING.load(Ordering::Relaxed),
        WHEEL_CONNECTED.load(Ordering::Relaxed),
        settings::fired_names(FIRED.swap(0, Ordering::Relaxed)),
        format_summary("decode", decode),
        format_summary("read_interval", interval),
        format_summary("value_age", age),
        format_summary("poll_interval", poll),
        format_summary("poll_age", poll_age),
        format_summary("action_delay", action_delay),
    ))
}

#[cfg(test)]
mod tests {
    use super::{CURVE_BUCKETS, curve_bucket, format_curve, format_summary};
    use crate::latency::Summary;

    #[test]
    fn stick_values_land_in_the_table() {
        assert_eq!(curve_bucket(-32768), 0);
        assert_eq!(curve_bucket(0), CURVE_BUCKETS / 2);
        assert_eq!(curve_bucket(32767), CURVE_BUCKETS - 1);
    }

    #[test]
    fn the_table_lists_only_observed_ranges() {
        let mut values = [None; CURVE_BUCKETS];
        values[CURVE_BUCKETS / 2] = Some(0.25);
        assert_eq!(
            format_curve(values),
            "steering curve (stick range centre -> game action): 0.00->0.250"
        );
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

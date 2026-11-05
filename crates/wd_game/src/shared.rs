//! State shared between the wheel reader, the game's thread and the logger. Everything on
//! the wheel-to-game path is a plain atomic: no locks and no allocation.

use crate::drive;
use crate::latency::{Histogram, Summary};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;
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

static DECODE: Histogram = Histogram::new();
static READ_INTERVAL: Histogram = Histogram::new();
static VALUE_AGE: Histogram = Histogram::new();
static POLL_INTERVAL: Histogram = Histogram::new();
static POLL_AGE: Histogram = Histogram::new();
static ACTION_DELAY: Histogram = Histogram::new();

/// Number of stick ranges in the stick-to-action table.
const CURVE_BUCKETS: usize = 21;
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
pub fn record_report(axes: [Option<i64>; 3], received: Instant) {
    let now = Instant::now();
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
        "stats paused={} driving={} connected={} reports={reports} keys={keys} applied={applied} {} {} {} {} {} {}",
        PAUSED.load(Ordering::Relaxed),
        DRIVING.load(Ordering::Relaxed),
        WHEEL_CONNECTED.load(Ordering::Relaxed),
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

//! State shared between the wheel reader, the game's thread and the logger. Everything on
//! the wheel-to-game path is a plain atomic: no locks and no allocation.

use crate::latency::{Histogram, Summary};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;

/// A read this soon after a report counts as "wheel moving" for value age.
const MOVING_WINDOW_US: u64 = 50_000;
/// Marks that no report has arrived yet.
const NEVER: u64 = u64::MAX;

static START: OnceLock<Instant> = OnceLock::new();

pub static PAUSED: AtomicBool = AtomicBool::new(false);
pub static STEERING: AtomicI32 = AtomicI32::new(0);
static LAST_REPORT_US: AtomicU64 = AtomicU64::new(NEVER);
static LAST_READ_US: AtomicU64 = AtomicU64::new(NEVER);
static REPORTS: AtomicU32 = AtomicU32::new(0);
static KEYS: AtomicU32 = AtomicU32::new(0);
static TOTAL_KEYS: AtomicU32 = AtomicU32::new(0);

static DECODE: Histogram = Histogram::new();
static READ_INTERVAL: Histogram = Histogram::new();
static VALUE_AGE: Histogram = Histogram::new();

fn micros_since_start(instant: Instant) -> u64 {
    let start = *START.get_or_init(Instant::now);
    instant.saturating_duration_since(start).as_micros() as u64
}

/// Called by the wheel reader for each wheel report.
pub fn record_report(steering: Option<i64>, received: Instant) {
    let now = Instant::now();
    if let Some(value) = steering {
        STEERING.store(value as i32, Ordering::Relaxed);
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
    STEERING.load(Ordering::Relaxed)
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
    if reports == 0 && keys == 0 && interval.count == 0 {
        return None;
    }
    Some(format!(
        "stats paused={} reports={reports} keys={keys} {} {} {}",
        PAUSED.load(Ordering::Relaxed),
        format_summary("decode", decode),
        format_summary("read_interval", interval),
        format_summary("value_age", age),
    ))
}

#[cfg(test)]
mod tests {
    use super::format_summary;
    use crate::latency::Summary;

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

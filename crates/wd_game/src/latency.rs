//! Lock-free timing counters. Recording is one or two atomic operations, so measuring
//! does not add meaningful delay to the path being measured.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Bucket `n` holds values below `2^n` microseconds; the last also holds everything above.
const BUCKETS: usize = 21;

pub struct Histogram {
    counts: [AtomicU32; BUCKETS],
    max: AtomicU64,
}

/// What a histogram held over one interval. Percentiles are upper bounds of their bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub count: u32,
    pub p50_us: u64,
    pub p99_us: u64,
    pub max_us: u64,
}

fn bucket_for(micros: u64) -> usize {
    let bits = (u64::BITS - micros.leading_zeros()) as usize;
    bits.min(BUCKETS - 1)
}

fn upper_bound(bucket: usize) -> u64 {
    1u64 << bucket
}

fn percentile(counts: &[u32; BUCKETS], total: u32, percent: u64) -> u64 {
    let needed = (u64::from(total) * percent).div_ceil(100);
    let mut seen = 0u64;
    for (bucket, &count) in counts.iter().enumerate() {
        seen += u64::from(count);
        if seen >= needed && count > 0 {
            return upper_bound(bucket);
        }
    }
    0
}

impl Histogram {
    pub const fn new() -> Self {
        Self {
            counts: [const { AtomicU32::new(0) }; BUCKETS],
            max: AtomicU64::new(0),
        }
    }

    pub fn record(&self, micros: u64) {
        self.counts[bucket_for(micros)].fetch_add(1, Ordering::Relaxed);
        self.max.fetch_max(micros, Ordering::Relaxed);
    }

    /// Returns the interval's summary and starts a new interval.
    pub fn drain(&self) -> Summary {
        let mut counts = [0u32; BUCKETS];
        for (slot, counter) in counts.iter_mut().zip(&self.counts) {
            *slot = counter.swap(0, Ordering::Relaxed);
        }
        let count = counts.iter().sum();
        Summary {
            count,
            p50_us: percentile(&counts, count, 50),
            p99_us: percentile(&counts, count, 99),
            max_us: self.max.swap(0, Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BUCKETS, Histogram, Summary, bucket_for};

    #[test]
    fn buckets_are_powers_of_two() {
        assert_eq!(bucket_for(0), 0);
        assert_eq!(bucket_for(1), 1);
        assert_eq!(bucket_for(3), 2);
        assert_eq!(bucket_for(4), 3);
    }

    #[test]
    fn very_large_values_stay_in_the_last_bucket() {
        assert_eq!(bucket_for(u64::MAX), BUCKETS - 1);
    }

    #[test]
    fn an_empty_interval_is_all_zero() {
        let empty = Summary {
            count: 0,
            p50_us: 0,
            p99_us: 0,
            max_us: 0,
        };
        assert_eq!(Histogram::new().drain(), empty);
    }

    #[test]
    fn percentiles_reflect_the_distribution() {
        let histogram = Histogram::new();
        for _ in 0..99 {
            histogram.record(10);
        }
        histogram.record(5000);
        let summary = histogram.drain();
        assert_eq!(summary.count, 100);
        assert_eq!(summary.p50_us, 16);
        assert_eq!(summary.p99_us, 16);
        assert_eq!(summary.max_us, 5000);
    }

    #[test]
    fn draining_starts_a_new_interval() {
        let histogram = Histogram::new();
        histogram.record(7);
        histogram.drain();
        assert_eq!(histogram.drain().count, 0);
    }
}

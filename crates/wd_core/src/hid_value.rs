//! Interpretation of raw HID usage values.
//!
//! `HidP_GetUsageValue` returns an unsigned value without sign extension, so a
//! field with a negative logical minimum has to be extended here.

/// Converts a raw field to its logical value.
///
/// Returns `None` when `bit_size` is outside 1..=32.
pub fn decode_logical(raw: u32, bit_size: u16, logical_min: i32) -> Option<i64> {
    if !(1..=32).contains(&bit_size) {
        return None;
    }
    let bits = u32::from(bit_size);
    let mask = if bits == 32 {
        u32::MAX
    } else {
        (1u32 << bits) - 1
    };
    let value = u64::from(raw & mask);
    if logical_min < 0 && value & (1u64 << (bits - 1)) != 0 {
        Some(value as i64 - (1i64 << bits))
    } else {
        Some(value as i64)
    }
}

/// Position of `value` within its logical range, clamped to 0.0..=1.0.
///
/// Returns `None` for an empty or reversed range, which some devices report.
pub fn logical_fraction(value: i64, logical_min: i64, logical_max: i64) -> Option<f64> {
    if logical_max <= logical_min {
        return None;
    }
    let span = (logical_max - logical_min) as f64;
    Some((((value - logical_min) as f64) / span).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned_field_is_not_sign_extended() {
        assert_eq!(decode_logical(0xFFFF, 16, 0), Some(65535));
    }

    #[test]
    fn signed_field_is_sign_extended() {
        assert_eq!(decode_logical(0xFFFF, 16, -32768), Some(-1));
        assert_eq!(decode_logical(0x8000, 16, -32768), Some(-32768));
        assert_eq!(decode_logical(0x7FFF, 16, -32768), Some(32767));
    }

    #[test]
    fn bits_above_the_field_are_ignored() {
        assert_eq!(decode_logical(0xABCD_00FF, 8, 0), Some(255));
    }

    #[test]
    fn full_width_fields_do_not_overflow() {
        assert_eq!(decode_logical(u32::MAX, 32, 0), Some(4_294_967_295));
        assert_eq!(decode_logical(u32::MAX, 32, -1), Some(-1));
    }

    #[test]
    fn single_bit_field_decodes() {
        assert_eq!(decode_logical(1, 1, 0), Some(1));
        assert_eq!(decode_logical(1, 1, -1), Some(-1));
    }

    #[test]
    fn invalid_bit_sizes_are_rejected() {
        assert_eq!(decode_logical(1, 0, 0), None);
        assert_eq!(decode_logical(1, 33, 0), None);
    }

    #[test]
    fn fraction_covers_range_and_clamps() {
        assert_eq!(logical_fraction(0, 0, 100), Some(0.0));
        assert_eq!(logical_fraction(50, 0, 100), Some(0.5));
        assert_eq!(logical_fraction(100, 0, 100), Some(1.0));
        assert_eq!(logical_fraction(-5, 0, 100), Some(0.0));
        assert_eq!(logical_fraction(105, 0, 100), Some(1.0));
    }

    #[test]
    fn fraction_rejects_empty_or_reversed_range() {
        assert_eq!(logical_fraction(1, 5, 5), None);
        assert_eq!(logical_fraction(1, 5, 0), None);
    }
}

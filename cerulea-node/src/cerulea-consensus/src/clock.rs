//! The wall clock every Cerulea engine reads.
//!
//! The Authoring Engine names the slot from this clock. Import bounds the
//! moment a seal claims by it. The timestamp of each authored block comes from
//! it. One function serves all of them, so one setting moves the whole clock
//! of one node.
//!
//! The setting is `CERULEA_CLOCK_OFFSET_MS`, a signed count of milliseconds
//! added to the operating system clock. A fault test uses it to run one
//! validator ahead of or behind the others. The node honours the setting only
//! in a build carrying the `clock-offset` feature: the test builds and the
//! multi-host exercise image. A release build ignores the setting and says so
//! in its log.

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// The setting that moves the clock of one node, in signed milliseconds.
pub const CLOCK_OFFSET_ENV: &str = "CERULEA_CLOCK_OFFSET_MS";

/// Report whether this build honours the clock offset setting.
pub const OFFSET_HONOURED: bool = cfg!(feature = "clock-offset");

/// Read the offset from its text form.
///
/// An absent or unreadable value reads as no offset.
pub fn parse_offset(value: Option<&str>) -> i64 {
    value
        .and_then(|text| text.trim().parse::<i64>().ok())
        .unwrap_or(0)
}

/// The offset this build applies, given the setting it found.
///
/// A build without the `clock-offset` feature applies none.
pub fn applied_offset(setting: Option<&str>, honoured: bool) -> i64 {
    if honoured {
        parse_offset(setting)
    } else {
        0
    }
}

/// The offset this process runs under, read once at the first clock reading.
pub fn offset_ms() -> i64 {
    static OFFSET: OnceLock<i64> = OnceLock::new();
    *OFFSET.get_or_init(|| {
        let setting = std::env::var(CLOCK_OFFSET_ENV).ok();
        let offset = applied_offset(setting.as_deref(), OFFSET_HONOURED);
        if offset != 0 {
            log::warn!(
                "Cerulea clock: this node runs {} milliseconds {} the system clock through {}. Fault exercises alone use this setting.",
                offset.unsigned_abs(),
                if offset > 0 { "ahead of" } else { "behind" },
                CLOCK_OFFSET_ENV
            );
        } else if setting.is_some() && !OFFSET_HONOURED {
            log::warn!(
                "Cerulea clock: {} is set and this release build ignores it. The node reads the system clock.",
                CLOCK_OFFSET_ENV
            );
        }
        offset
    })
}

/// Apply an offset to a system clock reading, never below zero.
pub fn shifted(system_ms: u64, offset_ms: i64) -> u64 {
    if offset_ms >= 0 {
        system_ms.saturating_add(offset_ms.unsigned_abs())
    } else {
        system_ms.saturating_sub(offset_ms.unsigned_abs())
    }
}

/// The operating system clock in milliseconds since its epoch.
///
/// A clock set before its epoch reads as zero and the log says so.
pub fn system_ms() -> u64 {
    since_epoch(SystemTime::now())
}

/// Milliseconds from the epoch to one moment, zero for a moment before it.
pub fn since_epoch(moment: SystemTime) -> u64 {
    match moment.duration_since(UNIX_EPOCH) {
        Ok(elapsed) => elapsed.as_millis() as u64,
        Err(reason) => {
            log::error!(
                "Cerulea clock: the system clock sits before its epoch ({:?}). Fix: set the system clock forward.",
                reason
            );
            0
        }
    }
}

/// The moment now on the Cerulea clock, in milliseconds since the epoch.
pub fn now_ms() -> u64 {
    shifted(system_ms(), offset_ms())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_offset_reads_from_its_text() {
        assert_eq!(parse_offset(None), 0);
        assert_eq!(parse_offset(Some("30000")), 30_000);
        assert_eq!(parse_offset(Some(" -1500 ")), -1_500);
        assert_eq!(parse_offset(Some("soon")), 0);
    }

    #[test]
    fn a_release_build_applies_no_offset() {
        assert_eq!(applied_offset(Some("30000"), true), 30_000);
        assert_eq!(applied_offset(Some("30000"), false), 0);
        assert_eq!(applied_offset(None, true), 0);
    }

    #[test]
    fn the_offset_moves_the_clock_both_ways() {
        assert_eq!(shifted(1_000, 30_000), 31_000);
        assert_eq!(shifted(1_000, -400), 600);
        assert_eq!(shifted(100, -400), 0);
        assert_eq!(shifted(u64::MAX, 1), u64::MAX);
    }

    #[test]
    fn a_moment_before_the_epoch_reads_zero() {
        let before = UNIX_EPOCH - std::time::Duration::from_secs(5);
        assert_eq!(since_epoch(before), 0);
        let after = UNIX_EPOCH + std::time::Duration::from_millis(1_234);
        assert_eq!(since_epoch(after), 1_234);
    }

    #[test]
    fn the_clock_reads_close_to_the_system_clock() {
        let system = system_ms();
        let cerulea = now_ms();
        let offset = offset_ms();
        let expected = shifted(system, offset);
        assert!(cerulea.abs_diff(expected) < 1_000);
    }
}

//! Standing decay during inactivity.
//!
//! Standing falls slowly during inactivity. A validator holding a seat without
//! answering challenges or signing blocks drifts downward and exits
//! automatically once the standing floor is crossed.
//!
//! Decay applies per idle epoch. An epoch counts as idle where the validator
//! met none of the service conditions. The fall is a fixed share of the
//! remaining standing, so the drop is steep at first and gentle near zero. That
//! shape costs an absent validator its seat while leaving a path back.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;
use sp_runtime::Perbill;

/// The share of standing one idle epoch removes, in percent.
pub const DEFAULT_DECAY_PERCENT: u32 = 5;

/// The epochs of inactivity the engine allows before decay starts.
///
/// A short outage costs nothing. A sustained absence costs the seat.
pub const DEFAULT_GRACE_EPOCHS: u32 = 2;

/// The state decay reads.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct DecaySettings {
    /// The share of standing one idle epoch removes, in percent.
    pub decay_percent: u32,
    /// The idle epochs the engine allows before decay starts.
    pub grace_epochs: u32,
}

impl DecaySettings {
    /// The profile a deployment starts from.
    pub const fn starting_profile() -> Self {
        DecaySettings {
            decay_percent: DEFAULT_DECAY_PERCENT,
            grace_epochs: DEFAULT_GRACE_EPOCHS,
        }
    }
}

/// The idle epochs one validator has accumulated.
pub fn idle_epochs(current_epoch: u32, last_served_epoch: u32) -> u32 {
    current_epoch.saturating_sub(last_served_epoch)
}

/// The standing share left after one idle epoch.
///
/// The fall is a share of the remaining standing. A positive standing always
/// loses at least one part, so a long absence reaches zero rather than resting
/// just above it.
pub fn decay_once(standing: Perbill, decay_percent: u32) -> Perbill {
    if standing.is_zero() || decay_percent == 0 {
        return standing;
    }
    let parts = standing.deconstruct();
    let share = Perbill::from_percent(decay_percent.min(100)).mul_floor(parts);
    let removed = share.max(1);
    Perbill::from_parts(parts.saturating_sub(removed))
}

/// The standing share left after a run of idle epochs.
///
/// The grace span costs nothing. Every idle epoch past the grace span removes a
/// share of the remaining standing.
pub fn decayed(standing: Perbill, idle: u32, settings: DecaySettings) -> Perbill {
    if idle <= settings.grace_epochs {
        return standing;
    }
    let mut held = standing;
    let steps = idle.saturating_sub(settings.grace_epochs);
    for _ in 0..steps {
        held = decay_once(held, settings.decay_percent);
        if held.is_zero() {
            break;
        }
    }
    held
}

/// The idle epochs a validator survives before crossing a floor.
///
/// The Admission Engine reads this when it explains an approaching exit.
pub fn epochs_until_floor(standing: Perbill, floor: Perbill, settings: DecaySettings) -> u32 {
    if standing <= floor {
        return 0;
    }
    if settings.decay_percent == 0 {
        return u32::MAX;
    }
    let mut held = standing;
    let mut epochs = settings.grace_epochs;
    // The loop ends because every step removes a share of a positive value and
    // the value reaches the floor or reaches zero.
    while held > floor && epochs < u32::MAX {
        let next = decay_once(held, settings.decay_percent);
        if next == held {
            return u32::MAX;
        }
        held = next;
        epochs = epochs.saturating_add(1);
    }
    epochs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> DecaySettings {
        DecaySettings::starting_profile()
    }

    #[test]
    fn a_short_outage_costs_nothing() {
        let standing = Perbill::from_percent(80);
        assert_eq!(decayed(standing, 0, settings()), standing);
        assert_eq!(decayed(standing, 1, settings()), standing);
        assert_eq!(decayed(standing, 2, settings()), standing);
    }

    #[test]
    fn a_sustained_absence_lowers_standing() {
        let standing = Perbill::from_percent(80);
        let after = decayed(standing, 10, settings());
        assert!(after < standing);
    }

    #[test]
    fn standing_falls_further_with_every_idle_epoch() {
        let standing = Perbill::from_percent(90);
        let mut previous = standing;
        for idle in 3..20u32 {
            let held = decayed(standing, idle, settings());
            assert!(held < previous, "epoch {} falls further", idle);
            previous = held;
        }
    }

    #[test]
    fn an_active_validator_holds_its_standing() {
        let standing = Perbill::from_percent(75);
        assert_eq!(idle_epochs(40, 40), 0);
        assert_eq!(decayed(standing, idle_epochs(40, 40), settings()), standing);
    }

    #[test]
    fn standing_never_falls_below_zero() {
        let standing = Perbill::from_percent(10);
        let after = decayed(standing, 5_000, settings());
        assert_eq!(after, Perbill::zero());
    }

    #[test]
    fn a_decay_share_of_zero_holds_standing_still() {
        let standing = Perbill::from_percent(60);
        let held = DecaySettings {
            decay_percent: 0,
            grace_epochs: 0,
        };
        assert_eq!(decayed(standing, 100, held), standing);
    }

    #[test]
    fn the_engine_reports_the_epochs_until_a_floor() {
        let standing = Perbill::from_percent(90);
        let floor = Perbill::from_percent(50);
        let epochs = epochs_until_floor(standing, floor, settings());
        assert!(epochs > settings().grace_epochs);
        assert!(decayed(standing, epochs, settings()) <= floor);
        assert!(decayed(standing, epochs - 1, settings()) > floor);
    }

    #[test]
    fn a_validator_already_below_the_floor_reports_zero_epochs() {
        assert_eq!(
            epochs_until_floor(
                Perbill::from_percent(10),
                Perbill::from_percent(50),
                settings()
            ),
            0
        );
    }

    #[test]
    fn a_drifting_validator_crosses_the_floor_and_exits() {
        // A validator at the whole share that stops serving crosses a floor of
        // one third inside a working number of epochs.
        let floor = Perbill::from_percent(33);
        let epochs = epochs_until_floor(Perbill::one(), floor, settings());
        assert!(epochs < 60, "the drift ends the seat inside 60 epochs");
        assert!(decayed(Perbill::one(), epochs, settings()) <= floor);
    }

    #[test]
    fn idle_epochs_count_from_the_last_served_epoch() {
        assert_eq!(idle_epochs(100, 90), 10);
        assert_eq!(idle_epochs(90, 100), 0);
    }
}

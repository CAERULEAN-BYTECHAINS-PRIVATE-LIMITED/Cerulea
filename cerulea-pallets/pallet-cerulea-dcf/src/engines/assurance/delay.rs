//! The delay window a constitutional change waits.
//!
//! A constitutional change carries the authority to amend policy or to replace
//! the runtime. Such a change commits at nine tenths of aggregated weight and
//! then waits. The wait gives every participant time to read the change and to
//! respond before the change binds.
//!
//! The window is a count of blocks. The chain records the block at which the
//! change committed and the block at which the change takes effect. A release
//! before the second block fails and names the blocks remaining. The chain
//! releases the change at the first block at or after the second block.

use super::classes::{ClassTable, TransactionClass};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;

/// A reason a delayed change fails to release.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum DelayError {
    /// The window still holds. The count carries the blocks remaining.
    WindowOpen(u32),
    /// The change already released, so a second release finds nothing.
    AlreadyReleased,
}

impl DelayError {
    /// The sentence a person reads, naming the fix.
    pub fn plain_language(&self) -> &'static str {
        match self {
            DelayError::WindowOpen(_) => {
                "The delay window still holds. Wait for the remaining blocks, then release the change."
            }
            DelayError::AlreadyReleased => {
                "The change already took effect. Schedule a new change to alter the policy again."
            }
        }
    }

    /// The blocks remaining, where the window still holds.
    pub const fn blocks_remaining(&self) -> u32 {
        match self {
            DelayError::WindowOpen(blocks) => *blocks,
            DelayError::AlreadyReleased => 0,
        }
    }
}

/// The window one change waits.
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
pub struct DelayWindow {
    /// The block at which the change committed.
    pub committed_at: u32,
    /// The first block at which the change takes effect.
    pub effective_at: u32,
}

impl DelayWindow {
    /// Open a window of the given length at the given block.
    pub const fn open(committed_at: u32, delay_blocks: u32) -> Self {
        DelayWindow {
            committed_at,
            effective_at: committed_at.saturating_add(delay_blocks),
        }
    }

    /// The length of the window, in blocks.
    pub const fn length(&self) -> u32 {
        self.effective_at.saturating_sub(self.committed_at)
    }

    /// Report whether the window has run out at the block given.
    pub const fn is_ready(&self, now: u32) -> bool {
        now >= self.effective_at
    }

    /// The blocks remaining at the block given.
    pub const fn blocks_remaining(&self, now: u32) -> u32 {
        self.effective_at.saturating_sub(now)
    }
}

/// One change waiting out its window.
///
/// The payload is whatever the deployment schedules. The Amendment Engine
/// schedules a policy record. A runtime replacement schedules the code hash.
#[derive(
    Encode,
    Decode,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct ScheduledChange<Payload> {
    /// The class the change carries.
    pub class: TransactionClass,
    /// The window the change waits.
    pub window: DelayWindow,
    /// The change itself.
    pub payload: Payload,
    /// The state of the change.
    pub released: bool,
}

impl<Payload> ScheduledChange<Payload> {
    /// Schedule a change under the window its class carries.
    ///
    /// A class carrying no window takes effect at the block of commitment.
    pub fn schedule(
        table: &ClassTable,
        class: TransactionClass,
        payload: Payload,
        committed_at: u32,
    ) -> Self {
        ScheduledChange {
            class,
            window: DelayWindow::open(committed_at, table.delay_blocks(class)),
            payload,
            released: false,
        }
    }

    /// Report whether the change is ready to release at the block given.
    pub fn is_ready(&self, now: u32) -> bool {
        !self.released && self.window.is_ready(now)
    }

    /// Release the change at the block given.
    ///
    /// A release inside the window fails and names the blocks remaining. A
    /// second release fails as well.
    pub fn release(&mut self, now: u32) -> Result<&Payload, DelayError> {
        if self.released {
            return Err(DelayError::AlreadyReleased);
        }
        if !self.window.is_ready(now) {
            return Err(DelayError::WindowOpen(self.window.blocks_remaining(now)));
        }
        self.released = true;
        Ok(&self.payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> ClassTable {
        ClassTable::starting_profile()
    }

    #[test]
    fn a_routine_change_takes_effect_at_the_block_of_commitment() {
        let mut change = ScheduledChange::schedule(&table(), TransactionClass::Routine, 7u32, 10);
        assert_eq!(change.window.length(), 0);
        assert!(change.is_ready(10));
        assert_eq!(change.release(10), Ok(&7u32));
    }

    /// Design assertion 6.
    ///
    /// A constitutional change waits the full delay window. The change fails to
    /// release at every block inside the window, and the failure names the
    /// blocks remaining. The change releases at the first block at or after the
    /// effective block.
    #[test]
    fn a_constitutional_change_waits_the_full_window() {
        let table = table();
        let committed_at = 10u32;
        let mut change = ScheduledChange::schedule(
            &table,
            TransactionClass::Constitutional,
            42u32,
            committed_at,
        );

        let window = table.delay_blocks(TransactionClass::Constitutional);
        assert_eq!(change.window.length(), window);
        assert_eq!(change.window.effective_at, committed_at + window);

        for now in committed_at..(committed_at + window) {
            assert!(!change.is_ready(now));
            let outcome = change.release(now);
            let remaining = committed_at + window - now;
            assert_eq!(outcome, Err(DelayError::WindowOpen(remaining)));
        }

        assert!(change.is_ready(committed_at + window));
        assert_eq!(change.release(committed_at + window), Ok(&42u32));
    }

    #[test]
    fn a_released_change_releases_once_only() {
        let mut change =
            ScheduledChange::schedule(&table(), TransactionClass::Constitutional, 1u32, 0);
        let window = table().delay_blocks(TransactionClass::Constitutional);
        assert!(change.release(window).is_ok());
        assert_eq!(change.release(window), Err(DelayError::AlreadyReleased));
        assert!(!change.is_ready(window));
    }

    #[test]
    fn the_window_reports_the_blocks_remaining() {
        let window = DelayWindow::open(100, 30);
        assert_eq!(window.blocks_remaining(100), 30);
        assert_eq!(window.blocks_remaining(115), 15);
        assert_eq!(window.blocks_remaining(130), 0);
        assert_eq!(window.blocks_remaining(200), 0);
    }

    #[test]
    fn every_delay_failure_names_a_fix() {
        assert!(DelayError::WindowOpen(5).plain_language().len() > 20);
        assert!(DelayError::AlreadyReleased.plain_language().len() > 20);
        assert_eq!(DelayError::WindowOpen(5).blocks_remaining(), 5);
    }
}

//! The contract every platform module writes the audit log through.
//!
//! The audit module holds the log. Every other platform module records its
//! work by calling [`AuditSink`]. No module reaches into audit storage
//! directly. The trait lives beside the module so a dependant reads one
//! definition.
//!
//! A record carries the actor, the class of work, the module that acted, a
//! plain reason and the payload. The log stamps the sequence number, the
//! block and the time.

use alloc::{string::String, vec::Vec};
use cerulea_blueprint::actions::TransactionClass;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use pallet_cerulea_schema::primitives::DeploymentId;
use scale_info::TypeInfo;
use sp_runtime::DispatchError;

/// The largest reason sentence the log stores, in bytes.
pub const MAX_REASON_BYTES: u32 = 256;

/// The largest summary sentence an event definition stores, in bytes.
pub const MAX_SUMMARY_BYTES: u32 = 256;

/// A bounded plain sentence.
pub type Sentence = BoundedVec<u8, ConstU32<MAX_REASON_BYTES>>;

/// The class of work one entry records.
///
/// The four classes carry the agreement thresholds of the Blockchain.
/// Routine work needs 51 percent. Financial work needs 67 percent.
/// Structural work needs 80 percent. Constitutional work needs 90 percent
/// and a delay.
///
/// The Blueprint carries the same four classes. This form adds the storage
/// bounds the Blockchain needs, so a class sits inside a bounded storage
/// item and inside a call argument.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum WorkClass {
    /// Ordinary work. Scans, readings and log entries.
    #[default]
    Routine,
    /// Work moving value. Transfers, settlements and payments.
    Financial,
    /// Work changing the shape of the deployment. Schema, permission and
    /// validator set changes.
    Structural,
    /// Work changing the rules of the deployment. Policy amendment and
    /// upgrade authority.
    Constitutional,
}

impl WorkClass {
    /// The share of the electorate that agrees before the work lands.
    pub fn threshold_percent(&self) -> u8 {
        match self {
            WorkClass::Routine => 51,
            WorkClass::Financial => 67,
            WorkClass::Structural => 80,
            WorkClass::Constitutional => 90,
        }
    }

    /// Report whether a mandatory delay applies before the work lands.
    pub fn holds_delay(&self) -> bool {
        matches!(self, WorkClass::Constitutional)
    }

    /// A word naming the class for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            WorkClass::Routine => "routine",
            WorkClass::Financial => "financial",
            WorkClass::Structural => "structural",
            WorkClass::Constitutional => "constitutional",
        }
    }
}

impl From<TransactionClass> for WorkClass {
    fn from(class: TransactionClass) -> Self {
        match class {
            TransactionClass::Routine => WorkClass::Routine,
            TransactionClass::Financial => WorkClass::Financial,
            TransactionClass::Structural => WorkClass::Structural,
            TransactionClass::Constitutional => WorkClass::Constitutional,
        }
    }
}

impl From<WorkClass> for TransactionClass {
    fn from(class: WorkClass) -> Self {
        match class {
            WorkClass::Routine => TransactionClass::Routine,
            WorkClass::Financial => TransactionClass::Financial,
            WorkClass::Structural => TransactionClass::Structural,
            WorkClass::Constitutional => TransactionClass::Constitutional,
        }
    }
}

/// One record a platform module asks the log to hold.
///
/// The module names itself, names the event and names the actor. The log
/// adds the sequence number, the block and the time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditRequest<'a, AccountId> {
    /// The deployment the work belongs to.
    pub deployment: DeploymentId,
    /// The module that performed the work.
    pub module: &'a str,
    /// The event the module records.
    pub event: &'a str,
    /// The class of the work.
    pub class: WorkClass,
    /// The account the Blockchain records as the actor.
    pub actor: &'a AccountId,
    /// One plain sentence stating why the work ran.
    pub reason: &'a str,
    /// The canonical encoding of the detail the event carries.
    pub payload: Vec<u8>,
    /// The entry this record corrects, when the record is a correction.
    pub corrects: Option<u64>,
}

impl<'a, AccountId> AuditRequest<'a, AccountId> {
    /// Start a record of routine work.
    pub fn new(
        deployment: DeploymentId,
        module: &'a str,
        event: &'a str,
        actor: &'a AccountId,
        reason: &'a str,
    ) -> Self {
        AuditRequest {
            deployment,
            module,
            event,
            class: WorkClass::Routine,
            actor,
            reason,
            payload: Vec::new(),
            corrects: None,
        }
    }

    /// Set the class of the work.
    pub fn classed(mut self, class: WorkClass) -> Self {
        self.class = class;
        self
    }

    /// Carry a payload with the record.
    pub fn carrying(mut self, payload: Vec<u8>) -> Self {
        self.payload = payload;
        self
    }

    /// Mark the record as the correction of an earlier entry.
    pub fn correcting(mut self, sequence: u64) -> Self {
        self.corrects = Some(sequence);
        self
    }
}

/// The log every platform module writes into.
///
/// The audit module implements this trait. A module records its work
/// through [`AuditSink::record`] and keeps the sequence number the log
/// answers with.
pub trait AuditSink {
    /// The account type the runtime carries.
    type AccountId;

    /// Append one record and answer with its sequence number.
    ///
    /// The call appends. Nothing in the log ever changes afterwards. A
    /// correction appends a further record naming the entry it corrects.
    fn record(request: AuditRequest<'_, Self::AccountId>) -> Result<u64, DispatchError>;
}

/// One entry of the log as a reader receives it.
///
/// Cerulea Explorer, Cerulea Dashboard and an external subscriber all read
/// this shape. The text fields arrive as plain text ready to show.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct LogRecord<AccountId, BlockNumber> {
    /// The position of the entry in the log of the deployment.
    pub sequence: u64,
    /// The event name.
    pub event: String,
    /// The module that recorded the work.
    pub module: String,
    /// The class of the work.
    pub class: WorkClass,
    /// The account the Blockchain records as the actor.
    pub actor: AccountId,
    /// The wall clock time of the entry, in milliseconds.
    pub at_millis: u64,
    /// The block the entry landed in.
    pub block: BlockNumber,
    /// One plain sentence stating why the work ran.
    pub reason: String,
    /// The canonical encoding of the detail the event carries.
    pub payload: Vec<u8>,
    /// The entry this record corrects.
    pub corrects: Option<u64>,
    /// The running digest of the log up to and including this entry.
    pub digest: [u8; 32],
}

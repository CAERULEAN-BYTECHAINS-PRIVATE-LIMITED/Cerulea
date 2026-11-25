//! The anchoring path.
//!
//! Anchoring writes a Cerulea state root into the Bitcoin network. The
//! record of that write is then proved back on this chain. Anchoring gives
//! a deployment an external witness of its own history, and the witness
//! rests on the work the far network spent.
//!
//! # The two halves
//!
//! 1. **The request.** The module reads the inclusion root of one
//!    collection and records an anchor holding that root. The event
//!    carries the script bytes the driver writes into a data carrying
//!    output. The off-chain driver builds the transaction, signs it with
//!    the key it holds outside this chain and broadcasts it.
//! 2. **The confirmation.** The driver submits the transaction, the
//!    merkle branch and the height. The module reads the accepted header
//!    of that height, counts the confirmations, folds the branch to the
//!    transaction merkle root and reads the data output. The root in the
//!    output equals the root of the request.
//!
//! The chain holds no key of the far network at any point. The driver
//! signs. The chain checks.

use crate::endpoint::{EndpointKey, VerifierParameters};
use crate::envelope::BridgeFault;
use crate::networks::bitcoin;
use crate::verify::{Branch, RawTransaction};
use crate::{AnchorNonce, Anchors, Config, Error, Event, Headers, Pallet, Tips};
use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_cerulea_audit::contract::WorkClass;
use pallet_cerulea_schema::primitives::{BoundedName, DeploymentId};
use pallet_cerulea_state::RecordStore;
use scale_info::TypeInfo;

/// Where one anchor stands.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum AnchorStatus {
    /// The anchor waits for the far network transaction.
    #[default]
    Requested,
    /// The far network holds the anchor.
    Confirmed,
}

impl AnchorStatus {
    /// A word naming the status for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            AnchorStatus::Requested => "requested",
            AnchorStatus::Confirmed => "confirmed",
        }
    }
}

/// One state root anchor.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct AnchorRecord<BlockNumber> {
    /// The collection the root covers.
    pub collection: BoundedName,
    /// The inclusion root of that collection.
    pub root: [u8; 32],
    /// Where the anchor stands.
    pub status: AnchorStatus,
    /// The block the request landed at.
    pub requested_at: BlockNumber,
    /// The far network height holding the anchor.
    pub height: u64,
    /// The identifier of the far network transaction.
    pub reference: [u8; 32],
    /// The confirmations counted above the anchor.
    pub confirmations: u32,
}

/// The evidence one anchor confirmation carries.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct AnchorEvidence {
    /// The far network height the transaction sits at.
    pub height: u64,
    /// The encoded transaction.
    pub transaction: RawTransaction,
    /// The position of the transaction in the block.
    pub position: u32,
    /// The sibling hash at each level, the leaf level first.
    pub branch: Branch,
    /// The position of the data carrying output in the transaction.
    pub output_index: u32,
}

/// The name the checkpoint anchor record carries.
///
/// A checkpoint anchor commits a Cerulea checkpoint rather than the inclusion
/// root of one collection, so the record carries this name in place of a
/// collection name.
pub const CHECKPOINT_RECORD_NAME: &[u8] = b"checkpoint";

/// Request one Cerulea checkpoint anchor.
///
/// A checkpoint is one committed block the chain names as settled. The
/// framework pallet anchors checkpoints in index order. This path writes the
/// thirty two byte commitment of one checkpoint into the far network, so the
/// Cerulea history gains a witness outside Cerulea.
///
/// The anchored set the framework pallet holds is the set this path commits.
/// The commitment folds the checkpoint index, the block number, the block hash
/// and the state root under a Cerulea separator, so any party recomputes the
/// value from chain state and compares.
pub fn request_checkpoint<T: Config>(
    who: &T::AccountId,
    deployment: DeploymentId,
    name: BoundedName,
    checkpoint: cerulea_primitives::checkpoint::Checkpoint,
) -> DispatchResult {
    let record = BoundedName::try_from(CHECKPOINT_RECORD_NAME.to_vec())
        .map_err(|_| Error::<T>::NameTooLong)?;
    request_value::<T>(who, deployment, name, record, checkpoint.commitment())
}

/// Request one state root anchor.
pub fn request<T: Config>(
    who: &T::AccountId,
    deployment: DeploymentId,
    name: BoundedName,
    collection: BoundedName,
) -> DispatchResult {
    let key = EndpointKey::new(deployment, name.clone());
    let record = crate::Endpoints::<T>::get(&key).ok_or(Error::<T>::EndpointUnknown)?;
    ensure!(record.live, Error::<T>::EndpointRetired);
    match &record.params {
        VerifierParameters::Bitcoin(_) => {}
        _ => return Err(Error::<T>::KindMismatch.into()),
    };

    let held = cerulea_blueprint::types::CollectionId::new(
        core::str::from_utf8(collection.as_slice()).unwrap_or_default(),
    );
    let root = T::Records::root(&deployment, &held).ok_or(Error::<T>::RootAbsent)?;
    request_value::<T>(who, deployment, name, collection, root)
}

/// Record one anchor request over a thirty two byte value.
///
/// Both the collection root path and the checkpoint path end here, so one
/// record shape and one event serve both.
pub fn request_value<T: Config>(
    who: &T::AccountId,
    deployment: DeploymentId,
    name: BoundedName,
    collection: BoundedName,
    root: [u8; 32],
) -> DispatchResult {
    let key = EndpointKey::new(deployment, name.clone());
    let record = crate::Endpoints::<T>::get(&key).ok_or(Error::<T>::EndpointUnknown)?;
    ensure!(record.live, Error::<T>::EndpointRetired);
    let params = match &record.params {
        VerifierParameters::Bitcoin(held) => held.clone(),
        _ => return Err(Error::<T>::KindMismatch.into()),
    };

    let position = AnchorNonce::<T>::get(&key);
    AnchorNonce::<T>::insert(&key, position.saturating_add(1));
    Anchors::<T>::insert(
        &key,
        position,
        AnchorRecord::<BlockNumberFor<T>> {
            collection,
            root,
            status: AnchorStatus::Requested,
            requested_at: frame_system::Pallet::<T>::block_number(),
            height: 0,
            reference: [0u8; 32],
            confirmations: 0,
        },
    );

    let script = bitcoin::write_anchor(params.anchor_marker.as_slice(), &root);
    Pallet::<T>::log(
        &deployment,
        "anchor.requested",
        WorkClass::Structural,
        who,
        "The deployment asked the driver to write a state root into the far network.",
        (position, root, script).encode(),
    )?;
    Pallet::<T>::deposit_event(Event::AnchorRequested {
        deployment,
        endpoint: name,
        anchor: position,
        root,
    });
    Ok(())
}

/// Confirm that the far network holds one state root anchor.
pub fn confirm<T: Config>(
    who: &T::AccountId,
    deployment: DeploymentId,
    name: BoundedName,
    position: u64,
    evidence: &AnchorEvidence,
) -> DispatchResult {
    let key = EndpointKey::new(deployment, name.clone());
    let record = crate::Endpoints::<T>::get(&key).ok_or(Error::<T>::EndpointUnknown)?;
    let params = match &record.params {
        VerifierParameters::Bitcoin(held) => held.clone(),
        _ => return Err(Error::<T>::KindMismatch.into()),
    };
    let anchor = Anchors::<T>::get(&key, position).ok_or(Error::<T>::AnchorUnknown)?;
    ensure!(
        anchor.status == AnchorStatus::Requested,
        Error::<T>::MessageSettled
    );

    let header = Headers::<T>::get(&key, evidence.height).ok_or(Error::<T>::ProofDeclined)?;
    let tip = Tips::<T>::get(&key);
    let depth = tip
        .saturating_sub(evidence.height)
        .saturating_add(1)
        .min(u32::MAX as u64) as u32;
    ensure!(
        depth >= params.confirmations.max(1),
        Error::<T>::ProofDeclined
    );

    let found = read_anchor_output(&params, evidence, &header.state_root)
        .map_err(|_| Error::<T>::ProofDeclined)?;
    ensure!(found.0 == anchor.root, Error::<T>::AnchorMismatch);

    Anchors::<T>::try_mutate(&key, position, |slot| -> DispatchResult {
        let held = slot.as_mut().ok_or(Error::<T>::AnchorUnknown)?;
        held.status = AnchorStatus::Confirmed;
        held.height = evidence.height;
        held.reference = found.1;
        held.confirmations = depth;
        Ok(())
    })?;

    Pallet::<T>::log(
        &deployment,
        "anchor.confirmed",
        WorkClass::Structural,
        who,
        "The far network holds the state root the deployment anchored.",
        (position, evidence.height, found.1, depth).encode(),
    )?;
    Pallet::<T>::deposit_event(Event::AnchorConfirmed {
        deployment,
        endpoint: name,
        anchor: position,
        height: evidence.height,
    });
    Ok(())
}

/// Read the anchored root and the transaction identifier from one piece of
/// evidence.
pub fn read_anchor_output(
    params: &crate::endpoint::BitcoinParameters,
    evidence: &AnchorEvidence,
    merkle_root: &[u8; 32],
) -> Result<([u8; 32], [u8; 32]), BridgeFault> {
    let transaction = bitcoin::read_transaction(evidence.transaction.as_slice())?;
    let proof = bitcoin::BitcoinProof {
        height: evidence.height,
        transaction: evidence.transaction.clone(),
        position: evidence.position,
        branch: evidence.branch.clone(),
        output_index: evidence.output_index,
    };
    bitcoin::check_branch(&transaction.txid, &proof, merkle_root)?;
    let output = transaction
        .outputs
        .get(evidence.output_index as usize)
        .ok_or(BridgeFault::ValueAbsent)?;
    let root = bitcoin::read_anchor(&output.script, params.anchor_marker.as_slice())
        .ok_or(BridgeFault::ValueAbsent)?;
    Ok((root, transaction.txid))
}

/// The anchors of one endpoint, in position order.
pub fn listed<T: Config>(
    deployment: &DeploymentId,
    name: &BoundedName,
    limit: u32,
) -> Vec<(u64, AnchorRecord<BlockNumberFor<T>>)> {
    let key = EndpointKey::new(*deployment, name.clone());
    let mut found: Vec<(u64, AnchorRecord<BlockNumberFor<T>>)> =
        Anchors::<T>::iter_prefix(&key).collect();
    found.sort_by(|left, right| left.0.cmp(&right.0));
    found.truncate(limit as usize);
    found
}

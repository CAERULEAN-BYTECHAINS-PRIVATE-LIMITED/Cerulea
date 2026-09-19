//! Inbound message execution with class assignment.
//!
//! An arriving message runs five steps in order.
//!
//! 1. **The endpoint.** The module reads the endpoint the envelope names
//!    and checks that the endpoint takes arriving messages.
//! 2. **The expiry.** The module compares the moment the envelope names
//!    with the clock of the runtime.
//! 3. **The proof.** The verifier of the network kind checks the proof
//!    against the accepted headers and the verifier parameters.
//! 4. **The replay registry.** The module writes the identifier of the
//!    message. A second attempt finds the entry and stops.
//! 5. **The action.** The module runs the Blueprint action the endpoint
//!    names through the logic module.
//!
//! A failure at any step aborts the whole dispatch. The five steps run
//! inside one call, so the Blockchain commits all five or none.
//!
//! # The transaction class
//!
//! The envelope carries a class. The endpoint carries a class. The message
//! runs under the higher of the two, so an endpoint raises the bar of
//! every message that reaches it and never lowers the bar a sender set.
//!
//! # The actor context
//!
//! The far network sender holds no account on this chain. The action runs
//! as the account the endpoint names as its executor. The audit record
//! names the network the message came from and the address that sent it,
//! so the log carries the original source.

use crate::endpoint::{EndpointKey, EndpointState};
use crate::envelope::{MessageEnvelope, MessageId, Travel};
use crate::verify::{self, Proof};
use crate::{
    ActionRunner, Config, Error, Event, Inbound, InboundRecord, MessageStatus, Pallet, Seen,
    SeenRecord, StoredHeaders,
};
use alloc::vec::Vec;
use codec::Encode;
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_cerulea_audit::contract::WorkClass;

/// Deliver one inbound message with its proof.
pub fn deliver<T: Config>(
    who: &T::AccountId,
    envelope: &MessageEnvelope,
    proof: &Proof,
) -> DispatchResult {
    let key = EndpointKey::new(envelope.deployment, envelope.endpoint.clone());
    let record = crate::Endpoints::<T>::get(&key).ok_or(Error::<T>::EndpointUnknown)?;
    ensure!(record.live, Error::<T>::EndpointRetired);
    ensure!(record.takes(Travel::Inbound), Error::<T>::DirectionDeclined);
    ensure!(record.kind == proof.kind(), Error::<T>::KindMismatch);
    ensure!(
        record.format.accepts(envelope.payload.len()),
        Error::<T>::PayloadTooLarge
    );

    let now = Pallet::<T>::now_millis();
    ensure!(envelope.is_live(now), Error::<T>::MessageExpired);

    let id = envelope.id();
    ensure!(
        Seen::<T>::get(&key, id).is_none(),
        Error::<T>::MessageReplayed
    );

    let headers = StoredHeaders::<T>::new(key.clone());
    let state = EndpointState::new(&record.params, record.format, &headers, now);
    let accepted =
        verify::verify(envelope, proof, &state).map_err(|_| Error::<T>::ProofDeclined)?;

    Seen::<T>::insert(
        &key,
        id,
        SeenRecord::<BlockNumberFor<T>> {
            at: frame_system::Pallet::<T>::block_number(),
            height: accepted.height,
            travel: Travel::Inbound,
        },
    );

    let class = class_of(envelope.class, record.class);
    let effects = T::Actions::run(
        &envelope.deployment,
        &record.executor,
        core::str::from_utf8(record.action.as_slice()).unwrap_or_default(),
        envelope.payload.as_slice(),
        record.format,
    )?;

    Inbound::<T>::insert(
        &key,
        id,
        InboundRecord::<BlockNumberFor<T>> {
            envelope: envelope.clone(),
            height: accepted.height,
            block_hash: accepted.block_hash,
            trust: accepted.trust,
            status: MessageStatus::Executed,
            accepted_at: frame_system::Pallet::<T>::block_number(),
            effects,
        },
    );

    let actor = Pallet::<T>::source_account(&envelope.source, envelope.party.as_slice());
    Pallet::<T>::log(
        &envelope.deployment,
        "message.executed",
        class,
        &actor,
        "The deployment ran the action a verified bridge message targets.",
        context(envelope, &id, accepted.height),
    )?;
    Pallet::<T>::log(
        &envelope.deployment,
        "message.relayed",
        WorkClass::Routine,
        who,
        "The relayer submitted a verified bridge message.",
        id.encode(),
    )?;

    Pallet::<T>::deposit_event(Event::MessageExecuted {
        deployment: envelope.deployment,
        endpoint: envelope.endpoint.clone(),
        id,
        height: accepted.height,
        effects,
    });
    Ok(())
}

/// The class one arriving message runs under.
///
/// The message runs under the higher of the class the envelope carries and
/// the class the endpoint carries.
pub fn class_of(carried: WorkClass, endpoint: WorkClass) -> WorkClass {
    if endpoint > carried {
        endpoint
    } else {
        carried
    }
}

/// The payload one audit record carries for an arriving message.
pub fn context(envelope: &MessageEnvelope, id: &MessageId, height: u64) -> Vec<u8> {
    (
        envelope.source.clone(),
        envelope.party.clone(),
        envelope.nonce,
        *id,
        height,
    )
        .encode()
}

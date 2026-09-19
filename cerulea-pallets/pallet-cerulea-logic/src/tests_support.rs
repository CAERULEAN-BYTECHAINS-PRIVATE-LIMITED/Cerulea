//! Shared setup the logic module tests use.
//!
//! The tests run against the real permission module, so every write needs
//! a real role and a real capability entry. These helpers build that
//! setting once.

use crate::mock::*;
use codec::Encode;
use frame_support::{assert_ok, BoundedVec};
use pallet_cerulea_permission::{CapabilityKey, GrantPayload};
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};

/// The account the tests act as.
pub const ACTOR: u64 = 1;

/// Turn text into a bounded name.
pub fn name(text: &str) -> BoundedName {
    name_of(text).expect("the name fits")
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

/// Register the actor, define the registrar role, assign it and grant the
/// role every capability over the named collections.
pub fn grant_over(deployment: DeploymentId, collections: &[&str], actions: &[&str]) {
    assert_ok!(Identity::register_account(
        RuntimeOrigin::signed(ACTOR),
        ACTOR,
        [7u8; 32]
    ));
    assert_ok!(Identity::define_role(
        RuntimeOrigin::signed(ACTOR),
        deployment,
        name("registrar"),
        None,
        BoundedVec::default(),
        BoundedVec::truncate_from(b"Registrar".to_vec())
    ));
    assert_ok!(Identity::assign_role(
        RuntimeOrigin::signed(ACTOR),
        deployment,
        ACTOR,
        name("registrar"),
        None
    ));
    for collection in collections.iter() {
        for capability in [
            CapabilityKey::Read,
            CapabilityKey::Insert,
            CapabilityKey::Update,
            CapabilityKey::Delete,
        ] {
            assert_ok!(Permission::grant(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("registrar"),
                name(collection),
                capability,
                bounded(GrantPayload::everything().encode())
            ));
        }
        for action in actions.iter() {
            assert_ok!(Permission::grant(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("registrar"),
                name(collection),
                CapabilityKey::Invoke(name(action)),
                bounded(GrantPayload::everything().encode())
            ));
        }
    }
}

/// Grant the actor every capability over the parcel collection.
pub fn grant_everything(deployment: DeploymentId) {
    grant_over(deployment, &["parcel"], &[]);
}

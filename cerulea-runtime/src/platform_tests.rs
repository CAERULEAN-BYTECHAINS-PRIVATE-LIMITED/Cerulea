//! The platform module wiring and the platform runtime interfaces.
//!
//! The tests here run against the real runtime rather than a mock. Each one
//! builds the development genesis preset, so the preset itself is under
//! test beside the modules it fills.

use crate::genesis_config_presets::{
    cerulea_operator, starting_blueprint, starting_deployment, starting_licence_message,
    DEVELOPMENT_LICENCE_SIGNATURE,
};
use crate::{Runtime, RuntimeGenesisConfig};
use sp_core::Pair;
use sp_keyring::Ed25519Keyring;
use sp_runtime::BuildStorage;

/// Build the externalities of a development chain.
pub fn development_ext() -> sp_io::TestExternalities {
    let json = crate::genesis_config_presets::development_config_genesis();
    let patch = serde_json::to_vec(&json).expect("the development preset serialises");
    let storage = frame_system::GenesisConfig::<Runtime>::default()
        .build_storage()
        .expect("the system module builds its storage");
    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| {
        frame_support::genesis_builder_helper::build_state::<RuntimeGenesisConfig>(patch)
            .expect("the development preset builds");
        frame_system::Pallet::<Runtime>::set_block_number(1);
    });
    ext.commit_all().expect("the genesis state commits");
    ext
}

#[test]
fn the_development_licence_signature_holds() {
    let message = starting_licence_message();
    let public = Ed25519Keyring::Alice.public();
    let held = sp_core::ed25519::Signature::from_raw(DEVELOPMENT_LICENCE_SIGNATURE);
    assert!(
        sp_core::ed25519::Pair::verify(&held, &message, &public),
        "The development licence signature fails the control plane key. Produce the signature again with the control plane key. The signature the message needs is {:?}",
        Ed25519Keyring::Alice.pair().sign(&message).0
    );
}

#[test]
fn the_development_preset_installs_the_starting_blueprint() {
    development_ext().execute_with(|| {
        let deployment = starting_deployment();
        let held = pallet_cerulea_logic::Installed::<Runtime>::get(deployment)
            .expect("the development preset installs the starting Blueprint");
        assert_eq!(held, starting_blueprint());
    });
}

#[test]
fn the_development_preset_installs_a_licence_that_holds() {
    development_ext().execute_with(|| {
        let deployment = starting_deployment();
        let licence = pallet_cerulea_tier::Licences::<Runtime>::get(deployment)
            .expect("the development preset installs a licence");
        assert_eq!(
            licence.tier.application,
            pallet_cerulea_tier::Application::Enterprise
        );
        assert_eq!(licence.tier.level, 1);
        assert_eq!(
            pallet_cerulea_tier::States::<Runtime>::get(deployment),
            Some(pallet_cerulea_tier::LicenceState::Live)
        );
    });
}

#[test]
fn the_development_preset_appoints_the_cerulea_operator() {
    development_ext().execute_with(|| {
        let deployment = starting_deployment();
        let record =
            pallet_cerulea_operator::Operators::<Runtime>::get(deployment, cerulea_operator())
                .expect("the development preset appoints the Cerulea operator");
        assert!(record.active);
    });
}

#[test]
fn every_platform_module_sits_at_its_design_index() {
    use frame_support::traits::PalletInfoAccess;
    assert_eq!(pallet_cerulea_schema::Pallet::<Runtime>::index(), 11);
    assert_eq!(pallet_cerulea_state::Pallet::<Runtime>::index(), 12);
    assert_eq!(pallet_cerulea_collections::Pallet::<Runtime>::index(), 13);
    assert_eq!(pallet_cerulea_identity::Pallet::<Runtime>::index(), 14);
    assert_eq!(pallet_cerulea_permission::Pallet::<Runtime>::index(), 15);
    assert_eq!(pallet_cerulea_logic::Pallet::<Runtime>::index(), 16);
    assert_eq!(pallet_cerulea_workflow::Pallet::<Runtime>::index(), 17);
    assert_eq!(pallet_cerulea_rules::Pallet::<Runtime>::index(), 18);
    assert_eq!(pallet_cerulea_governance::Pallet::<Runtime>::index(), 19);
    assert_eq!(pallet_cerulea_evolution::Pallet::<Runtime>::index(), 20);
    assert_eq!(pallet_cerulea_oracle::Pallet::<Runtime>::index(), 22);
    assert_eq!(pallet_cerulea_relayer::Pallet::<Runtime>::index(), 23);
    assert_eq!(pallet_cerulea_tier::Pallet::<Runtime>::index(), 24);
    assert_eq!(pallet_cerulea_audit::Pallet::<Runtime>::index(), 25);
    assert_eq!(pallet_cerulea_operator::Pallet::<Runtime>::index(), 26);
    assert_eq!(pallet_cerulea_custody::Pallet::<Runtime>::index(), 27);
}

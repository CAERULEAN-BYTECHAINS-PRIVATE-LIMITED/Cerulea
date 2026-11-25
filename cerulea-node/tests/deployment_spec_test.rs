//! The deployment specifications carry the validator licence key.
//!
//! Task P10-028. The public-access and private presets write the control
//! plane key into the validator registry at genesis, with the lowest licence
//! class the chain admits. The key comes from `CERULEA_CONTROL_PLANE_KEY` at
//! the build. A build naming no key, a malformed key or a development key
//! stops. Under the public-access specification a validator joins only under
//! a licence the control plane signed.

use cerulea_node::chain_spec::{deployment_chain_spec, DeploymentSpec};
use cerulea_runtime::genesis_config_presets::{
    parse_control_plane_key, private_config_genesis, public_access_config_genesis,
    DeploymentKeyError, CONTROL_PLANE_KEY_ENV,
};
use cerulea_runtime::{AccountId, Runtime, RuntimeOrigin};
use frame_support::{assert_noop, assert_ok};
use pallet_cerulea_dvf::engines::admission::licence::{
    attestation_payload, LicenceAttestation, LicenceClass,
};
use sp_core::{ed25519, Pair};
use sp_keyring::{Ed25519Keyring, Sr25519Keyring};
use sp_runtime::BuildStorage;

type Validators = pallet_cerulea_dvf::Pallet<Runtime>;

/// The control plane key the tests name. Its secret is no well known seed.
fn control_plane() -> ed25519::Pair {
    ed25519::Pair::from_seed(&[0x5a; 32])
}

fn hex_of(key: [u8; 32]) -> String {
    use std::fmt::Write as _;
    key.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{:02x}", byte);
        text
    })
}

/// Build the externalities of one deployment preset.
fn deployment_ext(genesis: serde_json::Value) -> sp_io::TestExternalities {
    let patch = serde_json::to_vec(&genesis).expect("the preset serialises");
    let storage = frame_system::GenesisConfig::<Runtime>::default()
        .build_storage()
        .expect("the system module builds its storage");
    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| {
        frame_support::genesis_builder_helper::build_state::<cerulea_runtime::RuntimeGenesisConfig>(
            patch,
        )
        .expect("the deployment preset builds");
        frame_system::Pallet::<Runtime>::set_block_number(1);
        pallet_timestamp::Pallet::<Runtime>::set_timestamp(1_700_000_000_000);
    });
    ext.commit_all().expect("the genesis state commits");
    ext
}

/// A public-access specification naming no key refuses to build.
#[test]
fn a_public_access_spec_without_the_control_plane_key_refuses_to_build() {
    let error = deployment_chain_spec(DeploymentSpec::PublicAccess, None)
        .err()
        .expect("a specification without a key stops");
    assert!(error.contains(CONTROL_PLANE_KEY_ENV), "{error}");
    assert!(error.contains("Fix:"), "{error}");
    assert!(deployment_chain_spec(DeploymentSpec::PublicAccess, Some("  ")).is_err());
    assert!(deployment_chain_spec(DeploymentSpec::Private, None).is_err());

    // A malformed key stops the build.
    assert_eq!(
        parse_control_plane_key(Some("0x1234")),
        Err(DeploymentKeyError::Malformed)
    );
    let mut wrong = hex_of(control_plane().public().0);
    wrong.replace_range(0..2, "zz");
    assert_eq!(
        parse_control_plane_key(Some(&wrong)),
        Err(DeploymentKeyError::Malformed)
    );

    // Every well known development key stops the build.
    for development in [
        Ed25519Keyring::Alice.public().0,
        Ed25519Keyring::Bob.public().0,
        Sr25519Keyring::Alice.public().0,
    ] {
        assert_eq!(
            parse_control_plane_key(Some(&hex_of(development))),
            Err(DeploymentKeyError::DevelopmentKey)
        );
        assert!(
            deployment_chain_spec(DeploymentSpec::PublicAccess, Some(&hex_of(development)))
                .is_err()
        );
    }

    // The control plane key builds both specifications, with or without 0x.
    let key = control_plane().public().0;
    assert_eq!(
        parse_control_plane_key(Some(&format!("0x{}", hex_of(key)))),
        Ok(key)
    );
    for kind in [DeploymentSpec::PublicAccess, DeploymentSpec::Private] {
        let spec = deployment_chain_spec(kind, Some(&hex_of(key))).expect("the key builds");
        let json = spec.as_json(false).expect("the specification serialises");
        assert!(json.contains("validatorLicenceKey"), "{json}");
        assert!(json.contains("requiredLicenceClass"), "{json}");
    }
}

/// Under the public-access specification a join without a licence is refused.
#[test]
fn a_join_without_a_licence_is_refused_under_the_public_access_spec() {
    let issuer = control_plane();
    deployment_ext(public_access_config_genesis(issuer.public().0)).execute_with(|| {
        assert_eq!(
            pallet_cerulea_dvf::ValidatorLicenceKey::<Runtime>::get(),
            Some(issuer.public().0)
        );
        assert_eq!(
            pallet_cerulea_dvf::RequiredLicenceClass::<Runtime>::get(),
            LicenceClass::PublicAccess
        );

        let joiner: AccountId = Ed25519Keyring::Dave.to_account_id();
        assert_noop!(
            Validators::join_validators(RuntimeOrigin::signed(joiner.clone()), None),
            pallet_cerulea_dvf::Error::<Runtime>::LicenceRequired
        );

        // A private class licence sits below the class this chain asks for.
        let private = LicenceAttestation {
            holder: joiner.clone(),
            class: LicenceClass::Private,
            issued_at: 0,
            expires_at: 1_000,
            nonce: 1,
        };
        assert_noop!(
            Validators::submit_validator_licence(
                RuntimeOrigin::signed(joiner.clone()),
                private.clone(),
                issuer.sign(&attestation_payload(&private)).0
            ),
            pallet_cerulea_dvf::Error::<Runtime>::LicenceClassTooLow
        );

        // A public-access licence the control plane signed opens the join.
        let held = LicenceAttestation {
            holder: joiner.clone(),
            class: LicenceClass::PublicAccess,
            issued_at: 0,
            expires_at: 1_000,
            nonce: 1,
        };
        assert_ok!(Validators::submit_validator_licence(
            RuntimeOrigin::signed(joiner.clone()),
            held.clone(),
            issuer.sign(&attestation_payload(&held)).0
        ));
        assert_ok!(Validators::join_validators(
            RuntimeOrigin::signed(joiner.clone()),
            None
        ));
    });
}

/// The private specification asks for the private class at the least.
#[test]
fn the_private_spec_names_the_private_class() {
    let issuer = control_plane();
    deployment_ext(private_config_genesis(issuer.public().0)).execute_with(|| {
        assert_eq!(
            pallet_cerulea_dvf::ValidatorLicenceKey::<Runtime>::get(),
            Some(issuer.public().0)
        );
        assert_eq!(
            pallet_cerulea_dvf::RequiredLicenceClass::<Runtime>::get(),
            LicenceClass::Private
        );
        assert_noop!(
            Validators::join_validators(
                RuntimeOrigin::signed(Ed25519Keyring::Dave.to_account_id()),
                None
            ),
            pallet_cerulea_dvf::Error::<Runtime>::LicenceRequired
        );
    });
}

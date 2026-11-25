//! Further tests of the identity module.
//!
//! These tests cover the registry limits, the suspended account branches,
//! the reads a dependant module performs through the identity registry and
//! the plain sentences every fault carries.

use crate::{
    mock::*, tests::*, Assignment, Attestation, CredentialFault, Delegation, DelegationFault,
    DelegationOutcome, Delegations, Error, Event, HeldDelegations, IdentityRegistry, Pallet,
    RequirementOutcome, Units,
};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_schema::primitives::BoundedName;

fn numbered(prefix: &str, place: u32) -> BoundedName {
    let mut held = String::from(prefix);
    held.push('_');
    held.push_str(&place.to_string());
    name(&held)
}

fn define_role(role: &str) {
    assert_ok!(Identity::define_role(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name(role),
        None,
        credentials(&[]),
        text("A role")
    ));
}

fn suspend(who: u64) {
    assert_ok!(Identity::suspend_account(
        RuntimeOrigin::signed(STEWARD),
        who
    ));
}

/// Place one delegation straight into storage.
///
/// The calls refuse every broken chain when a link is made. A chain still
/// breaks later, so these tests place the broken shape directly and read
/// the resolution.
fn place_link(number: u64, grantor: u64, holder: u64, actions: &[&str], parent: Option<u64>) {
    Delegations::<Test>::insert(
        DEPLOYMENT,
        number,
        Delegation {
            grantor,
            holder,
            role: name("registrar"),
            scope: scope(actions),
            granted_at: START,
            expires_at: START + 10_000,
            parent,
            withdrawn: false,
        },
    );
    HeldDelegations::<Test>::mutate(DEPLOYMENT, holder, |held| {
        held.try_push(number).expect("the holder list has room");
    });
}

fn resolve(who: u64, action: &str) -> DelegationOutcome<u64> {
    <Pallet<Test> as IdentityRegistry>::delegation_for(&DEPLOYMENT, &who, &name(action), START)
}

#[test]
fn a_suspension_marks_the_account_inactive_and_writes_the_event() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        assert!(<Pallet<Test> as IdentityRegistry>::is_registered(&ANITHA));
        suspend(ANITHA);
        System::assert_last_event(RuntimeEvent::Identity(Event::AccountSuspended {
            who: ANITHA,
        }));
        assert!(!<Pallet<Test> as IdentityRegistry>::is_registered(&ANITHA));
        // A suspended account keeps its identity value.
        assert_eq!(
            <Pallet<Test> as IdentityRegistry>::identity_of(&ANITHA),
            Some([7u8; 32])
        );
    });
}

#[test]
fn a_suspension_of_an_absent_account_names_the_fix() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Identity::suspend_account(RuntimeOrigin::signed(STEWARD), ANITHA),
            Error::<Test>::AccountUnknown
        );
        assert!(!<Pallet<Test> as IdentityRegistry>::is_registered(&ANITHA));
        assert!(<Pallet<Test> as IdentityRegistry>::identity_of(&ANITHA).is_none());
    });
}

#[test]
fn a_suspended_account_takes_no_role_no_credential_and_no_delegation() {
    new_test_ext().execute_with(|| {
        registrar_with_role();
        suspend(BENNY);
        assert_noop!(
            Identity::assign_role(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                BENNY,
                name("registrar"),
                None
            ),
            Error::<Test>::AccountSuspended
        );
        assert_noop!(
            Identity::attest_credential(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                BENNY,
                name("survey_licence"),
                [1u8; 32],
                None
            ),
            Error::<Test>::AccountSuspended
        );
        assert_noop!(
            delegate_to(ANITHA, BENNY, &["read_parcel"], START + 10_000, None),
            Error::<Test>::AccountSuspended
        );
    });
}

#[test]
fn an_attestation_and_a_delegation_to_absent_parties_name_the_fix() {
    new_test_ext().execute_with(|| {
        registrar_with_role();
        assert_noop!(
            delegate_to(ANITHA, 42, &["read_parcel"], START + 10_000, None),
            Error::<Test>::AccountUnknown
        );
        assert_noop!(
            Identity::delegate(
                RuntimeOrigin::signed(ANITHA),
                DEPLOYMENT,
                BENNY,
                name("treasurer"),
                scope(&["read_parcel"]),
                START + 10_000,
                None
            ),
            Error::<Test>::RoleUnknown
        );
    });
}

#[test]
fn a_tree_past_its_unit_limit_names_the_fix() {
    new_test_ext().execute_with(|| {
        for place in 0..32u32 {
            assert_ok!(Identity::create_unit(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                numbered("office", place),
                None,
                text("Office"),
                text("One office of the district.")
            ));
        }
        assert_noop!(
            Identity::create_unit(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("office_extra"),
                None,
                text("Office"),
                text("One office too many.")
            ),
            Error::<Test>::UnitLimitReached
        );
    });
}

#[test]
fn a_deployment_past_its_role_limit_names_the_fix() {
    new_test_ext().execute_with(|| {
        for place in 0..32u32 {
            assert_ok!(Identity::define_role(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                numbered("role", place),
                None,
                credentials(&[]),
                text("A role")
            ));
        }
        assert_noop!(
            Identity::define_role(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("role_extra"),
                None,
                credentials(&[]),
                text("A role")
            ),
            Error::<Test>::RoleLimitReached
        );
    });
}

#[test]
fn an_account_past_its_role_limit_names_the_fix() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        for place in 0..9u32 {
            assert_ok!(Identity::define_role(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                numbered("role", place),
                None,
                credentials(&[]),
                text("A role")
            ));
        }
        for place in 0..8u32 {
            assert_ok!(Identity::assign_role(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                ANITHA,
                numbered("role", place),
                None
            ));
        }
        assert_noop!(
            Identity::assign_role(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                ANITHA,
                numbered("role", 8),
                None
            ),
            Error::<Test>::AccountRoleLimitReached
        );
        assert_eq!(
            <Pallet<Test> as IdentityRegistry>::roles_of(&DEPLOYMENT, &ANITHA, START).len(),
            8
        );
    });
}

#[test]
fn an_account_past_its_credential_limit_names_the_fix() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        for place in 0..8u32 {
            let credential = numbered("licence", place);
            assert_ok!(Identity::attest_credential(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                ANITHA,
                credential,
                [1u8; 32],
                None
            ));
        }
        assert_noop!(
            Identity::attest_credential(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                ANITHA,
                name("licence_extra"),
                [1u8; 32],
                None
            ),
            Error::<Test>::AccountCredentialLimitReached
        );
        assert_eq!(
            Pallet::<Test>::held_credentials(&DEPLOYMENT, &ANITHA).len(),
            8
        );
    });
}

#[test]
fn a_holder_past_its_delegation_limit_names_the_fix() {
    new_test_ext().execute_with(|| {
        registrar_with_role();
        for _ in 0..8 {
            assert_ok!(delegate_to(
                ANITHA,
                BENNY,
                &["read_parcel"],
                START + 10_000,
                None
            ));
        }
        assert_noop!(
            delegate_to(ANITHA, BENNY, &["read_parcel"], START + 10_000, None),
            Error::<Test>::DelegationLimitReached
        );
    });
}

#[test]
fn a_link_under_a_different_role_is_declined() {
    new_test_ext().execute_with(|| {
        registrar_with_role();
        define_role("clerk");
        assert_ok!(delegate_to(
            ANITHA,
            BENNY,
            &["read_parcel"],
            START + 10_000,
            None
        ));
        assert_noop!(
            Identity::delegate(
                RuntimeOrigin::signed(BENNY),
                DEPLOYMENT,
                CLERK,
                name("clerk"),
                scope(&["read_parcel"]),
                START + 5_000,
                Some(0)
            ),
            Error::<Test>::ScopeTooWide
        );
    });
}

#[test]
fn a_link_under_a_lapsed_parent_is_declined() {
    new_test_ext().execute_with(|| {
        registrar_with_role();
        assert_ok!(delegate_to(
            ANITHA,
            BENNY,
            &["read_parcel"],
            START + 1_000,
            None
        ));
        set_clock(START + 2_000);
        assert_noop!(
            delegate_to(BENNY, CLERK, &["read_parcel"], START + 3_000, Some(0)),
            Error::<Test>::DelegationNotHeld
        );
    });
}

#[test]
fn a_withdrawal_of_an_absent_delegation_names_the_fix() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Identity::revoke_delegation(RuntimeOrigin::signed(ANITHA), DEPLOYMENT, 3),
            Error::<Test>::DelegationUnknown
        );
    });
}

#[test]
fn a_withdrawal_writes_the_event() {
    new_test_ext().execute_with(|| {
        registrar_with_role();
        assert_ok!(delegate_to(
            ANITHA,
            BENNY,
            &["read_parcel"],
            START + 10_000,
            None
        ));
        assert_ok!(Identity::revoke_delegation(
            RuntimeOrigin::signed(ANITHA),
            DEPLOYMENT,
            0
        ));
        System::assert_last_event(RuntimeEvent::Identity(Event::DelegationWithdrawn {
            deployment: DEPLOYMENT,
            delegation: 0,
        }));
        assert!(
            Delegations::<Test>::get(DEPLOYMENT, 0)
                .expect("the registry keeps the withdrawn link")
                .withdrawn
        );
    });
}

#[test]
fn a_role_revocation_of_an_unheld_role_names_the_fix() {
    new_test_ext().execute_with(|| {
        registrar_with_role();
        assert_noop!(
            Identity::revoke_role(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                BENNY,
                name("registrar")
            ),
            Error::<Test>::RoleNotHeld
        );
        assert_ok!(Identity::revoke_role(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            ANITHA,
            name("registrar")
        ));
        System::assert_last_event(RuntimeEvent::Identity(Event::RoleRevoked {
            deployment: DEPLOYMENT,
            who: ANITHA,
            role: name("registrar"),
        }));
    });
}

#[test]
fn a_link_whose_parent_left_storage_resolves_to_a_broken_chain() {
    new_test_ext().execute_with(|| {
        registrar_with_role();
        place_link(5, BENNY, CLERK, &["read_parcel"], Some(4));
        assert_eq!(
            resolve(CLERK, "read_parcel").fault(),
            Some(DelegationFault::ChainBroken)
        );
    });
}

#[test]
fn a_link_passing_an_action_its_parent_lacks_resolves_to_out_of_scope() {
    new_test_ext().execute_with(|| {
        registrar_with_role();
        place_link(0, ANITHA, BENNY, &["read_parcel"], None);
        place_link(1, BENNY, CLERK, &["record_transfer"], Some(0));
        assert_eq!(
            resolve(CLERK, "record_transfer").fault(),
            Some(DelegationFault::OutOfScope)
        );
        // The link that carries the action from the role holder still
        // resolves.
        assert!(resolve(BENNY, "read_parcel").is_allowed());
    });
}

#[test]
fn a_chain_longer_than_the_depth_limit_resolves_to_depth_exceeded() {
    new_test_ext().execute_with(|| {
        registrar_with_role();
        register(5, 11);
        register(6, 12);
        place_link(0, ANITHA, BENNY, &["read_parcel"], None);
        place_link(1, BENNY, CLERK, &["read_parcel"], Some(0));
        place_link(2, CLERK, 5, &["read_parcel"], Some(1));
        place_link(3, 5, 6, &["read_parcel"], Some(2));
        assert_eq!(
            resolve(6, "read_parcel").fault(),
            Some(DelegationFault::DepthExceeded)
        );
        assert_eq!(Pallet::<Test>::chain_depth(&DEPLOYMENT, 3), 3);
    });
}

#[test]
fn the_first_usable_link_wins_over_an_earlier_broken_one() {
    new_test_ext().execute_with(|| {
        registrar_with_role();
        place_link(0, ANITHA, BENNY, &["read_parcel"], Some(9));
        place_link(1, ANITHA, BENNY, &["read_parcel"], None);
        match resolve(BENNY, "read_parcel") {
            DelegationOutcome::Allowed { delegation, .. } => assert_eq!(delegation, 1),
            DelegationOutcome::Denied(fault) => {
                unreachable!("the second link resolves, the outcome was {:?}", fault)
            }
        }
    });
}

#[test]
fn a_unit_chain_stops_at_a_loop_in_the_tree() {
    new_test_ext().execute_with(|| {
        build_tree();
        // A tree written with a loop still yields a bounded chain.
        Units::<Test>::mutate(DEPLOYMENT, name("district"), |held| {
            if let Some(unit) = held.as_mut() {
                unit.parent = Some(name("taluk"));
            }
        });
        assert_eq!(
            <Pallet<Test> as IdentityRegistry>::unit_chain(&DEPLOYMENT, &name("taluk")),
            vec![name("taluk"), name("district")]
        );
    });
}

#[test]
fn the_registry_answers_unit_credential_and_clock_questions() {
    new_test_ext().execute_with(|| {
        register(ANITHA, 7);
        build_tree();
        define_registrar();
        define_surveyor();
        assert_eq!(
            <Pallet<Test> as IdentityRegistry>::unit_of(&DEPLOYMENT, &name("registrar")),
            Some(name("taluk"))
        );
        assert!(
            <Pallet<Test> as IdentityRegistry>::unit_of(&DEPLOYMENT, &name("absent")).is_none()
        );
        assert_eq!(<Pallet<Test> as IdentityRegistry>::now_millis(), START);

        assert!(!<Pallet<Test> as IdentityRegistry>::has_credential(
            &DEPLOYMENT,
            &ANITHA,
            &name("survey_licence"),
            START
        ));
        let before = <Pallet<Test> as IdentityRegistry>::requirements_of(
            &DEPLOYMENT,
            &ANITHA,
            &name("surveyor"),
            START,
        );
        assert!(!before.holds());
        assert_eq!(
            before.first(),
            Some(&(name("survey_licence"), CredentialFault::Absent))
        );

        attest(STEWARD, ANITHA, "survey_licence", None);
        assert!(<Pallet<Test> as IdentityRegistry>::has_credential(
            &DEPLOYMENT,
            &ANITHA,
            &name("survey_licence"),
            START
        ));
        assert!(<Pallet<Test> as IdentityRegistry>::requirements_of(
            &DEPLOYMENT,
            &ANITHA,
            &name("surveyor"),
            START
        )
        .holds());
        // A role the registry lacks carries no requirement.
        assert_eq!(
            Pallet::<Test>::requirements(&DEPLOYMENT, &ANITHA, &name("absent"), START),
            RequirementOutcome::clean()
        );
    });
}

#[test]
fn an_absent_or_suspended_account_holds_no_role() {
    new_test_ext().execute_with(|| {
        registrar_with_role();
        assert!(<Pallet<Test> as IdentityRegistry>::roles_of(&DEPLOYMENT, &42, START).is_empty());
        assert!(<Pallet<Test> as IdentityRegistry>::has_role(
            &DEPLOYMENT,
            &ANITHA,
            &name("registrar"),
            START
        ));
        suspend(ANITHA);
        assert!(!<Pallet<Test> as IdentityRegistry>::has_role(
            &DEPLOYMENT,
            &ANITHA,
            &name("registrar"),
            START
        ));
    });
}

#[test]
fn an_assignment_holds_until_its_expiry() {
    let open = Assignment {
        granter: STEWARD,
        granted_at: START,
        expires_at: None,
    };
    assert!(open.holds_at(u64::MAX));
    let bounded = Assignment {
        granter: STEWARD,
        granted_at: START,
        expires_at: Some(START + 5),
    };
    assert!(bounded.holds_at(START + 4));
    assert!(!bounded.holds_at(START + 5));
}

#[test]
fn an_attestation_reports_its_horizon_and_its_fault() {
    let held = Attestation {
        issuer: STEWARD,
        issued_at: START,
        expires_at: Some(START + 5),
        evidence: [0u8; 32],
        withdrawn: false,
    };
    assert_eq!(held.horizon(), START + 5);
    assert_eq!(held.holds_at(START), Ok(()));
    assert_eq!(held.holds_at(START + 5), Err(CredentialFault::Expired));
    let withdrawn = Attestation {
        withdrawn: true,
        ..held
    };
    assert_eq!(withdrawn.holds_at(START), Err(CredentialFault::Withdrawn));
}

#[test]
fn every_credential_fault_names_the_problem_and_the_fix() {
    let expected = [
        (
            CredentialFault::Absent,
            "The account holds no attestation under that credential.",
            "Ask the issuer to attest the credential.",
        ),
        (
            CredentialFault::Expired,
            "The attestation of that credential lapsed.",
            "Ask the issuer to attest the credential again.",
        ),
        (
            CredentialFault::Withdrawn,
            "The issuer withdrew that attestation.",
            "Ask the issuer to attest the credential again.",
        ),
    ];
    for (fault, problem, fix) in expected {
        assert_eq!(fault.problem(), problem);
        assert_eq!(fault.fix(), fix);
    }
}

#[test]
fn every_delegation_fault_names_the_problem_and_the_fix() {
    let expected = [
        (
            DelegationFault::Absent,
            "No delegation passes that action to the account.",
            "Ask the role holder to delegate the action.",
        ),
        (
            DelegationFault::Expired,
            "The delegation lapsed.",
            "Ask the grantor to delegate the action again.",
        ),
        (
            DelegationFault::Withdrawn,
            "The grantor withdrew the delegation.",
            "Ask the grantor to delegate the action again.",
        ),
        (
            DelegationFault::OutOfScope,
            "The delegation passes an action the delegation above it lacks.",
            "Delegate an action the delegation above it already carries.",
        ),
        (
            DelegationFault::RoleLost,
            "A grantor on the delegation chain lost the role the chain runs under.",
            "Ask a present role holder to delegate the action.",
        ),
        (
            DelegationFault::DepthExceeded,
            "The delegation chain runs past the depth the Blockchain admits.",
            "Take the delegation from a holder nearer the role.",
        ),
        (
            DelegationFault::ChainBroken,
            "The delegation names a delegation the registry lacks.",
            "Ask a present role holder to delegate the action.",
        ),
    ];
    for (fault, problem, fix) in expected {
        assert_eq!(fault.problem(), problem);
        assert_eq!(fault.fix(), fix);
        let mut line = String::from(problem);
        line.push(' ');
        line.push_str(fix);
        assert_eq!(fault.line(), line);
    }
}

#[test]
fn an_allowed_outcome_carries_no_fault() {
    let allowed: DelegationOutcome<u64> = DelegationOutcome::Allowed {
        delegation: 0,
        role: name("registrar"),
        source: ANITHA,
        expires_at: START,
        depth: 1,
    };
    assert!(allowed.fault().is_none());
    let denied: DelegationOutcome<u64> = DelegationOutcome::Denied(DelegationFault::Expired);
    assert!(!denied.is_allowed());
    assert_eq!(denied.fault(), Some(DelegationFault::Expired));
}

#[test]
fn a_delegation_holds_until_its_expiry_or_its_withdrawal() {
    let link: Delegation<u64, frame_support::traits::ConstU32<8>> = Delegation {
        grantor: ANITHA,
        holder: BENNY,
        role: name("registrar"),
        scope: BoundedVec::truncate_from(vec![name("read_parcel")]),
        granted_at: START,
        expires_at: START + 5,
        parent: None,
        withdrawn: false,
    };
    assert!(link.holds_at(START));
    assert!(!link.holds_at(START + 5));
    assert!(link.covers(&name("read_parcel")));
    assert!(!link.covers(&name("record_transfer")));
    let withdrawn = Delegation {
        withdrawn: true,
        ..link
    };
    assert!(!withdrawn.holds_at(START));
}

//! Wire tests of the platform reply shapes.
//!
//! A reader outside the Blockchain decodes each reply from the bytes the
//! runtime sends. Each test builds one reply the way the runtime fills it,
//! sends it through the canonical encoding and reads it back. The tests also
//! pin the byte layout a reader depends on and the refusals a damaged reply
//! meets.

use super::*;
use alloc::vec;
use codec::{Decode, Encode};

/// Encode one value, decode it back and confirm the two agree.
fn round_trip<V: Encode + Decode + PartialEq + core::fmt::Debug>(value: &V) -> Vec<u8> {
    let bytes = value.encode();
    let read = V::decode(&mut &bytes[..]).expect("the canonical encoding reads back");
    assert_eq!(&read, value);
    bytes
}

/// Confirm every shorter prefix of an encoding fails to decode.
fn every_truncation_fails<V: Decode + core::fmt::Debug>(bytes: &[u8]) {
    for cut in 0..bytes.len() {
        assert!(
            V::decode(&mut &bytes[..cut]).is_err(),
            "a reply cut at byte {} of {} fails to decode",
            cut,
            bytes.len()
        );
    }
}

fn text(value: &str) -> Vec<u8> {
    value.as_bytes().to_vec()
}

fn collection() -> CollectionSummary {
    CollectionSummary {
        name: text("parcel"),
        record_type: text("parcel_record"),
        privacy: PrivacyLabel::HashOnly,
        records: 42,
        root: [7u8; 32],
        version: 3,
        retired: false,
        summary: text("The parcel collection holds one record per land parcel."),
    }
}

fn record() -> RecordReply {
    RecordReply {
        collection: text("parcel"),
        key: text("12/3"),
        value: Some(vec![1, 2, 3, 4]),
        commitment: [9u8; 32],
        version: 2,
        created_block: 10,
        changed_block: 14,
        privacy: PrivacyLabel::Open,
        history: vec![
            RecordVersionEntry {
                version: 1,
                commitment: [1u8; 32],
                block: 10,
                at_millis: 1_700_000_000_000,
            },
            RecordVersionEntry {
                version: 2,
                commitment: [9u8; 32],
                block: 14,
                at_millis: 1_700_000_024_000,
            },
        ],
        proof: Some(vec![0xaa, 0xbb]),
        root: [5u8; 32],
    }
}

#[test]
fn every_privacy_label_takes_its_own_index() {
    let labels = [
        PrivacyLabel::Open,
        PrivacyLabel::HashOnly,
        PrivacyLabel::Encrypted,
        PrivacyLabel::External,
    ];
    for (index, label) in labels.iter().enumerate() {
        let bytes = round_trip(label);
        assert_eq!(bytes, vec![index as u8]);
    }
    assert!(PrivacyLabel::decode(&mut &[4u8][..]).is_err());
    assert!(PrivacyLabel::decode(&mut &[][..]).is_err());
}

#[test]
fn a_collection_summary_reads_back_with_its_layout() {
    let held = collection();
    let bytes = round_trip(&held);
    // The name leads with its compact length, six bytes shifted left by two.
    assert_eq!(bytes[0], 6 << 2);
    assert_eq!(&bytes[1..7], b"parcel");
    every_truncation_fails::<CollectionSummary>(&bytes);

    let mut retired = held.clone();
    retired.retired = true;
    retired.privacy = PrivacyLabel::External;
    assert_ne!(round_trip(&retired), bytes);
}

#[test]
fn a_record_reply_carries_its_history_in_order() {
    let held = record();
    let bytes = round_trip(&held);
    let read = RecordReply::decode(&mut &bytes[..]).expect("reads");
    assert_eq!(read.history.first().map(|entry| entry.version), Some(1));
    assert_eq!(read.history.last().map(|entry| entry.block), Some(14));
    every_truncation_fails::<RecordReply>(&bytes);
}

#[test]
fn a_sealed_record_answers_with_no_value_and_no_proof() {
    let mut sealed = record();
    sealed.value = None;
    sealed.proof = None;
    sealed.privacy = PrivacyLabel::Encrypted;
    let sealed_bytes = round_trip(&sealed);
    let open_bytes = record().encode();
    // Two absent options each take one byte where the open record carries
    // its payload.
    assert!(sealed_bytes.len() < open_bytes.len());
    assert_eq!(
        RecordReply::decode(&mut &sealed_bytes[..])
            .expect("reads")
            .value,
        None
    );
}

#[test]
fn a_version_entry_holds_a_fixed_width() {
    let entry = RecordVersionEntry {
        version: 1,
        commitment: [3u8; 32],
        block: 77,
        at_millis: 5,
    };
    let bytes = round_trip(&entry);
    assert_eq!(bytes.len(), 4 + 32 + 4 + 8);
    assert_eq!(&bytes[..4], &1u32.to_le_bytes());
    every_truncation_fails::<RecordVersionEntry>(&bytes);
}

#[test]
fn a_role_reply_reads_back_with_its_field_list() {
    let held = RoleReply {
        role: text("registrar"),
        collection: text("parcel"),
        read: true,
        insert: true,
        update: false,
        delete: false,
        readable_fields: vec![text("survey_number"), text("holder")],
        every_field: false,
        actions: vec![text("record_transfer")],
        statement: text("The registrar reads two fields and records transfers."),
    };
    let bytes = round_trip(&held);
    every_truncation_fails::<RoleReply>(&bytes);
}

#[test]
fn a_permission_reply_carries_the_fix_on_a_refusal() {
    let refused = PermissionReply {
        admitted: false,
        role: None,
        fields: vec![],
        every_field: false,
        problem: text("The account holds no role on this collection."),
        fix: text("Assign a role that reaches the collection."),
    };
    let bytes = round_trip(&refused);
    let read = PermissionReply::decode(&mut &bytes[..]).expect("reads");
    assert!(!read.admitted);
    assert_eq!(read.fix, text("Assign a role that reaches the collection."));

    let admitted = PermissionReply {
        admitted: true,
        role: Some(text("registrar")),
        fields: vec![text("holder")],
        every_field: true,
        problem: vec![],
        fix: vec![],
    };
    let bytes = round_trip(&admitted);
    assert_eq!(bytes[0], 1);
    every_truncation_fails::<PermissionReply>(&bytes);
}

#[test]
fn a_view_reply_reads_back_row_by_row() {
    let held = ViewReply {
        view: text("open_parcels"),
        evaluated: true,
        rows: 2,
        encoded_rows: vec![vec![1, 2], vec![3, 4, 5]],
        digest: [2u8; 32],
        problem: vec![],
    };
    let bytes = round_trip(&held);
    every_truncation_fails::<ViewReply>(&bytes);
}

#[test]
fn a_workflow_instance_reads_back_with_its_steps_and_timers() {
    let step: ProcessStep = (text("approve"), text("filed"), text("approved"), 1_000);
    let held = WorkflowInstanceReply {
        workflow: text("transfer"),
        instance: 4,
        record: text("12/3"),
        state: text("approved"),
        status: text("running"),
        entered_at: 1_000,
        started_at: 500,
        started_block: 3,
        history: vec![step.clone()],
        scheduled: vec![(text("reminder"), 90_000)],
    };
    let bytes = round_trip(&held);
    let read = WorkflowInstanceReply::decode(&mut &bytes[..]).expect("reads");
    assert_eq!(read.history, vec![step]);
    every_truncation_fails::<WorkflowInstanceReply>(&bytes);
}

#[test]
fn a_proposal_reply_reads_back_before_and_after_the_decision() {
    let open = ProposalReply {
        id: 11,
        kind: text("schema"),
        class: text("structural"),
        threshold_percent: 80,
        payload_hash: [4u8; 32],
        status: text("open"),
        opened_at: 100,
        deadline: 200,
        approve_weight: 0,
        reject_weight: 0,
        voters: 0,
        roll_weight: 1_000,
        decided_at: None,
        executed_block: None,
    };
    let open_bytes = round_trip(&open);

    let mut executed = open.clone();
    executed.status = text("executed");
    executed.approve_weight = 850;
    executed.voters = 5;
    executed.decided_at = Some(180);
    executed.executed_block = Some(19);
    let executed_bytes = round_trip(&executed);
    assert!(executed_bytes.len() > open_bytes.len());
    every_truncation_fails::<ProposalReply>(&executed_bytes);
}

#[test]
fn a_vote_reply_carries_the_account_shape_of_the_chain() {
    let wide = VoteReply::<[u8; 32]> {
        voter: [8u8; 32],
        approve: true,
        weight: 250,
        at_millis: 9,
    };
    let wide_bytes = round_trip(&wide);
    assert_eq!(wide_bytes.len(), 32 + 1 + 8 + 8);

    let narrow = VoteReply::<u64> {
        voter: 8,
        approve: false,
        weight: 250,
        at_millis: 9,
    };
    let narrow_bytes = round_trip(&narrow);
    assert_eq!(narrow_bytes.len(), 8 + 1 + 8 + 8);
    every_truncation_fails::<VoteReply<u64>>(&narrow_bytes);
}

#[test]
fn an_audit_reply_reads_back_with_its_correction_link() {
    let held = AuditReply::<u64> {
        sequence: 17,
        event: text("record.corrected"),
        module: text("state"),
        class: text("routine"),
        actor: 3,
        at_millis: 1_000,
        block: 12,
        reason: text("The registrar corrected a holder name."),
        payload: vec![1, 2, 3],
        corrects: Some(16),
        digest: [6u8; 32],
    };
    let bytes = round_trip(&held);
    let read = AuditReply::<u64>::decode(&mut &bytes[..]).expect("reads");
    assert_eq!(read.corrects, Some(16));
    every_truncation_fails::<AuditReply<u64>>(&bytes);
}

#[test]
fn a_licence_reply_reads_back_for_a_live_and_a_suspended_deployment() {
    let live = LicenceReply {
        application: text("enterprise"),
        level: 2,
        features: vec![text("workflows"), text("views")],
        addons: vec![text("bridge")],
        seats: 50,
        seats_used: 12,
        expires_at: 2_000_000,
        state: text("live"),
        suspension_reason: vec![],
        production_permitted: true,
    };
    round_trip(&live);

    let suspended = LicenceReply {
        state: text("suspended"),
        suspension_reason: text("The licence lapsed. Renew the subscription."),
        production_permitted: false,
        ..live.clone()
    };
    let bytes = round_trip(&suspended);
    assert_eq!(*bytes.last().expect("a flag closes the reply"), 0);
    every_truncation_fails::<LicenceReply>(&bytes);
}

#[test]
fn an_oracle_reply_reads_back() {
    let held = OracleReply {
        feed: text("inr_usd"),
        round: 3,
        value: vec![0, 1, 2, 3],
        attested_at: 44,
    };
    let bytes = round_trip(&held);
    every_truncation_fails::<OracleReply>(&bytes);
}

#[test]
fn a_version_reply_reads_back() {
    let held = VersionReply {
        revision: 4,
        blueprint: [1u8; 32],
        recorded_at: 70,
        recorded_block: 8,
        summary: text("The revision adds the survey collection."),
        active: true,
    };
    let bytes = round_trip(&held);
    every_truncation_fails::<VersionReply>(&bytes);
}

#[test]
fn a_rollback_window_reads_back_open_and_closed() {
    let open = RollbackWindowReply {
        plan: 2,
        collection: text("parcel"),
        from_version: 1,
        to_version: 2,
        status: text("applied"),
        breaking: false,
        reversible: true,
        window_millis: 86_400_000,
        deadline: Some(90_000_000),
        open: true,
    };
    round_trip(&open);
    let closed = RollbackWindowReply {
        deadline: None,
        open: false,
        status: text("verified"),
        ..open
    };
    let bytes = round_trip(&closed);
    every_truncation_fails::<RollbackWindowReply>(&bytes);
}

#[test]
fn a_consensus_profile_carries_the_four_class_thresholds() {
    let held = ConsensusProfileReply {
        routine_percent: 51,
        financial_percent: 67,
        structural_percent: 80,
        constitutional_percent: 90,
        committee_size: 5,
        block_time_millis: 6_000,
        epoch_length: 600,
    };
    let bytes = round_trip(&held);
    assert_eq!(&bytes[..4], &[51, 67, 80, 90]);
    assert_eq!(bytes.len(), 4 + 4 + 8 + 4);
    every_truncation_fails::<ConsensusProfileReply>(&bytes);
}

#[test]
fn an_action_manifest_names_its_reads_and_writes() {
    let held = ActionManifestReply {
        action: text("record_transfer"),
        class: text("financial"),
        reads: vec![text("parcel")],
        writes: vec![text("parcel"), text("transfer_log")],
        roles: vec![text("registrar")],
        emits: vec![text("transfer_recorded")],
    };
    let bytes = round_trip(&held);
    let read = ActionManifestReply::decode(&mut &bytes[..]).expect("reads");
    assert_eq!(read.writes.len(), 2);
    every_truncation_fails::<ActionManifestReply>(&bytes);
}

#[test]
fn a_custody_reply_reads_back_answered_and_missed() {
    let answered = CustodyReply::<u64> {
        validator: 4,
        issued_block: 20,
        deadline_block: 23,
        answered: true,
        verified: true,
    };
    let bytes = round_trip(&answered);
    assert_eq!(bytes.len(), 8 + 4 + 4 + 1 + 1);
    let missed = CustodyReply::<u64> {
        answered: false,
        verified: false,
        ..answered
    };
    round_trip(&missed);
    every_truncation_fails::<CustodyReply<u64>>(&bytes);
}

#[test]
fn a_boolean_flag_outside_zero_and_one_fails_to_decode() {
    let mut bytes = CustodyReply::<u64> {
        validator: 1,
        issued_block: 2,
        deadline_block: 5,
        answered: true,
        verified: false,
    }
    .encode();
    let flag = bytes.len() - 2;
    bytes[flag] = 7;
    assert!(CustodyReply::<u64>::decode(&mut &bytes[..]).is_err());
}

#[test]
fn the_type_registry_names_every_reply_field() {
    use scale_info::{form::MetaForm, Type, TypeDef, TypeInfo};

    fn field_names<T: TypeInfo + 'static>() -> Vec<&'static str> {
        let held: Type<MetaForm> = T::type_info();
        match held.type_def {
            TypeDef::Composite(composite) => composite
                .fields
                .iter()
                .filter_map(|field| field.name)
                .collect(),
            _ => Vec::new(),
        }
    }

    assert_eq!(
        field_names::<CustodyReply<u64>>(),
        vec![
            "validator",
            "issued_block",
            "deadline_block",
            "answered",
            "verified"
        ]
    );
    assert_eq!(
        field_names::<ConsensusProfileReply>(),
        vec![
            "routine_percent",
            "financial_percent",
            "structural_percent",
            "constitutional_percent",
            "committee_size",
            "block_time_millis",
            "epoch_length"
        ]
    );
    assert!(field_names::<RecordReply>().contains(&"proof"));
    assert!(field_names::<LicenceReply>().contains(&"production_permitted"));

    // Every other reply registers its fields in declaration order, so a
    // reader decoding by the registry reads the same layout the encoding
    // carries.
    let registered: Vec<(Vec<&str>, &str, usize)> = vec![
        (field_names::<CollectionSummary>(), "name", 8),
        (field_names::<RecordVersionEntry>(), "version", 4),
        (field_names::<RoleReply>(), "role", 10),
        (field_names::<PermissionReply>(), "admitted", 6),
        (field_names::<ViewReply>(), "view", 6),
        (field_names::<WorkflowInstanceReply>(), "workflow", 10),
        (field_names::<ProposalReply>(), "id", 14),
        (field_names::<VoteReply<u64>>(), "voter", 4),
        (field_names::<AuditReply<u64>>(), "sequence", 11),
        (field_names::<OracleReply>(), "feed", 4),
        (field_names::<VersionReply>(), "revision", 6),
        (field_names::<RollbackWindowReply>(), "plan", 10),
        (field_names::<ActionManifestReply>(), "action", 6),
    ];
    for (names, first, count) in registered {
        assert_eq!(names.first(), Some(&first));
        assert_eq!(names.len(), count, "{first} registers every field");
    }
    match PrivacyLabel::type_info().type_def {
        TypeDef::Variant(variants) => {
            let names: Vec<&str> = variants.variants.iter().map(|held| held.name).collect();
            assert_eq!(names, vec!["Open", "HashOnly", "Encrypted", "External"]);
        }
        _ => panic!("the visibility class registers as a variant type"),
    }
}

#[test]
fn the_runtime_interface_registers_under_its_own_identifier() {
    use sp_api::RuntimeApiInfo;
    use sp_runtime::generic::{Block, Header};
    use sp_runtime::traits::BlakeTwo256;
    use sp_runtime::OpaqueExtrinsic;

    type ReaderBlock = Block<Header<u32, BlakeTwo256>, OpaqueExtrinsic>;

    let id = <dyn CeruleaPlatformApi<ReaderBlock, u64> as RuntimeApiInfo>::ID;
    let version = <dyn CeruleaPlatformApi<ReaderBlock, u64> as RuntimeApiInfo>::VERSION;
    assert_eq!(version, 1);
    assert_ne!(id, [0u8; 8]);
    // The identifier depends on the interface name alone, so every account
    // shape shares it.
    assert_eq!(
        id,
        <dyn CeruleaPlatformApi<ReaderBlock, [u8; 32]> as RuntimeApiInfo>::ID
    );
    assert_eq!(runtime_decl_for_cerulea_platform_api::ID, id);
    assert_eq!(runtime_decl_for_cerulea_platform_api::VERSION, version);
}

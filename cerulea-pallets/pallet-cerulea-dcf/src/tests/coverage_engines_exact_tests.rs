//! Every line and every region of the Assurance Engine class table and the
//! Standing Engine arithmetic.
//!
//! The engine modules under `crate::engines::assurance` and
//! `crate::engines::standing` hold the class thresholds and the Trust Score
//! arithmetic. The coverage rule for this code is 100 percent of lines and
//! regions. These tests reach the lines the behaviour tests leave: the derived
//! encodings, the type metadata, the debug text, every plain sentence and
//! every arm of every branch.

use crate::engines::assurance::{
    domain_commits, highest_class_present, required_weight, resolve_domain, ClassEntry,
    ClassTable, ClassTableError, DelayError, DelayWindow, DomainAssurance, ScheduledChange,
    ThresholdError, TransactionClass, CLASS_COUNT, CONSTITUTIONAL_DELAY_BLOCKS,
    MAX_THRESHOLD_PERCENT, MIN_THRESHOLD_PERCENT,
};
use crate::engines::standing::{
    accumulated, compose, decay_once, decayed, draw_sample, enter, epochs_remaining,
    epochs_until_floor, fault_record, idle_epochs, is_accurate, record_epoch, reset,
    settle_sample, shares, ChainKind, Components, DecaySettings, FaultKey, FaultRecord,
    Misconduct, ResetRecord, SampleOutcome, SamplingRecord, ServiceEvidence, ServiceRecord,
    TrustScore, DEFAULT_RAMP_EPOCHS, DEFAULT_SAMPLE_SIZE, EQUIVOCATION_PLAIN_LANGUAGE,
};
use crate::engines::standing::sampling::{
    AGREEMENT_GAIN, FALSE_REPORT_LOSS, FAULT_CAUGHT_GAIN, STARTING_SHARE as SAMPLING_START,
};
use cerulea_primitives::equivocation::EquivocationFacts;
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::Perbill;
use sp_std::fmt::Debug;

// ---------------------------------------------------------------------------
// Shared checks over the derived code
// ---------------------------------------------------------------------------

/// Encode a value, decode it back, and read it as debug text.
///
/// The derived encoding, decoding and debug code of every engine type runs
/// through here. A truncated encoding fails to decode.
fn round_trip<T: Encode + Decode + PartialEq + Debug>(value: &T) -> sp_std::vec::Vec<u8> {
    let bytes = value.encode();
    let back = T::decode(&mut &bytes[..]).expect("the encoding decodes");
    assert_eq!(&back, value);
    assert!(!format!("{value:?}").is_empty());
    if !bytes.is_empty() {
        assert!(T::decode(&mut &bytes[..bytes.len() - 1]).is_err());
    }
    bytes
}

/// The encoded length sits inside the declared bound.
fn inside_bound<T: Encode + MaxEncodedLen>(value: &T) {
    assert!(value.encode().len() <= T::max_encoded_len());
}

/// The type registry names the type.
fn named<T: TypeInfo + 'static>(name: &str) {
    assert_eq!(T::type_info().path.ident(), Some(name));
}

// ---------------------------------------------------------------------------
// Assurance Engine: the class table
// ---------------------------------------------------------------------------

#[test]
fn every_class_carries_a_label_and_a_sentence() {
    let labels = ["routine", "financial", "structural", "constitutional"];
    for (class, label) in TransactionClass::ALL.iter().zip(labels.iter()) {
        assert_eq!(class.label(), *label);
        assert!(class.plain_language().len() > 40);
        assert!(class.plain_language().ends_with('.'));
    }
    assert_eq!(TransactionClass::default(), TransactionClass::Routine);
    assert_eq!(TransactionClass::from_index(usize::MAX), None);
}

#[test]
fn the_class_types_encode_decode_and_describe_themselves() {
    for class in TransactionClass::ALL.iter() {
        let bytes = round_trip(class);
        assert_eq!(bytes, sp_std::vec![class.index() as u8]);
        inside_bound(class);
    }
    named::<TransactionClass>("TransactionClass");
    assert_eq!(
        TransactionClass::Routine.partial_cmp(&TransactionClass::Financial),
        Some(sp_std::cmp::Ordering::Less)
    );
    assert_eq!(
        TransactionClass::Constitutional.cmp(&TransactionClass::Constitutional),
        sp_std::cmp::Ordering::Equal
    );
    assert_eq!(
        TransactionClass::Structural.max(TransactionClass::Financial),
        TransactionClass::Structural
    );
    #[allow(clippy::clone_on_copy)]
    let copy = TransactionClass::Financial.clone();
    assert_eq!(copy, TransactionClass::Financial);

    let entry = ClassEntry {
        class: TransactionClass::Structural,
        threshold_percent: 80,
        delay_blocks: 0,
    };
    round_trip(&entry);
    inside_bound(&entry);
    named::<ClassEntry>("ClassEntry");
    assert!(!entry.waits_delay_window());

    let table = ClassTable::default();
    assert_eq!(table, ClassTable::starting_profile());
    let bytes = round_trip(&table);
    assert_eq!(bytes.len(), CLASS_COUNT * (1 + 1 + 4));
    inside_bound(&table);
    assert_eq!(ClassTable::max_encoded_len(), CLASS_COUNT * (1 + 1 + 4));
    named::<ClassTable>("ClassTable");
    #[allow(clippy::clone_on_copy)]
    let copy = table.clone();
    assert_eq!(copy, table);
}

#[test]
fn a_table_naming_one_class_twice_fails_inspection() {
    let mut table = ClassTable::starting_profile();
    // The financial row now names the routine class, with its share kept.
    table.entries[1].class = TransactionClass::Routine;
    assert_eq!(table.inspect(), Err(ClassTableError::DuplicateClass));
}

#[test]
fn the_inspection_failures_encode_and_name_their_fix() {
    let failures = [
        ClassTableError::DuplicateClass,
        ClassTableError::ThresholdTooLow,
        ClassTableError::ThresholdTooHigh,
        ClassTableError::ThresholdOrderBroken,
        ClassTableError::ConstitutionalDelayAbsent,
    ];
    for failure in failures.iter() {
        let bytes = round_trip(failure);
        assert_eq!(bytes.len(), 1);
        assert!(failure.plain_language().contains('.'));
    }
    named::<ClassTableError>("ClassTableError");
    #[allow(clippy::clone_on_copy)]
    let copy = ClassTableError::DuplicateClass.clone();
    assert_eq!(copy, ClassTableError::DuplicateClass);
}

#[test]
fn the_permitted_span_holds_the_starting_shares() {
    let table = ClassTable::starting_profile();
    for class in TransactionClass::ALL.iter() {
        let share = table.threshold_percent(*class);
        assert!(share >= MIN_THRESHOLD_PERCENT && share <= MAX_THRESHOLD_PERCENT);
    }
    assert_eq!(
        table.delay_blocks(TransactionClass::Constitutional),
        CONSTITUTIONAL_DELAY_BLOCKS
    );
}

// ---------------------------------------------------------------------------
// Assurance Engine: threshold resolution
// ---------------------------------------------------------------------------

#[test]
fn the_threshold_types_encode_decode_and_describe_themselves() {
    for failure in [ThresholdError::DomainEmpty, ThresholdError::TotalWeightZero] {
        round_trip(&failure);
        #[allow(clippy::clone_on_copy)]
        let copy = failure.clone();
        assert_eq!(copy, failure);
    }
    named::<ThresholdError>("ThresholdError");

    let assurance = resolve_domain(
        &ClassTable::starting_profile(),
        [TransactionClass::Structural],
        1_000,
    )
    .expect("resolves");
    let bytes = round_trip(&assurance);
    assert_eq!(bytes.len(), 1 + 1 + 16 + 4);
    inside_bound(&assurance);
    assert_eq!(DomainAssurance::max_encoded_len(), 1 + 1 + 16 + 4);
    named::<DomainAssurance>("DomainAssurance");
    #[allow(clippy::clone_on_copy)]
    let copy = assurance.clone();
    assert_eq!(copy, assurance);
    assert!(assurance.reached(800));
    assert!(!assurance.reached(799));
}

#[test]
fn an_empty_domain_fails_to_commit_at_any_weight() {
    let empty: [TransactionClass; 0] = [];
    assert_eq!(
        domain_commits(&ClassTable::starting_profile(), empty, 100, 100),
        Err(ThresholdError::DomainEmpty)
    );
    assert_eq!(highest_class_present(empty), Err(ThresholdError::DomainEmpty));
}

#[test]
fn the_highest_class_wins_whatever_the_order() {
    let rising = [TransactionClass::Routine, TransactionClass::Constitutional];
    let falling = [TransactionClass::Constitutional, TransactionClass::Routine];
    let equal = [TransactionClass::Financial, TransactionClass::Financial];
    assert_eq!(
        highest_class_present(rising),
        Ok(TransactionClass::Constitutional)
    );
    assert_eq!(
        highest_class_present(falling),
        Ok(TransactionClass::Constitutional)
    );
    assert_eq!(highest_class_present(equal), Ok(TransactionClass::Financial));
}

#[test]
fn the_required_weight_saturates_at_the_widest_total() {
    assert_eq!(required_weight(u128::MAX, 90), u128::MAX / 100 + 1);
    assert_eq!(required_weight(0, 51), 0);
    assert_eq!(required_weight(1, 51), 1);
}

// ---------------------------------------------------------------------------
// Assurance Engine: the delay window
// ---------------------------------------------------------------------------

#[test]
fn the_delay_types_encode_decode_and_describe_themselves() {
    for failure in [DelayError::WindowOpen(7), DelayError::AlreadyReleased] {
        round_trip(&failure);
        #[allow(clippy::clone_on_copy)]
        let copy = failure.clone();
        assert_eq!(copy, failure);
    }
    named::<DelayError>("DelayError");
    assert_eq!(DelayError::AlreadyReleased.blocks_remaining(), 0);

    let window = DelayWindow::open(10, 20);
    let bytes = round_trip(&window);
    assert_eq!(bytes.len(), 8);
    inside_bound(&window);
    assert_eq!(DelayWindow::default(), DelayWindow::open(0, 0));
    named::<DelayWindow>("DelayWindow");
    #[allow(clippy::clone_on_copy)]
    let copy = window.clone();
    assert_eq!(copy, window);
    assert_eq!(DelayWindow::open(u32::MAX, 5).effective_at, u32::MAX);
    assert_eq!(DelayWindow::open(5, 0).length(), 0);

    let change = ScheduledChange::schedule(
        &ClassTable::starting_profile(),
        TransactionClass::Constitutional,
        9u32,
        1,
    );
    let bytes = round_trip(&change);
    assert_eq!(bytes.len(), 1 + 8 + 4 + 1);
    inside_bound(&change);
    assert_eq!(ScheduledChange::<u32>::max_encoded_len(), 1 + 8 + 4 + 1);
    named::<ScheduledChange<u32>>("ScheduledChange");
    assert_eq!(change.clone(), change);
}

// ---------------------------------------------------------------------------
// Standing Engine: standing accumulated through service
// ---------------------------------------------------------------------------

#[test]
fn the_service_types_encode_decode_and_describe_themselves() {
    let record = enter(4);
    let bytes = round_trip(&record);
    assert_eq!(bytes.len(), 12);
    inside_bound(&record);
    assert_eq!(ServiceRecord::default().joined_epoch, 0);
    named::<ServiceRecord>("ServiceRecord");
    #[allow(clippy::clone_on_copy)]
    let copy = record.clone();
    assert_eq!(copy, record);

    let evidence = ServiceEvidence {
        custody_held: true,
        participated: true,
        sampling_accurate: true,
    };
    let bytes = round_trip(&evidence);
    assert_eq!(bytes.len(), 3);
    inside_bound(&evidence);
    assert!(!ServiceEvidence::default().meets_conditions());
    named::<ServiceEvidence>("ServiceEvidence");
    #[allow(clippy::clone_on_copy)]
    let copy = evidence.clone();
    assert_eq!(copy, evidence);
}

#[test]
fn the_service_sentence_names_the_first_condition_missed() {
    let full = ServiceEvidence {
        custody_held: true,
        participated: true,
        sampling_accurate: true,
    };
    assert_eq!(full.plain_language(), "The epoch counts as served.");
    assert!(ServiceEvidence {
        custody_held: false,
        ..full
    }
    .plain_language()
    .starts_with("The validator left a custody challenge"));
    assert!(ServiceEvidence {
        participated: false,
        ..full
    }
    .plain_language()
    .starts_with("The validator authored and voted on no block"));
    assert!(ServiceEvidence {
        sampling_accurate: false,
        ..full
    }
    .plain_language()
    .starts_with("The sampling report disagreed"));
}

#[test]
fn standing_climbs_the_ramp_in_a_straight_line() {
    let mut record = enter(1);
    assert_eq!(accumulated(&record, DEFAULT_RAMP_EPOCHS), Perbill::zero());
    assert_eq!(accumulated(&record, 0), Perbill::one());
    assert_eq!(epochs_remaining(&record, 10), 10);

    let full = ServiceEvidence {
        custody_held: true,
        participated: true,
        sampling_accurate: true,
    };
    let lacking = ServiceEvidence {
        custody_held: false,
        ..full
    };
    // An epoch missing a condition leaves the count where it stands.
    assert!(!record_epoch(&mut record, 1, lacking));
    assert_eq!(record.epochs_served, 0);
    // The first served epoch counts, and counts once.
    assert!(record_epoch(&mut record, 1, full));
    assert!(!record_epoch(&mut record, 1, full));
    assert_eq!(record.epochs_served, 1);
    assert_eq!(record.last_served_epoch, 1);
    // An earlier epoch arriving late counts no second time.
    assert!(!record_epoch(&mut record, 0, full));
    assert!(record_epoch(&mut record, 2, full));
    assert_eq!(accumulated(&record, 4), Perbill::from_percent(50));
    assert_eq!(epochs_remaining(&record, 4), 2);
    assert!(record_epoch(&mut record, 3, full));
    assert!(record_epoch(&mut record, 4, full));
    assert_eq!(accumulated(&record, 4), Perbill::one());
    assert_eq!(accumulated(&record, 3), Perbill::one());
    assert_eq!(epochs_remaining(&record, 3), 0);
}

#[test]
fn the_served_count_saturates() {
    let mut record = ServiceRecord {
        epochs_served: u32::MAX,
        last_served_epoch: 1,
        joined_epoch: 0,
    };
    let full = ServiceEvidence {
        custody_held: true,
        participated: true,
        sampling_accurate: true,
    };
    assert!(record_epoch(&mut record, 2, full));
    assert_eq!(record.epochs_served, u32::MAX);
}

// ---------------------------------------------------------------------------
// Standing Engine: decay
// ---------------------------------------------------------------------------

#[test]
fn the_decay_settings_encode_decode_and_describe_themselves() {
    let settings = DecaySettings::starting_profile();
    let bytes = round_trip(&settings);
    assert_eq!(bytes.len(), 8);
    inside_bound(&settings);
    assert_eq!(DecaySettings::default().decay_percent, 0);
    named::<DecaySettings>("DecaySettings");
    #[allow(clippy::clone_on_copy)]
    let copy = settings.clone();
    assert_eq!(copy, settings);
    assert_eq!(idle_epochs(10, 4), 6);
    assert_eq!(idle_epochs(4, 10), 0);
}

#[test]
fn one_idle_epoch_removes_a_share_and_at_least_one_part() {
    assert_eq!(decay_once(Perbill::zero(), 10), Perbill::zero());
    assert_eq!(decay_once(Perbill::one(), 0), Perbill::one());
    assert_eq!(
        decay_once(Perbill::one(), 10),
        Perbill::from_parts(900_000_000)
    );
    // A share too small for one percent still loses one part.
    assert_eq!(decay_once(Perbill::from_parts(5), 10), Perbill::from_parts(4));
    // A percent above one hundred removes everything.
    assert_eq!(decay_once(Perbill::one(), 150), Perbill::zero());
}

#[test]
fn a_run_of_idle_epochs_spends_the_grace_span_first() {
    let settings = DecaySettings {
        decay_percent: 50,
        grace_epochs: 2,
    };
    assert_eq!(decayed(Perbill::one(), 2, settings), Perbill::one());
    assert_eq!(
        decayed(Perbill::one(), 3, settings),
        Perbill::from_percent(50)
    );
    assert_eq!(
        decayed(Perbill::one(), 4, settings),
        Perbill::from_percent(25)
    );
    // A small standing reaches zero and the run stops there.
    assert_eq!(
        decayed(Perbill::from_parts(1), 100, settings),
        Perbill::zero()
    );
}

#[test]
fn the_epochs_until_a_floor_read_every_arm() {
    let settings = DecaySettings {
        decay_percent: 50,
        grace_epochs: 2,
    };
    // Already at or under the floor.
    assert_eq!(
        epochs_until_floor(Perbill::from_percent(10), Perbill::from_percent(10), settings),
        0
    );
    // No decay means no exit.
    assert_eq!(
        epochs_until_floor(
            Perbill::one(),
            Perbill::zero(),
            DecaySettings {
                decay_percent: 0,
                grace_epochs: 2
            }
        ),
        u32::MAX
    );
    // Two grace epochs, then one half each epoch: 100, 50, 25, 12.5 under 20.
    assert_eq!(
        epochs_until_floor(Perbill::one(), Perbill::from_percent(20), settings),
        5
    );
    // A positive standing reaches a floor of zero.
    assert!(epochs_until_floor(Perbill::from_parts(3), Perbill::zero(), settings) >= 3);
}

// ---------------------------------------------------------------------------
// Standing Engine: equivocation records
// ---------------------------------------------------------------------------

fn facts() -> EquivocationFacts {
    EquivocationFacts {
        slot: 77,
        rank: 2,
        seal_key: [5u8; 32],
        draw_key: [6u8; 32],
        first_hash: [1u8; 32],
        second_hash: [2u8; 32],
    }
}

#[test]
fn the_fault_types_encode_decode_and_describe_themselves() {
    let record = fault_record(&facts(), 3, 400, 12);
    assert_eq!(record.slot, 77);
    assert_eq!(record.rank, 2);
    assert_eq!(record.epoch, 3);
    assert_eq!(record.block, 400);
    assert_eq!(record.first_hash, [1u8; 32]);
    assert_eq!(record.second_hash, [2u8; 32]);
    assert_eq!(record.epochs_destroyed, 12);
    let bytes = round_trip(&record);
    assert_eq!(bytes.len(), 8 + 1 + 4 + 4 + 32 + 32 + 4);
    inside_bound(&record);
    named::<FaultRecord>("FaultRecord");
    #[allow(clippy::clone_on_copy)]
    let copy = record.clone();
    assert_eq!(copy, record);

    let key = FaultKey::of(&facts());
    assert_eq!(key, FaultKey::new(77, 2, [5u8; 32]));
    let bytes = round_trip(&key);
    assert_eq!(bytes.len(), 8 + 1 + 32);
    inside_bound(&key);
    named::<FaultKey>("FaultKey");
    #[allow(clippy::clone_on_copy)]
    let copy = key.clone();
    assert_eq!(copy, key);
    // The key orders by slot, then rank, then seal key.
    assert!(key < FaultKey::new(78, 0, [0u8; 32]));
    assert!(key < FaultKey::new(77, 3, [0u8; 32]));
    assert!(key < FaultKey::new(77, 2, [6u8; 32]));
    assert_eq!(key.partial_cmp(&key), Some(sp_std::cmp::Ordering::Equal));
    assert_eq!(key.cmp(&key), sp_std::cmp::Ordering::Equal);
    assert!(EQUIVOCATION_PLAIN_LANGUAGE.ends_with("the seat goes."));
}

// ---------------------------------------------------------------------------
// Standing Engine: resets
// ---------------------------------------------------------------------------

#[test]
fn every_misconduct_carries_a_sentence_and_a_seat_rule() {
    let kinds = [
        Misconduct::TwoBlocksAtOneSlot,
        Misconduct::TwoResultsAtOneHeight,
        Misconduct::FalseCustodyProof,
        Misconduct::FalseSamplingReport,
        Misconduct::LapsedLicence,
    ];
    for (position, kind) in kinds.iter().enumerate() {
        assert!(kind.plain_language().ends_with("Standing returns to zero."));
        assert_eq!(kind.removes_the_seat_at_once(), position < 2);
        let bytes = round_trip(kind);
        assert_eq!(bytes, sp_std::vec![position as u8]);
        inside_bound(kind);
        #[allow(clippy::clone_on_copy)]
        let copy = kind.clone();
        assert_eq!(copy, *kind);
    }
    named::<Misconduct>("Misconduct");
}

#[test]
fn a_reset_destroys_every_served_epoch_and_records_the_loss() {
    let mut record = ServiceRecord {
        epochs_served: 40,
        last_served_epoch: 39,
        joined_epoch: 0,
    };
    let outcome = reset(&mut record, Misconduct::LapsedLicence, 41);
    assert_eq!(
        outcome,
        ResetRecord {
            misconduct: Misconduct::LapsedLicence,
            epoch: 41,
            epochs_destroyed: 40,
        }
    );
    assert_eq!(record, enter(41));
    let bytes = round_trip(&outcome);
    assert_eq!(bytes.len(), 1 + 4 + 4);
    inside_bound(&outcome);
    named::<ResetRecord>("ResetRecord");
    #[allow(clippy::clone_on_copy)]
    let copy = outcome.clone();
    assert_eq!(copy, outcome);
}

// ---------------------------------------------------------------------------
// Standing Engine: verification sampling
// ---------------------------------------------------------------------------

#[test]
fn the_sampling_types_encode_decode_and_describe_themselves() {
    let outcomes = [
        SampleOutcome::Agreed,
        SampleOutcome::FaultCaught,
        SampleOutcome::FalseReport,
    ];
    for (position, outcome) in outcomes.iter().enumerate() {
        assert!(outcome.plain_language().contains("Standing"));
        let bytes = round_trip(outcome);
        assert_eq!(bytes, sp_std::vec![position as u8]);
        inside_bound(outcome);
        #[allow(clippy::clone_on_copy)]
        let copy = outcome.clone();
        assert_eq!(copy, *outcome);
    }
    assert_eq!(SampleOutcome::default(), SampleOutcome::Agreed);
    named::<SampleOutcome>("SampleOutcome");

    let record = SamplingRecord::default();
    let bytes = round_trip(&record);
    assert_eq!(bytes.len(), 16);
    inside_bound(&record);
    named::<SamplingRecord>("SamplingRecord");
    #[allow(clippy::clone_on_copy)]
    let copy = record.clone();
    assert_eq!(copy, record);
    assert_eq!(record.settled(), 0);
    assert_eq!(record.share(), Perbill::from_parts(SAMPLING_START));
    assert!(is_accurate(&record));
}

#[test]
fn the_sample_draw_reads_every_arm() {
    let hash = H256::repeat_byte(9);
    assert!(draw_sample(hash, 1, 0, DEFAULT_SAMPLE_SIZE).is_empty());
    assert!(draw_sample(hash, 1, 100, 0).is_empty());
    // A sample wider than the window draws the whole window.
    let whole = draw_sample(hash, 1, 3, DEFAULT_SAMPLE_SIZE);
    assert_eq!(whole, sp_std::vec![0, 1, 2]);
    // A sample inside the window draws distinct sorted positions.
    let drawn = draw_sample(hash, 7, 1_000, DEFAULT_SAMPLE_SIZE);
    assert_eq!(drawn.len(), DEFAULT_SAMPLE_SIZE as usize);
    assert!(drawn.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(drawn.iter().all(|position| *position < 1_000));
    // The same inputs draw the same sample; another round draws another.
    assert_eq!(drawn, draw_sample(hash, 7, 1_000, DEFAULT_SAMPLE_SIZE));
    assert_ne!(drawn, draw_sample(hash, 8, 1_000, DEFAULT_SAMPLE_SIZE));
}

#[test]
fn every_sampling_outcome_moves_the_share_and_the_count() {
    let mut record = SamplingRecord::default();
    assert_eq!(
        settle_sample(&mut record, SampleOutcome::Agreed),
        Perbill::from_parts(SAMPLING_START + AGREEMENT_GAIN)
    );
    assert_eq!(record.agreements, 1);
    assert_eq!(
        settle_sample(&mut record, SampleOutcome::FaultCaught),
        Perbill::from_parts(SAMPLING_START + AGREEMENT_GAIN + FAULT_CAUGHT_GAIN)
    );
    assert_eq!(record.faults_caught, 1);
    assert_eq!(
        settle_sample(&mut record, SampleOutcome::FalseReport),
        Perbill::from_parts(SAMPLING_START + AGREEMENT_GAIN + FAULT_CAUGHT_GAIN - FALSE_REPORT_LOSS)
    );
    assert_eq!(record.false_reports, 1);
    assert_eq!(record.settled(), 3);
    assert!(!is_accurate(&record));
    assert_eq!(record.share(), Perbill::from_parts(record.share));

    // The share caps at the whole and floors at zero.
    let mut high = SamplingRecord::default();
    for _ in 0..10 {
        settle_sample(&mut high, SampleOutcome::FaultCaught);
    }
    assert_eq!(high.share(), Perbill::one());
    for _ in 0..5 {
        settle_sample(&mut high, SampleOutcome::Agreed);
    }
    assert_eq!(high.share(), Perbill::one());
    let mut low = SamplingRecord::default();
    for _ in 0..5 {
        settle_sample(&mut low, SampleOutcome::FalseReport);
    }
    assert_eq!(low.share(), Perbill::zero());

    // Every count saturates.
    let mut full = SamplingRecord {
        agreements: u32::MAX,
        faults_caught: u32::MAX,
        false_reports: u32::MAX,
        share: 0,
    };
    assert_eq!(full.settled(), u32::MAX);
    settle_sample(&mut full, SampleOutcome::Agreed);
    settle_sample(&mut full, SampleOutcome::FaultCaught);
    settle_sample(&mut full, SampleOutcome::FalseReport);
    assert_eq!(full.agreements, u32::MAX);
    assert_eq!(full.faults_caught, u32::MAX);
    assert_eq!(full.false_reports, u32::MAX);
}

// ---------------------------------------------------------------------------
// Standing Engine: the Trust Score composition
// ---------------------------------------------------------------------------

#[test]
fn the_score_types_encode_decode_and_describe_themselves() {
    for kind in [ChainKind::PublicAccess, ChainKind::Private] {
        round_trip(&kind);
        inside_bound(&kind);
        #[allow(clippy::clone_on_copy)]
        let copy = kind.clone();
        assert_eq!(copy, kind);
        assert_eq!(shares(kind).iter().sum::<u32>(), 100);
    }
    assert_eq!(ChainKind::default(), ChainKind::PublicAccess);
    named::<ChainKind>("ChainKind");

    let components = Components {
        standing: Perbill::from_percent(50),
        performance: Perbill::from_percent(60),
        custody: Perbill::from_percent(70),
        sampling: Perbill::from_percent(80),
        role: Perbill::from_percent(90),
        attestation: Perbill::one(),
    };
    let bytes = round_trip(&components);
    assert_eq!(bytes.len(), 24);
    inside_bound(&components);
    assert_eq!(Components::default(), Components::default());
    named::<Components>("Components");
    #[allow(clippy::clone_on_copy)]
    let copy = components.clone();
    assert_eq!(copy, components);

    let score = compose(ChainKind::Private, components);
    let bytes = round_trip(&score);
    assert_eq!(bytes.len(), 28);
    inside_bound(&score);
    assert_eq!(TrustScore::default().total, Perbill::zero());
    named::<TrustScore>("TrustScore");
    #[allow(clippy::clone_on_copy)]
    let copy = score.clone();
    assert_eq!(copy, score);
}

#[test]
fn the_private_composition_weighs_each_component() {
    let components = Components {
        standing: Perbill::one(),
        performance: Perbill::from_percent(60),
        custody: Perbill::from_percent(40),
        sampling: Perbill::one(),
        role: Perbill::from_percent(80),
        attestation: Perbill::from_percent(20),
    };
    let score = compose(ChainKind::Private, components);
    // 35 of 80, 25 of 20, 25 of 60, 15 of 40: 28 + 5 + 15 + 6 = 54 percent.
    assert_eq!(score.total, Perbill::from_percent(54));
    assert_eq!(score.parts_per_billion(), 540_000_000);
    assert_eq!(score.parts_per_million(), 540_000);
    assert_eq!(score.weight(), 540_000_000);
    assert_eq!(score.role, Perbill::from_percent(80));
    assert_eq!(score.attestation, Perbill::from_percent(20));
    assert_eq!(score.standing, Perbill::zero());
    assert_eq!(score.sampling, Perbill::zero());
}

#[test]
fn the_public_composition_weighs_each_component() {
    let components = Components {
        standing: Perbill::from_percent(50),
        performance: Perbill::from_percent(100),
        custody: Perbill::from_percent(50),
        sampling: Perbill::zero(),
        role: Perbill::one(),
        attestation: Perbill::one(),
    };
    let score = compose(ChainKind::PublicAccess, components);
    // 40 of 50, 30 of 100, 20 of 50, 10 of 0: 20 + 30 + 10 = 60 percent.
    assert_eq!(score.total, Perbill::from_percent(60));
    assert_eq!(score.standing, Perbill::from_percent(50));
    assert_eq!(score.custody, Perbill::from_percent(50));
    assert_eq!(score.role, Perbill::zero());
    assert_eq!(score.attestation, Perbill::zero());
}

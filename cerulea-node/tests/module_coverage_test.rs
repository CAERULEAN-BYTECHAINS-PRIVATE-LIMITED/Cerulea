//! Every module of the catalogue installs and executes.
//!
//! The catalogue holds 225 modules. The Government set holds six more.
//! Every one carries a Blueprint fragment. The suite here reads each
//! fragment, resolves its dependency closure, merges it, validates it,
//! installs it on a test runtime and runs at least one action of the
//! fragment with generated parameter values.
//!
//! # What counts as a pass
//!
//! 1. **Installation.** The merged Blueprint validates and installs. A
//!    fragment failing here fails the suite.
//! 2. **Execution.** At least one action of the fragment runs. A run
//!    answers with success, or with a precondition failure carrying the
//!    plain sentence the person wrote beside the rule. A fragment whose
//!    every action fails to run fails the suite.
//!
//! # Boundedness
//!
//! Each fragment runs in its own externalities, so one fragment never sees
//! the state of another. The suite spreads the fragments across the
//! available cores, so the whole catalogue finishes inside a few minutes
//! on a laptop.

mod support;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use cerulea_blueprint::Blueprint;
use support::*;

/// What one fragment produced.
#[derive(Clone, Debug)]
struct Outcome {
    /// The fragment file name.
    name: String,
    /// The merged Blueprint validated.
    validated: bool,
    /// The merged Blueprint installed.
    installed: bool,
    /// The count of actions the fragment declares.
    actions: u32,
    /// The count of actions that ran.
    executed: u32,
    /// The count of actions a precondition refused with a plain sentence.
    refused: u32,
    /// One sentence stating the problem, where the fragment failed.
    problem: String,
}

impl Outcome {
    fn new(name: &str) -> Self {
        Outcome {
            name: name.to_string(),
            validated: false,
            installed: false,
            actions: 0,
            executed: 0,
            refused: 0,
            problem: String::new(),
        }
    }

    /// Report whether the fragment passed both gates.
    fn passed(&self) -> bool {
        self.installed && (self.actions == 0 || self.executed + self.refused > 0)
    }
}

/// Run one fragment on its own chain.
fn run_fragment(name: &str, merged: &Blueprint) -> Outcome {
    let mut outcome = Outcome::new(name);
    if let Err(report) = merged.validate() {
        outcome.problem = format!("The merged Blueprint carries diagnostics. {report}");
        return outcome;
    }
    outcome.validated = true;
    outcome.actions = u32::try_from(merged.actions.len()).unwrap_or(u32::MAX);

    development_ext().execute_with(|| {
        let deployment = match install(merged) {
            Ok(held) => held,
            Err(error) => {
                outcome.problem = format!(
                    "The Blueprint failed to install. The Blockchain reported {error:?}. Correct the fragment and install it again."
                );
                return;
            }
        };
        outcome.installed = true;
        seed_identity_and_permissions(deployment, merged);

        for action in merged.actions.iter() {
            let arguments = match sample_arguments(action, &merged.types) {
                Some(held) => held,
                None => continue,
            };
            match run_action(deployment, action, &arguments) {
                Ok(()) => outcome.executed = outcome.executed.saturating_add(1),
                Err(error) => {
                    // A precondition refusal carries the plain sentence the
                    // person wrote beside the rule. That counts as a run.
                    if refusal_carries_a_sentence(&error) {
                        outcome.refused = outcome.refused.saturating_add(1);
                    }
                }
            }
            if outcome.executed > 0 {
                break;
            }
        }

        if outcome.actions > 0 && outcome.executed + outcome.refused == 0 {
            outcome.problem =
                "Every action of the fragment failed to run. Check the preconditions and the capability matrix of the fragment."
                    .to_string();
        }
    });
    outcome
}

/// Report whether one refusal carries the plain sentence a person reads.
///
/// A precondition failure answers through the rules module, which carries
/// the two sentences the person wrote beside the rule. Every module error
/// of the platform set carries its own plain sentence beside its name.
fn refusal_carries_a_sentence(error: &sp_runtime::DispatchError) -> bool {
    match error {
        sp_runtime::DispatchError::Module(held) => held.message.is_some(),
        sp_runtime::DispatchError::Other(held) => !held.is_empty(),
        _ => false,
    }
}

#[test]
fn every_module_of_the_catalogue_installs_and_executes() {
    let library = fragment_library();
    if library.is_empty() {
        println!("The content repository is absent from this checkout, so the suite stands down.");
        return;
    }

    // The reference fragment is the worked example beside the catalogue
    // rather than a catalogue module, so it joins the run under its own
    // name.
    let names: Vec<String> = library.keys().cloned().collect();

    // Resolve every dependency closure first. Resolution reads the library
    // alone, so the work happens once and the chains run over the result.
    let mut resolved: BTreeMap<String, Blueprint> = BTreeMap::new();
    for name in names.iter() {
        match resolve(name, &library) {
            Some(held) => {
                resolved.insert(name.clone(), held);
            }
            None => panic!("{name} resolves to no Blueprint. Check the fragment on disk."),
        }
    }

    let outcomes: Mutex<Vec<Outcome>> = Mutex::new(Vec::new());
    let cursor = AtomicUsize::new(0);
    let order: Vec<String> = resolved.keys().cloned().collect();
    let workers = std::thread::available_parallelism()
        .map(|held| held.get())
        .unwrap_or(4)
        .clamp(1, 8);

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let index = cursor.fetch_add(1, Ordering::SeqCst);
                if index >= order.len() {
                    break;
                }
                let name = &order[index];
                let merged = resolved.get(name).expect("the closure resolved");
                let held = run_fragment(name, merged);
                outcomes.lock().expect("the report list locks").push(held);
            });
        }
    });

    let mut found = outcomes.into_inner().expect("the report list unlocks");
    found.sort_by(|left, right| left.name.cmp(&right.name));

    let installed = found.iter().filter(|held| held.installed).count();
    let executed = found
        .iter()
        .filter(|held| held.executed > 0 || held.refused > 0)
        .count();
    let with_actions = found.iter().filter(|held| held.actions > 0).count();
    let failures: Vec<&Outcome> = found.iter().filter(|held| !held.passed()).collect();

    println!("Cerulea module coverage");
    println!("  fragments read      {}", found.len());
    println!(
        "  Blueprints validated {}",
        found.iter().filter(|held| held.validated).count()
    );
    println!("  Blueprints installed {installed}");
    println!("  fragments declaring an action {with_actions}");
    println!("  fragments running an action   {executed}");
    println!(
        "    of which an action succeeded  {}",
        found.iter().filter(|held| held.executed > 0).count()
    );
    println!(
        "    of which a precondition refused with a sentence {}",
        found
            .iter()
            .filter(|held| held.executed == 0 && held.refused > 0)
            .count()
    );

    if !failures.is_empty() {
        let report: Vec<String> = failures
            .iter()
            .map(|held| format!("  {} {}", held.name, held.problem))
            .collect();
        panic!(
            "{} fragments failed the coverage gate.\n{}",
            failures.len(),
            report.join("\n")
        );
    }

    assert_eq!(
        installed,
        found.len(),
        "Every fragment installs. {installed} of {} installed.",
        found.len()
    );
    assert_eq!(
        executed, with_actions,
        "Every fragment declaring an action runs one. {executed} of {with_actions} ran."
    );
}

#[test]
fn the_catalogue_carries_the_full_module_set() {
    let library = fragment_library();
    if library.is_empty() {
        println!("The content repository is absent from this checkout, so the suite stands down.");
        return;
    }
    // The catalogue holds 225 modules plus the worked example. The
    // Government set holds six more.
    assert!(
        library.len() >= 231,
        "The content repository holds {} fragments. The catalogue holds 225 modules, the worked example and the six Government modules.",
        library.len()
    );
}

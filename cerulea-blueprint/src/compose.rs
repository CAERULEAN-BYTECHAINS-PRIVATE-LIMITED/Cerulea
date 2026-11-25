//! Reusable blocks.
//!
//! A user groups any subgraph into a named block with typed inputs and
//! typed outputs. The block appears in the palette and composes further. A
//! finite primitive set closed under composition generates an unbounded
//! library.
//!
//! A block carries the same effect steps an action carries. Running a block
//! produces the same effect trace the subgraph produced, so a block replaces
//! its subgraph with no change in behaviour.
//!
//! Composition stays acyclic. A block that reaches itself raises a
//! diagnostic naming the loop.

use crate::{
    actions::{Action, BlockBody, BlockOutput, BlockSource, Effect, Param},
    collections::CollectionRegistry,
    describe_cycle,
    expression::{Expr, TypeContext},
    find_cycle,
    types::{BlockId, ParamName, TypeExpr, TypeRegistry},
    Diagnostic, DiagnosticCode, Location, Path, Report,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    format,
    string::{String, ToString},
    vec::Vec,
};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// The deepest run of nested blocks the inliner walks.
pub const MAX_COMPOSITION_DEPTH: u32 = 16;

/// A reusable block.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct Block {
    /// The name of the block.
    pub id: BlockId,
    /// The typed inputs the block takes.
    pub inputs: Vec<Param>,
    /// The typed outputs the block produces.
    pub outputs: Vec<BlockOutput>,
    /// The effect sequence the block runs.
    pub effects: Vec<Effect>,
    /// One plain sentence stating what the block does.
    pub summary: String,
}

impl Block {
    /// Declare a block.
    pub fn new(id: &str, summary: &str) -> Self {
        Block {
            id: BlockId::new(id),
            inputs: Vec::new(),
            outputs: Vec::new(),
            effects: Vec::new(),
            summary: summary.to_string(),
        }
    }

    /// The body of this block, as manifest derivation reads it.
    pub fn body(&self) -> BlockBody<'_> {
        BlockBody {
            inputs: &self.inputs,
            outputs: &self.outputs,
            effects: &self.effects,
        }
    }

    /// The blocks this block runs, each with its location.
    pub fn block_refs(&self, path: &Path) -> Vec<(BlockId, Location)> {
        self.effects
            .iter()
            .enumerate()
            .filter_map(|(position, effect)| match effect {
                Effect::RunBlock { block, .. } => Some((
                    block.clone(),
                    path.field("effects")
                        .index(position)
                        .field("block")
                        .location(),
                )),
                _ => None,
            })
            .collect()
    }

    /// The effect sequence this block runs under the supplied arguments.
    ///
    /// The result holds the same steps the subgraph held, with every input
    /// name replaced by the value the caller supplied.
    pub fn inline(
        &self,
        arguments: &[(ParamName, Expr)],
        path: &Path,
    ) -> Result<Vec<Effect>, Diagnostic> {
        let replacements = self.bindings(arguments, path)?;
        Ok(self
            .effects
            .iter()
            .map(|effect| effect.substitute(&replacements))
            .collect())
    }

    /// The output values this block produces under the supplied arguments.
    pub fn inline_outputs(
        &self,
        arguments: &[(ParamName, Expr)],
        path: &Path,
    ) -> Result<Vec<(ParamName, Expr)>, Diagnostic> {
        let replacements = self.bindings(arguments, path)?;
        Ok(self
            .outputs
            .iter()
            .map(|output| (output.name.clone(), output.value.substitute(&replacements)))
            .collect())
    }

    fn bindings(
        &self,
        arguments: &[(ParamName, Expr)],
        path: &Path,
    ) -> Result<BTreeMap<ParamName, Expr>, Diagnostic> {
        let mut replacements: BTreeMap<ParamName, Expr> = BTreeMap::new();
        for input in &self.inputs {
            let supplied = arguments
                .iter()
                .find(|(name, _)| name == &input.name)
                .map(|(_, value)| value.clone())
                .ok_or_else(|| {
                    Diagnostic::at(
                        DiagnosticCode::MissingValue,
                        path.location(),
                        format!(
                            "The call to `{}` leaves out the input `{}`.",
                            self.id, input.name
                        ),
                        format!("Supply a value for `{}`.", input.name),
                    )
                })?;
            replacements.insert(input.name.clone(), supplied);
        }
        Ok(replacements)
    }
}

/// An index over the declared blocks.
#[derive(Clone, Debug, Default)]
pub struct BlockLibrary<'a> {
    entries: BTreeMap<&'a str, &'a Block>,
}

impl<'a> BlockLibrary<'a> {
    /// Build a library over a slice of blocks.
    pub fn build(blocks: &'a [Block]) -> Self {
        let mut entries: BTreeMap<&'a str, &'a Block> = BTreeMap::new();
        for block in blocks {
            entries.entry(block.id.as_str()).or_insert(block);
        }
        BlockLibrary { entries }
    }

    /// Look up a block.
    pub fn get(&self, id: &BlockId) -> Option<&'a Block> {
        self.entries.get(id.as_str()).copied()
    }

    /// Report whether a block exists.
    pub fn contains(&self, id: &BlockId) -> bool {
        self.entries.contains_key(id.as_str())
    }

    /// The block names, in sorted order.
    pub fn names(&self) -> Vec<&str> {
        self.entries.keys().copied().collect()
    }

    /// Count the blocks.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Report whether the library holds zero blocks.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Raise a dangling reference diagnostic for a block the Blueprint
    /// lacks.
    pub fn missing(&self, id: &BlockId, location: Location) -> Diagnostic {
        let listed = self.names().join(", ");
        Diagnostic::at(
            DiagnosticCode::DanglingReference,
            location,
            format!("The entry names the block `{id}`, which the Blueprint lacks."),
            if listed.is_empty() {
                format!("Declare a block named `{id}` in the composition library.")
            } else {
                format!("Point the entry at one of these blocks: {listed}.")
            },
        )
    }
}

impl BlockSource for BlockLibrary<'_> {
    fn block(&self, id: &BlockId) -> Option<BlockBody<'_>> {
        self.get(id).map(|block| block.body())
    }
}

/// Replace every block call inside an effect sequence with the steps the
/// block runs.
///
/// The result holds primitive steps alone. Manifest derivation over the
/// result matches manifest derivation over the original.
pub fn inline_effects(
    effects: &[Effect],
    blocks: &BlockLibrary,
    depth: u32,
    path: &Path,
) -> Result<Vec<Effect>, Diagnostic> {
    if depth > MAX_COMPOSITION_DEPTH {
        return Err(Diagnostic::at(
            DiagnosticCode::BoundExceeded,
            path.location(),
            format!("The blocks nest deeper than {MAX_COMPOSITION_DEPTH} levels."),
            "Flatten one of the blocks so the nesting stays inside the limit.".to_string(),
        ));
    }
    let mut flattened = Vec::new();
    for (position, effect) in effects.iter().enumerate() {
        let here = path.index(position);
        match effect {
            Effect::RunBlock { block, arguments } => {
                let found = blocks
                    .get(block)
                    .ok_or_else(|| blocks.missing(block, here.field("block").location()))?;
                let inner = found.inline(arguments, &here)?;
                flattened.extend(inline_effects(&inner, blocks, depth + 1, &here)?);
            }
            other => flattened.push(other.clone()),
        }
    }
    Ok(flattened)
}

/// The action with every block call replaced by the steps the block runs.
pub fn inline_action(
    action: &Action,
    blocks: &BlockLibrary,
    path: &Path,
) -> Result<Action, Diagnostic> {
    let mut flattened = action.clone();
    flattened.effects = inline_effects(&action.effects, blocks, 0, &path.field("effects"))?;
    flattened.compensation =
        inline_effects(&action.compensation, blocks, 0, &path.field("compensation"))?;
    Ok(flattened)
}

/// Check the whole composition library.
///
/// The pass reports repeated names, inputs and outputs that share a name,
/// output values that fail the type check, blocks the Blueprint lacks and
/// every loop in the call graph.
pub fn validate_blocks(
    blocks: &[Block],
    types: &TypeRegistry,
    collections: &CollectionRegistry,
    path: &Path,
) -> Report {
    let library = BlockLibrary::build(blocks);
    let mut report = Report::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for (position, block) in blocks.iter().enumerate() {
        let here = path.index(position);
        report.absorb(block.id.validate(&here.field("id")));
        if !seen.insert(block.id.as_str()) {
            report.push(Diagnostic::at(
                DiagnosticCode::DuplicateIdentifier,
                here.field("id").location(),
                format!("Two blocks share the name `{}`.", block.id),
                format!("Rename one of the two blocks away from `{}`.", block.id),
            ));
        }
        if block.summary.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::MissingValue,
                here.field("summary").location(),
                format!("The block `{}` states nothing.", block.id),
                "Write one plain sentence stating what the block does.".to_string(),
            ));
        }

        let mut ctx = TypeContext::new(types);
        let mut names: BTreeSet<&str> = BTreeSet::new();
        for (index, input) in block.inputs.iter().enumerate() {
            let input_path = here.field("inputs").index(index);
            report.absorb(input.name.validate(&input_path.field("name")));
            report.absorb(input.ty.validate(types, &input_path.field("ty")));
            if !names.insert(input.name.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateName,
                    input_path.field("name").location(),
                    format!(
                        "The block `{}` declares the input `{}` twice.",
                        block.id, input.name
                    ),
                    format!("Rename one of the two `{}` inputs.", input.name),
                ));
            }
            ctx.bind(&input.name, input.ty.clone());
        }

        for (index, output) in block.outputs.iter().enumerate() {
            let output_path = here.field("outputs").index(index);
            report.absorb(output.name.validate(&output_path.field("name")));
            report.absorb(output.ty.validate(types, &output_path.field("ty")));
            if !names.insert(output.name.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateName,
                    output_path.field("name").location(),
                    format!(
                        "The block `{}` uses the name `{}` for an input and an output.",
                        block.id, output.name
                    ),
                    format!("Rename the output `{}`.", output.name),
                ));
            }
            match output.value.check(&ctx, &output_path.field("value")) {
                Ok(found) => {
                    if found != output.ty {
                        report.push(Diagnostic::at(
                            DiagnosticCode::TypeMismatch,
                            output_path.field("value").location(),
                            format!(
                                "The output `{}` carries {} and declares {}.",
                                output.name,
                                found.describe(),
                                output.ty.describe()
                            ),
                            format!("Give the output the type {}.", found.describe()),
                        ));
                    }
                }
                Err(diagnostic) => report.push(diagnostic),
            }
        }

        for (index, effect) in block.effects.iter().enumerate() {
            let effect_path = here.field("effects").index(index);
            if let Some((collection, _, _)) = effect.access() {
                if !collections.contains(collection) {
                    report.push(
                        collections.missing(collection, effect_path.field("collection").location()),
                    );
                }
            }
        }
        if block.effects.is_empty() && block.outputs.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::SectionEmpty,
                here.field("effects").location(),
                format!(
                    "The block `{}` runs zero steps and returns nothing.",
                    block.id
                ),
                "Add at least one step, or declare at least one output.".to_string(),
            ));
        }

        let targets = edges.entry(block.id.as_str().to_string()).or_default();
        for (called, location) in block.block_refs(&here) {
            if library.contains(&called) {
                targets.push(called.as_str().to_string());
            } else {
                report.push(library.missing(&called, location));
            }
        }
    }

    if let Some(cycle) = find_cycle(&edges) {
        let start = cycle.first().cloned().unwrap_or_default();
        let position = blocks
            .iter()
            .position(|block| block.id.as_str() == start)
            .unwrap_or(0);
        report.push(Diagnostic::at(
            DiagnosticCode::CycleDetected,
            path.index(position).field("effects").location(),
            format!("The composition closes a loop: {}.", describe_cycle(&cycle)),
            "Remove one call on the loop so the composition stays one way.".to_string(),
        ));
    }

    report
}

/// The type a block output carries, for the interface layer palette.
pub fn output_type(block: &Block, name: &ParamName) -> Option<TypeExpr> {
    block
        .outputs
        .iter()
        .find(|output| &output.name == name)
        .map(|output| output.ty.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        actions::{AccessManifest, Action, NoBlocks, TransactionClass},
        collections::{Collection, KeySelector, PurposeDeclaration},
        expression::BinaryOp,
        identities::{Identities, RoleDef},
        rules::Rules,
        types::{
            CollectionId, Decimal, FieldDef, FieldName, FieldPath, Identity, RecordDef, RoleId,
            Timestamp, TypeDef, TypeId, Value,
        },
    };
    use alloc::vec;

    fn types() -> Vec<TypeDef> {
        vec![TypeDef::Record(RecordDef {
            id: TypeId::new("ledger_entry"),
            fields: vec![
                FieldDef::new("reference", TypeExpr::Text, "The entry reference."),
                FieldDef::new("amount", TypeExpr::Decimal { scale: 2 }, "The amount."),
            ],
            summary: "One ledger entry.".to_string(),
        })]
    }

    fn collections() -> Vec<Collection> {
        let mut ledger = Collection::new(
            "ledger",
            "ledger_entry",
            "reference",
            "The deployment ledger.",
        );
        ledger.purpose = PurposeDeclaration::new("The ledger records amounts.", &["accounting"]);
        vec![ledger]
    }

    fn identities() -> Identities {
        Identities {
            units: Vec::new(),
            roles: vec![RoleDef::new(
                "treasurer",
                None,
                "Treasurer",
                "The treasurer records.",
            )],
            credentials: Vec::new(),
            delegations: Vec::new(),
        }
    }

    fn record_entry_effects() -> Vec<Effect> {
        vec![
            Effect::Read {
                collection: CollectionId::new("ledger"),
                key: KeySelector::Exact(Expr::bound("reference")),
                binding: ParamName::new("held"),
                element: TypeExpr::Record(TypeId::new("ledger_entry")),
            },
            Effect::Insert {
                collection: CollectionId::new("ledger"),
                key: KeySelector::Exact(Expr::bound("reference")),
                record: Expr::MakeRecord {
                    type_id: TypeId::new("ledger_entry"),
                    fields: vec![
                        (FieldName::new("reference"), Expr::bound("reference")),
                        (FieldName::new("amount"), Expr::bound("amount")),
                    ],
                },
            },
        ]
    }

    fn record_entry_block() -> Block {
        let mut block = Block::new("record_entry", "The block records one ledger entry.");
        block.inputs = vec![
            Param::new("reference", TypeExpr::Text, "The entry reference."),
            Param::new("amount", TypeExpr::Decimal { scale: 2 }, "The amount."),
        ];
        block.outputs = vec![BlockOutput::new(
            "doubled",
            TypeExpr::Decimal { scale: 2 },
            Expr::binary(BinaryOp::Add, Expr::bound("amount"), Expr::bound("amount")),
        )];
        block.effects = record_entry_effects();
        block
    }

    fn composed_action() -> Action {
        let mut action = Action::new("record_receipt", "The action records one receipt.");
        action.roles = vec![RoleId::new("treasurer")];
        action.class = TransactionClass::Financial;
        action.params = vec![
            Param::new("reference", TypeExpr::Text, "The entry reference."),
            Param::new("amount", TypeExpr::Decimal { scale: 2 }, "The amount."),
        ];
        action.effects = vec![Effect::RunBlock {
            block: BlockId::new("record_entry"),
            arguments: vec![
                (ParamName::new("reference"), Expr::bound("reference")),
                (ParamName::new("amount"), Expr::bound("amount")),
            ],
        }];
        action
    }

    fn inline_action_by_hand() -> Action {
        let mut action = composed_action();
        action.effects = record_entry_effects();
        action
    }

    fn arguments() -> BTreeMap<ParamName, Value> {
        let mut bound = BTreeMap::new();
        bound.insert(
            ParamName::new("reference"),
            Value::Text("LE-0001".to_string()),
        );
        bound.insert(
            ParamName::new("amount"),
            Value::Decimal(Decimal::new(2500, 2)),
        );
        bound
    }

    fn manifest(action: &Action, blocks: &dyn BlockSource) -> AccessManifest {
        action
            .derive_manifest(
                &arguments(),
                Identity([3u8; 32]),
                Timestamp::from_millis(1),
                blocks,
                &Path::root().field("actions").index(0),
            )
            .expect("the derivation runs")
    }

    fn run(blocks: &[Block]) -> Report {
        let defs = types();
        let (type_registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let held = collections();
        let (collection_registry, _) =
            CollectionRegistry::build(&held, &Path::root().field("collections"));
        validate_blocks(
            blocks,
            &type_registry,
            &collection_registry,
            &Path::root().field("blocks"),
        )
    }

    #[test]
    fn a_complete_block_validates() {
        let report = run(&[record_entry_block()]);
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn a_composed_block_behaves_like_the_subgraph_it_replaced() {
        let blocks = [record_entry_block()];
        let library = BlockLibrary::build(&blocks);
        let composed = manifest(&composed_action(), &library);
        let by_hand = manifest(&inline_action_by_hand(), &NoBlocks);
        assert_eq!(composed.reads, by_hand.reads);
        assert_eq!(composed.writes, by_hand.writes);
        assert_eq!(composed.class, by_hand.class);

        let flattened = inline_action(
            &composed_action(),
            &library,
            &Path::root().field("actions").index(0),
        )
        .expect("the inliner runs");
        assert_eq!(flattened.effects, inline_action_by_hand().effects);
        assert_eq!(manifest(&flattened, &NoBlocks), by_hand);
    }

    #[test]
    fn a_composed_action_type_checks_like_the_subgraph() {
        let defs = types();
        let (type_registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let held = collections();
        let (collection_registry, _) =
            CollectionRegistry::build(&held, &Path::root().field("collections"));
        let blocks = [record_entry_block()];
        let library = BlockLibrary::build(&blocks);
        let composed = composed_action().validate(
            &type_registry,
            &collection_registry,
            &identities(),
            &Rules::default(),
            &library,
            &Path::root().field("actions").index(0),
        );
        assert!(composed.is_clean(), "{composed}");
        let by_hand = inline_action_by_hand().validate(
            &type_registry,
            &collection_registry,
            &identities(),
            &Rules::default(),
            &NoBlocks,
            &Path::root().field("actions").index(0),
        );
        assert!(by_hand.is_clean(), "{by_hand}");
    }

    #[test]
    fn a_loop_in_the_composition_is_detected() {
        let mut first = Block::new("first_block", "The block calls the second block.");
        first.effects = vec![Effect::RunBlock {
            block: BlockId::new("second_block"),
            arguments: Vec::new(),
        }];
        let mut second = Block::new("second_block", "The block calls the first block.");
        second.effects = vec![Effect::RunBlock {
            block: BlockId::new("first_block"),
            arguments: Vec::new(),
        }];
        let report = run(&[first, second]);
        let found = report
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == DiagnosticCode::CycleDetected)
            .expect("the loop is reported");
        assert!(found.problem.contains("first_block"));
        assert!(found.problem.contains("->"));
        assert!(found.fix.contains("one way"));
    }

    #[test]
    fn a_block_reaching_itself_is_detected() {
        let mut block = Block::new("self_caller", "The block calls itself.");
        block.effects = vec![Effect::RunBlock {
            block: BlockId::new("self_caller"),
            arguments: Vec::new(),
        }];
        let report = run(&[block]);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::CycleDetected));
    }

    #[test]
    fn a_block_call_naming_an_absent_block_reports_its_location() {
        let mut block = Block::new("caller", "The block calls an absent block.");
        block.effects = vec![Effect::RunBlock {
            block: BlockId::new("absent_block"),
            arguments: Vec::new(),
        }];
        let report = run(&[block]);
        let found = report
            .find("blocks[0].effects[0].block")
            .expect("the diagnostic sits on the call");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn an_output_of_the_wrong_type_is_caught() {
        let mut block = record_entry_block();
        block.outputs[0].ty = TypeExpr::Text;
        let report = run(&[block]);
        let found = report
            .find("blocks[0].outputs[0].value")
            .expect("the diagnostic sits on the output");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
    }

    #[test]
    fn a_call_leaving_out_an_input_is_caught() {
        let mut action = composed_action();
        action.effects = vec![Effect::RunBlock {
            block: BlockId::new("record_entry"),
            arguments: vec![(ParamName::new("reference"), Expr::bound("reference"))],
        }];
        let defs = types();
        let (type_registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let held = collections();
        let (collection_registry, _) =
            CollectionRegistry::build(&held, &Path::root().field("collections"));
        let blocks = [record_entry_block()];
        let library = BlockLibrary::build(&blocks);
        let report = action.validate(
            &type_registry,
            &collection_registry,
            &identities(),
            &Rules::default(),
            &library,
            &Path::root().field("actions").index(0),
        );
        let found = report
            .find("actions[0].effects[0].arguments")
            .expect("the diagnostic sits on the arguments");
        assert_eq!(found.code, DiagnosticCode::MissingValue);
        assert!(found.fix.contains("amount"));
    }

    #[test]
    fn the_outputs_bind_for_the_later_effects() {
        let mut action = composed_action();
        action.effects.push(Effect::Update {
            collection: CollectionId::new("ledger"),
            key: KeySelector::Exact(Expr::bound("reference")),
            field: FieldPath::parse("amount"),
            value: Expr::bound("doubled"),
        });
        let defs = types();
        let (type_registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let held = collections();
        let (collection_registry, _) =
            CollectionRegistry::build(&held, &Path::root().field("collections"));
        let blocks = [record_entry_block()];
        let library = BlockLibrary::build(&blocks);
        let report = action.validate(
            &type_registry,
            &collection_registry,
            &identities(),
            &Rules::default(),
            &library,
            &Path::root().field("actions").index(0),
        );
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn the_inline_outputs_carry_the_supplied_arguments() {
        let block = record_entry_block();
        let outputs = block
            .inline_outputs(
                &[
                    (ParamName::new("reference"), Expr::text("LE-0001")),
                    (ParamName::new("amount"), Expr::decimal(2500, 2)),
                ],
                &Path::root().field("blocks").index(0),
            )
            .expect("the inliner runs");
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].0, ParamName::new("doubled"));
        assert_eq!(
            outputs[0].1.run(&crate::expression::Env::new(
                Identity::default(),
                Timestamp::from_millis(1)
            )),
            Ok(Value::Decimal(Decimal::new(5000, 2)))
        );
        assert_eq!(
            output_type(&block, &ParamName::new("doubled")),
            Some(TypeExpr::Decimal { scale: 2 })
        );
    }

    #[test]
    fn a_block_nesting_past_the_limit_stops_the_inliner() {
        let mut blocks = Vec::new();
        for level in 0..(MAX_COMPOSITION_DEPTH + 3) {
            let mut block = Block::new(
                &alloc::format!("level_{level}"),
                "The block calls the next level down.",
            );
            block.effects = vec![Effect::RunBlock {
                block: BlockId::new(&alloc::format!("level_{}", level + 1)),
                arguments: Vec::new(),
            }];
            blocks.push(block);
        }
        let mut last = Block::new(
            &alloc::format!("level_{}", MAX_COMPOSITION_DEPTH + 3),
            "The block runs one step.",
        );
        last.effects = vec![Effect::Delete {
            collection: CollectionId::new("ledger"),
            key: KeySelector::Full,
        }];
        blocks.push(last);
        let library = BlockLibrary::build(&blocks);
        let mut action = Action::new("deep_call", "The action calls a deep block.");
        action.roles = vec![RoleId::new("treasurer")];
        action.effects = vec![Effect::RunBlock {
            block: BlockId::new("level_0"),
            arguments: Vec::new(),
        }];
        let diagnostic = inline_action(&action, &library, &Path::root().field("actions").index(0))
            .expect_err("the depth limit stops the inliner");
        assert_eq!(diagnostic.code, DiagnosticCode::BoundExceeded);
        assert!(diagnostic.fix.contains("Flatten"));
    }

    #[test]
    fn the_library_reports_a_missing_block_with_the_fix() {
        let blocks = [record_entry_block()];
        let library = BlockLibrary::build(&blocks);
        assert_eq!(library.len(), 1);
        assert!(!library.is_empty());
        let diagnostic = library.missing(
            &BlockId::new("absent_block"),
            Path::root()
                .field("actions")
                .index(0)
                .field("effects")
                .index(0)
                .location(),
        );
        assert!(diagnostic.fix.contains("record_entry"));
    }

    #[test]
    fn the_block_round_trips_through_scale_and_json() {
        let block = record_entry_block();
        let bytes = block.encode();
        let back = Block::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(block, back);
        let text = serde_json::to_string(&block).expect("the block serialises");
        let parsed: Block = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(block, parsed);
    }
}

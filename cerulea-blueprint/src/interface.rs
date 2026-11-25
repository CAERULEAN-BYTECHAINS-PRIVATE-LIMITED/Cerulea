//! The interface section.
//!
//! The interface a person uses sits inside the Blueprint beside the data
//! model. One typed document carries the screens, the components placed on
//! them, the bindings those components read, the theme tokens, the
//! responsive breakpoints, the visibility rules and the translated strings.
//!
//! Cerulea generates this section from the data model. A person then edits
//! the generated result. Both paths write the same typed form.
//!
//! Every binding names an element the Blueprint already declares. A binding
//! naming an absent collection, field, action or workflow fails validation
//! with the exact location and the fix.

use crate::{
    actions::ActionRegistry,
    collections::CollectionRegistry,
    expression::{Expr, TypeContext},
    identities::Identities,
    permissions::ROW_BINDING,
    types::{
        ActionId, BreakpointId, CollectionId, ComponentId, EventId, FieldPath, ParamName,
        PropertyName, RoleId, ScreenId, StateId, TokenName, TypeExpr, TypeRegistry, ViewId,
        WorkflowId,
    },
    views::ViewRegistry,
    workflows::WorkflowRegistry,
    Diagnostic, DiagnosticCode, Path, Report,
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

/// The longest a translation key runs.
pub const MAX_STRING_KEY: usize = 160;

/// The named breakpoint every generated interface carries for a phone.
pub const PHONE_BREAKPOINT: &str = "phone";

/// The named breakpoint every generated interface carries for a tablet.
pub const TABLET_BREAKPOINT: &str = "tablet";

/// The named breakpoint every generated interface carries for a desktop.
pub const DESKTOP_BREAKPOINT: &str = "desktop";

/// A key naming one translated string.
///
/// A key reads as names joined by full stops, such as
/// `screen.parcels.list.title`. The key never reaches a reader. The
/// translation carries the sentence a reader sees.
#[derive(
    Clone,
    Debug,
    Default,
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
#[serde(transparent)]
pub struct StringKey(pub String);

impl StringKey {
    /// Build a key from text.
    pub fn new(text: &str) -> Self {
        StringKey(text.to_string())
    }

    /// Read the key as text.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Check the spelling of the key.
    pub fn validate(&self, path: &Path) -> Result<(), Diagnostic> {
        if self.0.is_empty() {
            return Err(Diagnostic::new(
                DiagnosticCode::MissingValue,
                path.clone(),
                "The screen carries an empty translation key.",
                "Give the string a key such as `screen.parcels.list.title`.",
            ));
        }
        if self.0.len() > MAX_STRING_KEY {
            return Err(Diagnostic::at(
                DiagnosticCode::ValueTooLong,
                path.location(),
                format!(
                    "The translation key `{}` runs past {MAX_STRING_KEY} characters.",
                    self.0
                ),
                format!("Shorten the key to {MAX_STRING_KEY} characters or fewer."),
            ));
        }
        for segment in self.0.split('.') {
            let opens = segment
                .chars()
                .next()
                .map(|first| first.is_ascii_lowercase())
                .unwrap_or(false);
            let held = segment
                .chars()
                .all(|held| held.is_ascii_lowercase() || held.is_ascii_digit() || held == '_');
            if !opens || !held {
                return Err(Diagnostic::at(
                    DiagnosticCode::InvalidIdentifier,
                    path.location(),
                    format!("The translation key `{}` holds the part `{segment}`.", self.0),
                    "Write each part with lower case letters, digits and underscores, joined by full stops.".to_string(),
                ));
            }
        }
        Ok(())
    }
}

impl core::fmt::Display for StringKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A language tag naming one translation file.
///
/// The tag reads as a language, with a region after a hyphen where one
/// applies. `en` and `en-in` both parse.
#[derive(
    Clone,
    Debug,
    Default,
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
#[serde(transparent)]
pub struct LanguageTag(pub String);

impl LanguageTag {
    /// Build a language tag from text.
    pub fn new(text: &str) -> Self {
        LanguageTag(text.to_string())
    }

    /// Read the tag as text.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Check the spelling of the tag.
    pub fn validate(&self, path: &Path) -> Result<(), Diagnostic> {
        let mut parts = self.0.split('-');
        let language = parts.next().unwrap_or("");
        let region = parts.next();
        let extra = parts.next();
        let language_held =
            language.len() >= 2 && language.chars().all(|held| held.is_ascii_lowercase());
        let region_held = match region {
            None => true,
            Some(held) => held.len() >= 2 && held.chars().all(|c| c.is_ascii_lowercase()),
        };
        if !language_held || !region_held || extra.is_some() {
            return Err(Diagnostic::at(
                DiagnosticCode::InvalidIdentifier,
                path.location(),
                format!("The language tag `{}` holds an unexpected shape.", self.0),
                "Write the tag as a language, with a region after a hyphen where one applies, such as `en` or `en-in`.".to_string(),
            ));
        }
        Ok(())
    }
}

impl core::fmt::Display for LanguageTag {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The group one component belongs to.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
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
pub enum ComponentGroup {
    /// Components that place other components.
    #[default]
    Layout,
    /// Components that show records.
    Data,
    /// Components that collect a value.
    Input,
    /// Components that show Blockchain evidence.
    Chain,
    /// Components that surround the working area.
    Chrome,
    /// Components that report a state.
    Feedback,
}

impl ComponentGroup {
    /// A word naming the group for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            ComponentGroup::Layout => "layout",
            ComponentGroup::Data => "data",
            ComponentGroup::Input => "input",
            ComponentGroup::Chain => "chain",
            ComponentGroup::Chrome => "chrome",
            ComponentGroup::Feedback => "feedback",
        }
    }
}

/// What a component binds to.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
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
pub enum BindingNeed {
    /// The component reads nothing from the model.
    #[default]
    Nothing,
    /// The component reads one collection.
    Collection,
    /// The component reads one field of one collection.
    Field,
    /// The component runs one action.
    Action,
    /// The component follows one workflow.
    Workflow,
}

/// One component of the Cerulea component set.
///
/// The set covers six groups. A generated screen holds components from this
/// set alone, so every screen renders with the components Cerulea ships.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
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
pub enum ComponentKind {
    /// The whole screen.
    #[default]
    Page,
    /// A titled band inside a page.
    Section,
    /// A column grid.
    Grid,
    /// A run of components along one axis.
    Stack,
    /// Two panes beside one another.
    Split,
    /// Panels selected one at a time by a tab strip.
    Tabs,
    /// Panels that open and close in place.
    Accordion,
    /// A panel over the screen that takes the focus.
    Modal,
    /// A panel that slides in from an edge.
    Drawer,
    /// Rows and columns of records.
    Table,
    /// Records one under another.
    List,
    /// Records as cards in a grid.
    CardGrid,
    /// One record with every readable field.
    DetailPanel,
    /// A measure drawn over a dimension.
    Chart,
    /// Events along a time axis.
    Timeline,
    /// Records placed on a map.
    Map,
    /// Records placed on a calendar.
    Calendar,
    /// Records in columns by workflow state.
    Kanban,
    /// A line of text.
    TextInput,
    /// A whole number.
    NumberInput,
    /// An exact decimal with a declared scale.
    DecimalInput,
    /// A truth value.
    BooleanInput,
    /// A moment in time.
    TimestampInput,
    /// An identity.
    IdentityInput,
    /// A hash written as hexadecimal.
    HashInput,
    /// Raw bytes written as hexadecimal.
    BytesInput,
    /// A pointer to a record in another collection.
    ReferenceInput,
    /// One alternative of an enumeration.
    EnumerationInput,
    /// One alternative of a sum type with the data that alternative carries.
    SumInput,
    /// Several values of one element type.
    ListInput,
    /// Key and value pairs.
    MapInput,
    /// A value that a person leaves out.
    OptionalInput,
    /// A file taken from the device.
    FileUpload,
    /// A signature drawn by hand.
    SignatureCapture,
    /// A photograph taken by the device camera.
    CameraCapture,
    /// A barcode read by the device camera.
    BarcodeScan,
    /// The inclusion proof of one record.
    ProofViewer,
    /// Every event written against one record.
    AuditTrail,
    /// The standing of one submitted transaction.
    TransactionStatus,
    /// A mark stating that a record verified.
    VerificationBadge,
    /// The difference between two revisions of one record.
    HistoryDiff,
    /// The list of screens a role reaches.
    Navigation,
    /// The masthead carrying the deployment name.
    Header,
    /// A search field over one collection.
    Search,
    /// Messages waiting for the person.
    Notifications,
    /// The signed in person and the way out.
    Profile,
    /// The language selector.
    LanguageSwitch,
    /// The state a screen holding zero records shows.
    EmptyState,
    /// The state a screen shows while it reads.
    LoadingState,
    /// The state a screen shows after a failed read.
    ErrorState,
    /// A question a person answers before a write runs.
    Confirmation,
    /// A short message that appears and clears itself.
    Toast,
}

impl ComponentKind {
    /// Every component of the set, in group order.
    pub fn every() -> &'static [ComponentKind] {
        use ComponentKind::*;
        &[
            Page,
            Section,
            Grid,
            Stack,
            Split,
            Tabs,
            Accordion,
            Modal,
            Drawer,
            Table,
            List,
            CardGrid,
            DetailPanel,
            Chart,
            Timeline,
            Map,
            Calendar,
            Kanban,
            TextInput,
            NumberInput,
            DecimalInput,
            BooleanInput,
            TimestampInput,
            IdentityInput,
            HashInput,
            BytesInput,
            ReferenceInput,
            EnumerationInput,
            SumInput,
            ListInput,
            MapInput,
            OptionalInput,
            FileUpload,
            SignatureCapture,
            CameraCapture,
            BarcodeScan,
            ProofViewer,
            AuditTrail,
            TransactionStatus,
            VerificationBadge,
            HistoryDiff,
            Navigation,
            Header,
            Search,
            Notifications,
            Profile,
            LanguageSwitch,
            EmptyState,
            LoadingState,
            ErrorState,
            Confirmation,
            Toast,
        ]
    }

    /// The group the component belongs to.
    pub fn group(&self) -> ComponentGroup {
        use ComponentKind::*;
        match self {
            Page | Section | Grid | Stack | Split | Tabs | Accordion | Modal | Drawer => {
                ComponentGroup::Layout
            }
            Table | List | CardGrid | DetailPanel | Chart | Timeline | Map | Calendar | Kanban => {
                ComponentGroup::Data
            }
            TextInput | NumberInput | DecimalInput | BooleanInput | TimestampInput
            | IdentityInput | HashInput | BytesInput | ReferenceInput | EnumerationInput
            | SumInput | ListInput | MapInput | OptionalInput | FileUpload | SignatureCapture
            | CameraCapture | BarcodeScan => ComponentGroup::Input,
            ProofViewer | AuditTrail | TransactionStatus | VerificationBadge | HistoryDiff => {
                ComponentGroup::Chain
            }
            Navigation | Header | Search | Notifications | Profile | LanguageSwitch => {
                ComponentGroup::Chrome
            }
            EmptyState | LoadingState | ErrorState | Confirmation | Toast => {
                ComponentGroup::Feedback
            }
        }
    }

    /// The name the interface layer writes for the component.
    pub fn label(&self) -> &'static str {
        use ComponentKind::*;
        match self {
            Page => "page",
            Section => "section",
            Grid => "grid",
            Stack => "stack",
            Split => "split",
            Tabs => "tabs",
            Accordion => "accordion",
            Modal => "modal",
            Drawer => "drawer",
            Table => "table",
            List => "list",
            CardGrid => "card_grid",
            DetailPanel => "detail_panel",
            Chart => "chart",
            Timeline => "timeline",
            Map => "map",
            Calendar => "calendar",
            Kanban => "kanban",
            TextInput => "text_input",
            NumberInput => "number_input",
            DecimalInput => "decimal_input",
            BooleanInput => "boolean_input",
            TimestampInput => "timestamp_input",
            IdentityInput => "identity_input",
            HashInput => "hash_input",
            BytesInput => "bytes_input",
            ReferenceInput => "reference_input",
            EnumerationInput => "enumeration_input",
            SumInput => "sum_input",
            ListInput => "list_input",
            MapInput => "map_input",
            OptionalInput => "optional_input",
            FileUpload => "file_upload",
            SignatureCapture => "signature_capture",
            CameraCapture => "camera_capture",
            BarcodeScan => "barcode_scan",
            ProofViewer => "proof_viewer",
            AuditTrail => "audit_trail",
            TransactionStatus => "transaction_status",
            VerificationBadge => "verification_badge",
            HistoryDiff => "history_diff",
            Navigation => "navigation",
            Header => "header",
            Search => "search",
            Notifications => "notifications",
            Profile => "profile",
            LanguageSwitch => "language_switch",
            EmptyState => "empty_state",
            LoadingState => "loading_state",
            ErrorState => "error_state",
            Confirmation => "confirmation",
            Toast => "toast",
        }
    }

    /// Report whether the component places other components inside itself.
    ///
    /// A layout component places any component. A data component places the
    /// field components that form its columns, its cells or its rows. Four
    /// input components wrap an inner input, one for the element of a list,
    /// one for the value of a map, one for the payload of a sum alternative
    /// and one for the value a person leaves out.
    pub fn holds_children(&self) -> bool {
        use ComponentKind::*;
        matches!(self.group(), ComponentGroup::Layout | ComponentGroup::Data)
            || matches!(self, ListInput | MapInput | OptionalInput | SumInput)
    }

    /// What the component binds to.
    pub fn binds(&self) -> BindingNeed {
        use ComponentKind::*;
        match self {
            Table | List | CardGrid | DetailPanel | Chart | Timeline | Map | Calendar | Search
            | AuditTrail | HistoryDiff | ProofViewer => BindingNeed::Collection,
            Kanban => BindingNeed::Workflow,
            TextInput | NumberInput | DecimalInput | BooleanInput | TimestampInput
            | IdentityInput | HashInput | BytesInput | ReferenceInput | EnumerationInput
            | SumInput | ListInput | MapInput | OptionalInput => BindingNeed::Field,
            // The four capture components attach evidence to the write the
            // screen runs rather than to one field of the record.
            TransactionStatus | Confirmation | FileUpload | SignatureCapture | CameraCapture
            | BarcodeScan => BindingNeed::Action,
            _ => BindingNeed::Nothing,
        }
    }

    /// The input component that collects a value of the given type.
    ///
    /// Every field of every declared type reaches one input component, so a
    /// generated form covers the whole record.
    pub fn for_type(ty: &TypeExpr) -> ComponentKind {
        match ty {
            TypeExpr::Text => ComponentKind::TextInput,
            TypeExpr::Integer => ComponentKind::NumberInput,
            TypeExpr::Decimal { .. } => ComponentKind::DecimalInput,
            TypeExpr::Bool => ComponentKind::BooleanInput,
            TypeExpr::Timestamp => ComponentKind::TimestampInput,
            TypeExpr::Identity => ComponentKind::IdentityInput,
            TypeExpr::Hash => ComponentKind::HashInput,
            TypeExpr::Bytes => ComponentKind::BytesInput,
            TypeExpr::Reference(_) => ComponentKind::ReferenceInput,
            TypeExpr::Enumeration(_) => ComponentKind::EnumerationInput,
            TypeExpr::Sum(_) => ComponentKind::SumInput,
            TypeExpr::List(_) => ComponentKind::ListInput,
            TypeExpr::Map { .. } => ComponentKind::MapInput,
            TypeExpr::Optional(_) => ComponentKind::OptionalInput,
            TypeExpr::Record(_) => ComponentKind::Section,
        }
    }
}

/// What one component reads out of the Blueprint.
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
pub enum Binding {
    /// Every record of one collection.
    Collection(CollectionId),
    /// One field of the record of one collection.
    Field {
        /// The collection holding the record.
        collection: CollectionId,
        /// The field inside the record.
        field: FieldPath,
    },
    /// One parameter of one action.
    Parameter {
        /// The action the parameter belongs to.
        action: ActionId,
        /// The parameter name.
        parameter: ParamName,
    },
    /// One action.
    Action(ActionId),
    /// One workflow.
    Workflow(WorkflowId),
    /// One declarative query.
    View(ViewId),
    /// One event type.
    Event(EventId),
}

impl Binding {
    /// A word naming the kind of binding.
    pub fn label(&self) -> &'static str {
        match self {
            Binding::Collection(_) => "collection",
            Binding::Field { .. } => "field",
            Binding::Parameter { .. } => "parameter",
            Binding::Action(_) => "action",
            Binding::Workflow(_) => "workflow",
            Binding::View(_) => "view",
            Binding::Event(_) => "event",
        }
    }

    /// The collection the binding reads, where the binding names one.
    pub fn collection(&self) -> Option<&CollectionId> {
        match self {
            Binding::Collection(id) => Some(id),
            Binding::Field { collection, .. } => Some(collection),
            _ => None,
        }
    }

    /// The binding written as one line for the interface layer.
    pub fn describe(&self) -> String {
        match self {
            Binding::Collection(id) => format!("collection {id}"),
            Binding::Field { collection, field } => format!("field {collection}.{field}"),
            Binding::Parameter { action, parameter } => {
                format!("parameter {action}.{parameter}")
            }
            Binding::Action(id) => format!("action {id}"),
            Binding::Workflow(id) => format!("workflow {id}"),
            Binding::View(id) => format!("view {id}"),
            Binding::Event(id) => format!("event {id}"),
        }
    }

    /// Check that the binding names elements the Blueprint declares.
    #[allow(clippy::too_many_arguments)]
    pub fn validate(
        &self,
        types: &TypeRegistry,
        collections: &CollectionRegistry,
        actions: &ActionRegistry,
        workflows: &WorkflowRegistry,
        views: &ViewRegistry,
        events: &BTreeSet<&str>,
        path: &Path,
    ) -> Report {
        let mut report = Report::new();
        match self {
            Binding::Collection(id) => {
                if !collections.contains(id) {
                    report.push(collections.missing(id, path.location()));
                }
            }
            Binding::Field { collection, field } => match collections.get(collection) {
                None => report.push(collections.missing(collection, path.location())),
                Some(found) => {
                    report.absorb(field.validate(&path.field("field")));
                    let record = TypeExpr::Record(found.record_type.clone());
                    if field.resolve(&record, types).is_none() {
                        report.push(Diagnostic::at(
                            DiagnosticCode::DanglingReference,
                            path.field("field").location(),
                            format!(
                                "The component binds to the field `{field}`, which `{collection}` lacks."
                            ),
                            format!(
                                "Bind to a field declared on the record type of `{collection}`."
                            ),
                        ));
                    }
                }
            },
            Binding::Parameter { action, parameter } => match actions.get(action) {
                None => report.push(actions.missing(action, path.location())),
                Some(found) => {
                    if found.param(parameter).is_none() {
                        report.push(Diagnostic::at(
                            DiagnosticCode::DanglingReference,
                            path.field("parameter").location(),
                            format!(
                                "The component binds to the parameter `{parameter}`, which the action `{action}` lacks."
                            ),
                            format!("Bind to a parameter the action `{action}` declares."),
                        ));
                    }
                }
            },
            Binding::Action(id) => {
                if !actions.contains(id) {
                    report.push(actions.missing(id, path.location()));
                }
            }
            Binding::Workflow(id) => {
                if !workflows.contains(id) {
                    report.push(workflows.missing(id, path.location()));
                }
            }
            Binding::View(id) => {
                if !views.contains(id) {
                    report.push(views.missing(id, path.location()));
                }
            }
            Binding::Event(id) => {
                if !events.contains(id.as_str()) {
                    let listed: Vec<&str> = events.iter().copied().collect();
                    report.push(Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        path.location(),
                        format!(
                            "The component binds to the event `{id}`, which the Blueprint lacks."
                        ),
                        if listed.is_empty() {
                            format!("Declare an event named `{id}` in the events section.")
                        } else {
                            format!("Bind to one of these events: {}.", listed.join(", "))
                        },
                    ));
                }
            }
        }
        report
    }
}

/// One rule deciding whether a component appears.
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
pub enum VisibilityRule {
    /// The component appears for the listed roles.
    ForRoles(Vec<RoleId>),
    /// The component appears while the record rests in one of the listed
    /// workflow states.
    InStates {
        /// The workflow the states belong to.
        workflow: WorkflowId,
        /// The states that show the component.
        states: Vec<StateId>,
    },
    /// The component appears while the expression holds. The record binds to
    /// the name `record`.
    WhenValue(Expr),
}

impl VisibilityRule {
    /// A word naming the kind of rule.
    pub fn label(&self) -> &'static str {
        match self {
            VisibilityRule::ForRoles(_) => "role",
            VisibilityRule::InStates { .. } => "workflow state",
            VisibilityRule::WhenValue(_) => "data value",
        }
    }
}

/// One change a breakpoint applies to a component.
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
pub struct BreakpointOverride {
    /// The breakpoint the change applies at.
    pub breakpoint: BreakpointId,
    /// Whether the component appears at this width.
    pub visible: bool,
    /// The count of grid columns the component covers.
    pub span: u8,
    /// The position the component takes among its siblings.
    pub order: u16,
}

impl BreakpointOverride {
    /// Build an override that keeps the component in place.
    pub fn new(breakpoint: &str, visible: bool, span: u8, order: u16) -> Self {
        BreakpointOverride {
            breakpoint: BreakpointId::new(breakpoint),
            visible,
            span,
            order,
        }
    }
}

/// One named property of a placed component.
///
/// A property carries a choice the model cannot infer. A sort direction. A
/// page size. The measure a chart draws. The name and the value are both
/// text, so the set grows without a change to the encoding.
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
pub struct Property {
    /// The name the renderer reads.
    pub name: PropertyName,
    /// The value the name carries.
    pub value: String,
}

impl Property {
    /// Build a property.
    pub fn new(name: &str, value: &str) -> Self {
        Property {
            name: PropertyName::new(name),
            value: value.to_string(),
        }
    }
}

/// One component placed on a screen.
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
pub struct ComponentNode {
    /// The name of this placement.
    pub id: ComponentId,
    /// The component placed.
    pub component: ComponentKind,
    /// What the component reads.
    pub binding: Option<Binding>,
    /// The translation key carrying the label a reader sees.
    pub label_key: Option<StringKey>,
    /// The rules deciding whether the component appears.
    pub visibility: Vec<VisibilityRule>,
    /// The per-breakpoint changes.
    pub overrides: Vec<BreakpointOverride>,
    /// The named properties the renderer reads.
    pub properties: Vec<Property>,
    /// The components placed inside this one.
    pub children: Vec<ComponentNode>,
}

impl ComponentNode {
    /// Place a component with no binding.
    pub fn new(id: &str, component: ComponentKind) -> Self {
        ComponentNode {
            id: ComponentId::new(id),
            component,
            binding: None,
            label_key: None,
            visibility: Vec::new(),
            overrides: Vec::new(),
            properties: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Place a component reading one binding.
    pub fn bound(id: &str, component: ComponentKind, binding: Binding) -> Self {
        let mut node = ComponentNode::new(id, component);
        node.binding = Some(binding);
        node
    }

    /// Give the component a label.
    pub fn labelled(mut self, key: &str) -> Self {
        self.label_key = Some(StringKey::new(key));
        self
    }

    /// Place components inside this one.
    pub fn holding(mut self, children: Vec<ComponentNode>) -> Self {
        self.children = children;
        self
    }

    /// Walk this component and every component under it.
    pub fn walk(&self, visit: &mut dyn FnMut(&ComponentNode)) {
        visit(self);
        for child in &self.children {
            child.walk(visit);
        }
    }

    /// Count this component and every component under it.
    pub fn count(&self) -> usize {
        let mut total = 0usize;
        self.walk(&mut |_| total += 1);
        total
    }
}

/// The reason a screen exists.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
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
pub enum ScreenKind {
    /// Every record of one collection.
    #[default]
    List,
    /// One record of one collection.
    Detail,
    /// A form that adds a record.
    Create,
    /// A form that changes a record already present.
    Edit,
    /// A queue of records waiting for a decision.
    Approval,
    /// A screen a person added by hand.
    Custom,
}

impl ScreenKind {
    /// A word naming the kind for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            ScreenKind::List => "list",
            ScreenKind::Detail => "detail",
            ScreenKind::Create => "create",
            ScreenKind::Edit => "edit",
            ScreenKind::Approval => "approval",
            ScreenKind::Custom => "custom",
        }
    }

    /// Report whether the screen writes state.
    pub fn writes(&self) -> bool {
        matches!(
            self,
            ScreenKind::Create | ScreenKind::Edit | ScreenKind::Approval
        )
    }
}

/// One screen of the generated application.
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
pub struct Screen {
    /// The name of the screen.
    pub id: ScreenId,
    /// The reason the screen exists.
    pub kind: ScreenKind,
    /// The address the screen answers on.
    pub route: String,
    /// The collection the screen reads, where the screen reads one.
    pub collection: Option<CollectionId>,
    /// The workflow the screen follows, where the screen follows one.
    pub workflow: Option<WorkflowId>,
    /// The action the screen runs, where the screen writes.
    pub action: Option<ActionId>,
    /// The roles that reach the screen.
    pub roles: Vec<RoleId>,
    /// The translation key carrying the title.
    pub title_key: StringKey,
    /// The components placed on the screen.
    pub components: Vec<ComponentNode>,
    /// Whether the screen captures records without a connection.
    pub offline_capture: bool,
}

impl Screen {
    /// Declare a screen.
    pub fn new(id: &str, kind: ScreenKind, route: &str, title_key: &str) -> Self {
        Screen {
            id: ScreenId::new(id),
            kind,
            route: route.to_string(),
            collection: None,
            workflow: None,
            action: None,
            roles: Vec::new(),
            title_key: StringKey::new(title_key),
            components: Vec::new(),
            offline_capture: false,
        }
    }

    /// Walk every component on the screen.
    pub fn walk(&self, visit: &mut dyn FnMut(&ComponentNode)) {
        for node in &self.components {
            node.walk(visit);
        }
    }

    /// Every translation key the screen names.
    pub fn string_keys(&self) -> Vec<StringKey> {
        let mut found = Vec::new();
        found.push(self.title_key.clone());
        self.walk(&mut |node| {
            if let Some(key) = &node.label_key {
                found.push(key.clone());
            }
        });
        found
    }

    /// Report whether the named role reaches the screen.
    pub fn reached_by(&self, role: &RoleId) -> bool {
        self.roles.iter().any(|held| held == role)
    }
}

/// One named theme token.
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
pub struct Token {
    /// The name the screens read.
    pub name: TokenName,
    /// The value the name carries.
    pub value: String,
}

impl Token {
    /// Build a token.
    pub fn new(name: &str, value: &str) -> Self {
        Token {
            name: TokenName::new(name),
            value: value.to_string(),
        }
    }
}

/// The colour token names every theme carries.
pub const REQUIRED_COLOUR_TOKENS: &[&str] = &[
    "brand_primary",
    "on_brand_primary",
    "brand_accent",
    "on_brand_accent",
    "brand_positive",
    "panel",
    "surface",
    "surface_raised",
    "text_primary",
    "text_secondary",
    "border",
    "warning",
    "critical",
    "focus_ring",
];

/// The type token names every theme carries.
pub const REQUIRED_TYPE_TOKENS: &[&str] = &[
    "family",
    "monospace_family",
    "size_display",
    "size_title",
    "size_heading",
    "size_subheading",
    "size_body",
    "size_supporting",
    "size_label",
];

/// The spacing token names every theme carries.
pub const REQUIRED_SPACING_TOKENS: &[&str] = &[
    "unit",
    "step_hair",
    "step_tight",
    "step_snug",
    "step_normal",
    "step_roomy",
    "step_wide",
    "step_section",
];

/// The theme token set the screens read.
///
/// One token set drives every surface. A customer theme replaces token
/// values. The screens stay as they are.
#[derive(
    Clone,
    Debug,
    Default,
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
pub struct Theme {
    /// The colour tokens the light theme reads.
    pub colour_light: Vec<Token>,
    /// The colour tokens dark mode reads.
    pub colour_dark: Vec<Token>,
    /// The type tokens.
    pub type_scale: Vec<Token>,
    /// The spacing tokens.
    pub spacing: Vec<Token>,
}

impl Theme {
    /// The Cerulea theme. Shared blue, Cerulea gold and Cerulea green.
    pub fn cerulea() -> Self {
        Theme {
            colour_light: alloc::vec![
                Token::new("brand_primary", "#004AAD"),
                Token::new("on_brand_primary", "#FFFFFF"),
                Token::new("brand_accent", "#EEC000"),
                Token::new("on_brand_accent", "#1A1A1A"),
                Token::new("brand_positive", "#30B700"),
                Token::new("panel", "#9BB7D4"),
                Token::new("surface", "#FFFFFF"),
                Token::new("surface_raised", "#F4F7FB"),
                Token::new("text_primary", "#12233A"),
                Token::new("text_secondary", "#45586F"),
                Token::new("border", "#C3D3E4"),
                Token::new("warning", "#A35B00"),
                Token::new("critical", "#B3261E"),
                Token::new("focus_ring", "#004AAD"),
            ],
            colour_dark: alloc::vec![
                Token::new("brand_primary", "#7FA9E6"),
                Token::new("on_brand_primary", "#06152B"),
                Token::new("brand_accent", "#EEC000"),
                Token::new("on_brand_accent", "#1A1A1A"),
                Token::new("brand_positive", "#69D34A"),
                Token::new("panel", "#1B2F48"),
                Token::new("surface", "#06152B"),
                Token::new("surface_raised", "#0F2440"),
                Token::new("text_primary", "#F2F6FB"),
                Token::new("text_secondary", "#B9C8DA"),
                Token::new("border", "#28425F"),
                Token::new("warning", "#F0B429"),
                Token::new("critical", "#FF8A80"),
                Token::new("focus_ring", "#7FA9E6"),
            ],
            type_scale: alloc::vec![
                Token::new("family", "Inter, system-ui, sans-serif"),
                Token::new("monospace_family", "ui-monospace, monospace"),
                Token::new("size_display", "40px"),
                Token::new("size_title", "30px"),
                Token::new("size_heading", "24px"),
                Token::new("size_subheading", "20px"),
                Token::new("size_body", "16px"),
                Token::new("size_supporting", "14px"),
                Token::new("size_label", "13px"),
            ],
            spacing: alloc::vec![
                Token::new("unit", "8px"),
                Token::new("step_hair", "2px"),
                Token::new("step_tight", "4px"),
                Token::new("step_snug", "8px"),
                Token::new("step_normal", "16px"),
                Token::new("step_roomy", "24px"),
                Token::new("step_wide", "32px"),
                Token::new("step_section", "48px"),
            ],
        }
    }

    /// Read one colour token of the light theme.
    pub fn light(&self, name: &str) -> Option<&str> {
        self.colour_light
            .iter()
            .find(|token| token.name.as_str() == name)
            .map(|token| token.value.as_str())
    }

    /// Check the theme.
    pub fn validate(&self, path: &Path) -> Report {
        let mut report = Report::new();
        check_tokens(
            &self.colour_light,
            REQUIRED_COLOUR_TOKENS,
            true,
            &path.field("colour_light"),
            &mut report,
        );
        check_tokens(
            &self.colour_dark,
            REQUIRED_COLOUR_TOKENS,
            true,
            &path.field("colour_dark"),
            &mut report,
        );
        check_tokens(
            &self.type_scale,
            REQUIRED_TYPE_TOKENS,
            false,
            &path.field("type_scale"),
            &mut report,
        );
        check_tokens(
            &self.spacing,
            REQUIRED_SPACING_TOKENS,
            false,
            &path.field("spacing"),
            &mut report,
        );
        report
    }
}

fn check_tokens(
    tokens: &[Token],
    required: &[&str],
    colour: bool,
    path: &Path,
    report: &mut Report,
) {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (position, token) in tokens.iter().enumerate() {
        let here = path.index(position);
        report.absorb(token.name.validate(&here.field("name")));
        if !seen.insert(token.name.as_str()) {
            report.push(Diagnostic::at(
                DiagnosticCode::DuplicateName,
                here.field("name").location(),
                format!("The theme carries the token `{}` twice.", token.name),
                format!("Remove one of the two `{}` tokens.", token.name),
            ));
        }
        if token.value.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::MissingValue,
                here.field("value").location(),
                format!("The token `{}` carries an empty value.", token.name),
                "Give the token a value.".to_string(),
            ));
        }
        if colour && !is_hex_colour(&token.value) {
            report.push(Diagnostic::at(
                DiagnosticCode::TypeMismatch,
                here.field("value").location(),
                format!(
                    "The colour token `{}` carries `{}`.",
                    token.name, token.value
                ),
                "Write the colour as six hexadecimal digits after a hash, such as `#004AAD`."
                    .to_string(),
            ));
        }
    }
    for name in required {
        if !seen.contains(name) {
            report.push(Diagnostic::at(
                DiagnosticCode::MissingValue,
                path.location(),
                format!("The theme leaves out the token `{name}`."),
                format!("Add a token named `{name}` so every screen reads a value."),
            ));
        }
    }
}

fn is_hex_colour(value: &str) -> bool {
    let mut characters = value.chars();
    if characters.next() != Some('#') {
        return false;
    }
    let rest: Vec<char> = characters.collect();
    rest.len() == 6 && rest.iter().all(|held| held.is_ascii_hexdigit())
}

/// One responsive breakpoint.
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
pub struct Breakpoint {
    /// The name the overrides point at.
    pub id: BreakpointId,
    /// The smallest viewport width the breakpoint covers, in pixels.
    pub min_width: u16,
    /// The count of grid columns at this width.
    pub columns: u8,
    /// The translation key carrying the name a reader sees.
    pub label_key: StringKey,
}

impl Breakpoint {
    /// Build a breakpoint.
    pub fn new(id: &str, min_width: u16, columns: u8, label_key: &str) -> Self {
        Breakpoint {
            id: BreakpointId::new(id),
            min_width,
            columns,
            label_key: StringKey::new(label_key),
        }
    }

    /// The three widths every generated interface answers on.
    pub fn standard() -> Vec<Breakpoint> {
        alloc::vec![
            Breakpoint::new(PHONE_BREAKPOINT, 0, 4, "breakpoint.phone"),
            Breakpoint::new(TABLET_BREAKPOINT, 600, 8, "breakpoint.tablet"),
            Breakpoint::new(DESKTOP_BREAKPOINT, 1024, 12, "breakpoint.desktop"),
        ]
    }
}

/// One translation file.
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
pub struct Translation {
    /// The language the file carries.
    pub language: LanguageTag,
    /// The key and sentence pairs, sorted by key.
    pub entries: Vec<(StringKey, String)>,
}

impl Translation {
    /// Build a translation file.
    pub fn new(language: &str, entries: Vec<(StringKey, String)>) -> Self {
        let mut held = entries;
        held.sort();
        Translation {
            language: LanguageTag::new(language),
            entries: held,
        }
    }

    /// Read one sentence.
    pub fn get(&self, key: &StringKey) -> Option<&str> {
        self.entries
            .iter()
            .find(|(held, _)| held == key)
            .map(|(_, sentence)| sentence.as_str())
    }

    /// The keys the file carries.
    pub fn keys(&self) -> BTreeSet<&str> {
        self.entries.iter().map(|(key, _)| key.as_str()).collect()
    }
}

/// How a conflict between an offline record and the stored record resolves.
#[derive(
    Clone,
    Debug,
    Default,
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
pub enum ConflictResolution {
    /// The record already stored stays.
    #[default]
    KeepStored,
    /// The record captured offline replaces the stored one.
    TakeCaptured,
    /// The record carrying the later timestamp in the named field stays.
    LaterTimestamp {
        /// The timestamp field the comparison reads.
        field: FieldPath,
    },
    /// The record carrying the larger value in the named field stays.
    LargerValue {
        /// The field the comparison reads.
        field: FieldPath,
    },
    /// The listed fields merge from the captured record. The rest stay.
    MergeFields {
        /// The fields the captured record supplies.
        fields: Vec<FieldPath>,
    },
    /// A person decides. The captured record waits in a queue.
    AskAPerson,
}

impl ConflictResolution {
    /// A word naming the rule for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            ConflictResolution::KeepStored => "keep_stored",
            ConflictResolution::TakeCaptured => "take_captured",
            ConflictResolution::LaterTimestamp { .. } => "later_timestamp",
            ConflictResolution::LargerValue { .. } => "larger_value",
            ConflictResolution::MergeFields { .. } => "merge_fields",
            ConflictResolution::AskAPerson => "ask_a_person",
        }
    }
}

/// One declared conflict rule.
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
pub struct ConflictRule {
    /// The collection the rule covers.
    pub collection: CollectionId,
    /// The way the conflict resolves.
    pub resolution: ConflictResolution,
    /// One plain sentence stating why the rule reads this way.
    pub statement: String,
}

/// The interface section of a Blueprint.
#[derive(
    Clone,
    Debug,
    Default,
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
pub struct Interface {
    /// The screens.
    pub screens: Vec<Screen>,
    /// The theme tokens every screen reads.
    pub theme: Theme,
    /// The responsive breakpoints.
    pub breakpoints: Vec<Breakpoint>,
    /// The translation files.
    pub translations: Vec<Translation>,
    /// The rules that resolve a conflict after an offline capture.
    pub conflict_rules: Vec<ConflictRule>,
    /// The language a reader sees before choosing one.
    pub default_language: LanguageTag,
}

impl Interface {
    /// Build an interface carrying the Cerulea theme and the three widths.
    pub fn new() -> Self {
        Interface {
            screens: Vec::new(),
            theme: Theme::cerulea(),
            breakpoints: Breakpoint::standard(),
            translations: Vec::new(),
            conflict_rules: Vec::new(),
            default_language: LanguageTag::new("en"),
        }
    }

    /// The screens one role reaches.
    pub fn screens_for(&self, role: &RoleId) -> Vec<&Screen> {
        self.screens
            .iter()
            .filter(|screen| screen.reached_by(role))
            .collect()
    }

    /// Every translation key the screens name, sorted and without repeats.
    pub fn string_keys(&self) -> Vec<StringKey> {
        let mut found: BTreeSet<StringKey> = BTreeSet::new();
        for screen in &self.screens {
            for key in screen.string_keys() {
                found.insert(key);
            }
        }
        for breakpoint in &self.breakpoints {
            found.insert(breakpoint.label_key.clone());
        }
        found.into_iter().collect()
    }

    /// Read the conflict rule covering one collection.
    pub fn conflict_rule(&self, collection: &CollectionId) -> Option<&ConflictRule> {
        self.conflict_rules
            .iter()
            .find(|rule| &rule.collection == collection)
    }

    /// Check the whole interface section.
    #[allow(clippy::too_many_arguments)]
    pub fn validate(
        &self,
        types: &TypeRegistry,
        collections: &CollectionRegistry,
        identities: &Identities,
        actions: &ActionRegistry,
        workflows: &WorkflowRegistry,
        views: &ViewRegistry,
        events: &BTreeSet<&str>,
        path: &Path,
    ) -> Report {
        let mut report = Report::new();
        report.absorb(
            self.default_language
                .validate(&path.field("default_language")),
        );
        report.merge(self.theme.validate(&path.field("theme")));

        let mut widths: BTreeSet<&str> = BTreeSet::new();
        let breakpoints_path = path.field("breakpoints");
        for (position, breakpoint) in self.breakpoints.iter().enumerate() {
            let here = breakpoints_path.index(position);
            report.absorb(breakpoint.id.validate(&here.field("id")));
            report.absorb(breakpoint.label_key.validate(&here.field("label_key")));
            if !widths.insert(breakpoint.id.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("id").location(),
                    format!("Two breakpoints share the name `{}`.", breakpoint.id),
                    format!("Rename one of the two `{}` breakpoints.", breakpoint.id),
                ));
            }
            if breakpoint.columns == 0 {
                report.push(Diagnostic::at(
                    DiagnosticCode::ValueOutOfRange,
                    here.field("columns").location(),
                    format!("The breakpoint `{}` carries zero columns.", breakpoint.id),
                    "Set the column count above zero.".to_string(),
                ));
            }
        }
        if self.breakpoints.is_empty() && !self.screens.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::SectionEmpty,
                breakpoints_path.location(),
                "The interface carries screens and zero breakpoints.".to_string(),
                "Declare the phone, tablet and desktop breakpoints so every screen answers at every width.".to_string(),
            ));
        }

        let screens_path = path.field("screens");
        let mut names: BTreeSet<&str> = BTreeSet::new();
        let mut routes: BTreeMap<&str, &str> = BTreeMap::new();
        for (position, screen) in self.screens.iter().enumerate() {
            let here = screens_path.index(position);
            report.absorb(screen.id.validate(&here.field("id")));
            report.absorb(screen.title_key.validate(&here.field("title_key")));
            if !names.insert(screen.id.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("id").location(),
                    format!("Two screens share the name `{}`.", screen.id),
                    format!("Rename one of the two `{}` screens.", screen.id),
                ));
            }
            if screen.route.is_empty() || !screen.route.starts_with('/') {
                report.push(Diagnostic::at(
                    DiagnosticCode::InvalidIdentifier,
                    here.field("route").location(),
                    format!(
                        "The screen `{}` answers on the address `{}`.",
                        screen.id, screen.route
                    ),
                    "Write the address with a leading slash, such as `/parcels`.".to_string(),
                ));
            } else if let Some(held) = routes.insert(screen.route.as_str(), screen.id.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("route").location(),
                    format!(
                        "The screens `{held}` and `{}` answer on `{}`.",
                        screen.id, screen.route
                    ),
                    "Give each screen its own address.".to_string(),
                ));
            }
            if screen.roles.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::SectionEmpty,
                    here.field("roles").location(),
                    format!("The screen `{}` names zero roles.", screen.id),
                    "Name the roles that reach the screen, so the permission matrix decides who sees it.".to_string(),
                ));
            }
            for (at, role) in screen.roles.iter().enumerate() {
                let role_path = here.field("roles").index(at);
                report.absorb(role.validate(&role_path));
                if !identities.has_role(role) {
                    report.push(identities.missing_role(role, role_path.location()));
                }
            }
            if let Some(collection) = &screen.collection {
                if !collections.contains(collection) {
                    report
                        .push(collections.missing(collection, here.field("collection").location()));
                }
            }
            if let Some(workflow) = &screen.workflow {
                if !workflows.contains(workflow) {
                    report.push(workflows.missing(workflow, here.field("workflow").location()));
                }
            }
            if let Some(action) = &screen.action {
                if !actions.contains(action) {
                    report.push(actions.missing(action, here.field("action").location()));
                }
            }
            if screen.kind.writes() && screen.action.is_none() {
                report.push(Diagnostic::at(
                    DiagnosticCode::MissingValue,
                    here.field("action").location(),
                    format!(
                        "The {} screen `{}` writes state and names no action.",
                        screen.kind.label(),
                        screen.id
                    ),
                    "Name the action the screen runs, so the write passes through the permission matrix.".to_string(),
                ));
            }
            if screen.components.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::SectionEmpty,
                    here.field("components").location(),
                    format!("The screen `{}` places zero components.", screen.id),
                    "Place at least one component on the screen.".to_string(),
                ));
            }

            let mut placements: BTreeSet<String> = BTreeSet::new();
            for (at, node) in screen.components.iter().enumerate() {
                Interface::check_node(
                    node,
                    screen,
                    types,
                    collections,
                    identities,
                    actions,
                    workflows,
                    views,
                    events,
                    &widths,
                    &mut placements,
                    &here.field("components").index(at),
                    &mut report,
                );
            }
        }

        let translations_path = path.field("translations");
        let declared: BTreeSet<StringKey> = self.string_keys().into_iter().collect();
        let mut languages: BTreeSet<&str> = BTreeSet::new();
        for (position, translation) in self.translations.iter().enumerate() {
            let here = translations_path.index(position);
            report.absorb(translation.language.validate(&here.field("language")));
            if !languages.insert(translation.language.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("language").location(),
                    format!(
                        "Two translation files carry the language `{}`.",
                        translation.language
                    ),
                    "Keep one file for each language.".to_string(),
                ));
            }
            let held = translation.keys();
            for key in &declared {
                if !held.contains(key.as_str()) {
                    report.push(Diagnostic::at(
                        DiagnosticCode::MissingValue,
                        here.field("entries").location(),
                        format!(
                            "The `{}` translation leaves out the key `{key}`.",
                            translation.language
                        ),
                        format!("Add a sentence for `{key}` so every reader sees words."),
                    ));
                }
            }
        }
        if !self.screens.is_empty()
            && !languages.contains(self.default_language.as_str())
            && !self.translations.is_empty()
        {
            report.push(Diagnostic::at(
                DiagnosticCode::MissingValue,
                translations_path.location(),
                format!(
                    "The interface opens in `{}` and carries no file for that language.",
                    self.default_language
                ),
                format!(
                    "Add the `{}` translation file, or open in a language a file carries.",
                    self.default_language
                ),
            ));
        }

        let rules_path = path.field("conflict_rules");
        let mut covered: BTreeSet<&str> = BTreeSet::new();
        for (position, rule) in self.conflict_rules.iter().enumerate() {
            let here = rules_path.index(position);
            let collection = collections.get(&rule.collection);
            if collection.is_none() {
                report.push(
                    collections.missing(&rule.collection, here.field("collection").location()),
                );
            }
            if !covered.insert(rule.collection.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("collection").location(),
                    format!(
                        "Two conflict rules cover the collection `{}`.",
                        rule.collection
                    ),
                    "Keep one rule for each collection.".to_string(),
                ));
            }
            if rule.statement.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::MissingValue,
                    here.field("statement").location(),
                    format!("The conflict rule on `{}` states nothing.", rule.collection),
                    "Write one plain sentence stating why the rule reads this way.".to_string(),
                ));
            }
            let named = match &rule.resolution {
                ConflictResolution::LaterTimestamp { field }
                | ConflictResolution::LargerValue { field } => alloc::vec![field.clone()],
                ConflictResolution::MergeFields { fields } => fields.clone(),
                _ => Vec::new(),
            };
            if let Some(found) = collection {
                let record = TypeExpr::Record(found.record_type.clone());
                for field in &named {
                    report.absorb(field.validate(&here.field("resolution")));
                    match field.resolve(&record, types) {
                        None => report.push(Diagnostic::at(
                            DiagnosticCode::DanglingReference,
                            here.field("resolution").location(),
                            format!(
                                "The conflict rule reads the field `{field}`, which `{}` lacks.",
                                rule.collection
                            ),
                            format!(
                                "Read a field declared on the record type of `{}`.",
                                rule.collection
                            ),
                        )),
                        Some(ty) => {
                            if matches!(rule.resolution, ConflictResolution::LaterTimestamp { .. })
                                && ty != TypeExpr::Timestamp
                            {
                                report.push(Diagnostic::at(
                                    DiagnosticCode::TypeMismatch,
                                    here.field("resolution").location(),
                                    format!(
                                        "The later timestamp rule reads `{field}`, which carries {}.",
                                        ty.describe()
                                    ),
                                    "Read a timestamp field, or choose another rule.".to_string(),
                                ));
                            }
                        }
                    }
                }
            }
        }

        report
    }

    #[allow(clippy::too_many_arguments)]
    fn check_node(
        node: &ComponentNode,
        screen: &Screen,
        types: &TypeRegistry,
        collections: &CollectionRegistry,
        identities: &Identities,
        actions: &ActionRegistry,
        workflows: &WorkflowRegistry,
        views: &ViewRegistry,
        events: &BTreeSet<&str>,
        widths: &BTreeSet<&str>,
        placements: &mut BTreeSet<String>,
        path: &Path,
        report: &mut Report,
    ) {
        report.absorb(node.id.validate(&path.field("id")));
        if let Some(key) = &node.label_key {
            report.absorb(key.validate(&path.field("label_key")));
        }

        if !placements.insert(node.id.as_str().to_string()) {
            report.push(Diagnostic::at(
                DiagnosticCode::DuplicateIdentifier,
                path.field("id").location(),
                format!(
                    "The screen `{}` places the name `{}` twice.",
                    screen.id, node.id
                ),
                format!("Rename one of the two `{}` placements.", node.id),
            ));
        }

        match (node.component.binds(), &node.binding) {
            (BindingNeed::Nothing, _) => {}
            (_, None) => report.push(Diagnostic::at(
                DiagnosticCode::MissingValue,
                path.field("binding").location(),
                format!(
                    "The {} component `{}` reads nothing.",
                    node.component.label(),
                    node.id
                ),
                format!(
                    "Bind the component to the {} it shows.",
                    describe_need(node.component.binds())
                ),
            )),
            (need, Some(binding)) => {
                report.merge(binding.validate(
                    types,
                    collections,
                    actions,
                    workflows,
                    views,
                    events,
                    &path.field("binding"),
                ));
                if !need_admits(need, binding) {
                    report.push(Diagnostic::at(
                        DiagnosticCode::TypeMismatch,
                        path.field("binding").location(),
                        format!(
                            "The {} component `{}` binds to a {}.",
                            node.component.label(),
                            node.id,
                            binding.label()
                        ),
                        format!(
                            "Bind the component to the {} it shows.",
                            describe_need(need)
                        ),
                    ));
                }
            }
        }

        if !node.children.is_empty() && !node.component.holds_children() {
            report.push(Diagnostic::at(
                DiagnosticCode::TypeMismatch,
                path.field("children").location(),
                format!(
                    "The {} component `{}` holds components inside itself.",
                    node.component.label(),
                    node.id
                ),
                "Place the components inside a page, a section, a grid, a stack, a split, a tab strip, an accordion, a modal or a drawer.".to_string(),
            ));
        }

        for (position, rule) in node.visibility.iter().enumerate() {
            let here = path.field("visibility").index(position);
            match rule {
                VisibilityRule::ForRoles(roles) => {
                    if roles.is_empty() {
                        report.push(Diagnostic::at(
                            DiagnosticCode::SectionEmpty,
                            here.location(),
                            format!("The role rule on `{}` names zero roles.", node.id),
                            "Name the roles that see the component.".to_string(),
                        ));
                    }
                    for (at, role) in roles.iter().enumerate() {
                        let role_path = here.index(at);
                        report.absorb(role.validate(&role_path));
                        if !identities.has_role(role) {
                            report.push(identities.missing_role(role, role_path.location()));
                        } else if !screen.reached_by(role) {
                            report.push(Diagnostic::at(
                                DiagnosticCode::DanglingReference,
                                role_path.location(),
                                format!(
                                    "The rule shows `{}` to the role `{role}`, which the screen `{}` leaves out.",
                                    node.id, screen.id
                                ),
                                format!(
                                    "Add `{role}` to the roles of the screen `{}`, or name a role the screen already carries.",
                                    screen.id
                                ),
                            ));
                        }
                    }
                }
                VisibilityRule::InStates { workflow, states } => match workflows.get(workflow) {
                    None => report.push(workflows.missing(workflow, here.location())),
                    Some(found) => {
                        if states.is_empty() {
                            report.push(Diagnostic::at(
                                DiagnosticCode::SectionEmpty,
                                here.location(),
                                format!("The state rule on `{}` names zero states.", node.id),
                                "Name the workflow states that show the component.".to_string(),
                            ));
                        }
                        for (at, state) in states.iter().enumerate() {
                            if !found.has_state(state) {
                                report.push(found.missing_state(state, here.index(at).location()));
                            }
                        }
                    }
                },
                VisibilityRule::WhenValue(expr) => {
                    let mut ctx = TypeContext::new(types);
                    if let Some(collection) = screen
                        .collection
                        .as_ref()
                        .and_then(|id| collections.get(id))
                    {
                        ctx.bind(
                            &ParamName::new(ROW_BINDING),
                            TypeExpr::Record(collection.record_type.clone()),
                        );
                    }
                    match expr.check(&ctx, &here) {
                        Ok(TypeExpr::Bool) => {}
                        Ok(other) => report.push(Diagnostic::at(
                            DiagnosticCode::TypeMismatch,
                            here.location(),
                            format!(
                                "The data rule on `{}` carries {}.",
                                node.id,
                                other.describe()
                            ),
                            "Give the rule a truth value.".to_string(),
                        )),
                        Err(diagnostic) => report.push(diagnostic),
                    }
                }
            }
        }

        let mut applied: BTreeSet<&str> = BTreeSet::new();
        for (position, held) in node.overrides.iter().enumerate() {
            let here = path.field("overrides").index(position);
            report.absorb(held.breakpoint.validate(&here.field("breakpoint")));
            if !widths.contains(held.breakpoint.as_str()) {
                let listed: Vec<&str> = widths.iter().copied().collect();
                report.push(Diagnostic::at(
                    DiagnosticCode::DanglingReference,
                    here.field("breakpoint").location(),
                    format!(
                        "The override names the breakpoint `{}`, which the interface lacks.",
                        held.breakpoint
                    ),
                    if listed.is_empty() {
                        format!("Declare a breakpoint named `{}`.", held.breakpoint)
                    } else {
                        format!(
                            "Point the override at one of these breakpoints: {}.",
                            listed.join(", ")
                        )
                    },
                ));
            }
            if !applied.insert(held.breakpoint.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("breakpoint").location(),
                    format!(
                        "The component `{}` carries two overrides at `{}`.",
                        node.id, held.breakpoint
                    ),
                    format!("Keep one override for `{}`.", held.breakpoint),
                ));
            }
            if held.span == 0 && held.visible {
                report.push(Diagnostic::at(
                    DiagnosticCode::ValueOutOfRange,
                    here.field("span").location(),
                    format!(
                        "The component `{}` appears at `{}` and covers zero columns.",
                        node.id, held.breakpoint
                    ),
                    "Set the column span above zero, or hide the component at this width."
                        .to_string(),
                ));
            }
        }

        let mut named: BTreeSet<&str> = BTreeSet::new();
        for (position, property) in node.properties.iter().enumerate() {
            let here = path.field("properties").index(position);
            report.absorb(property.name.validate(&here.field("name")));
            if !named.insert(property.name.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateName,
                    here.field("name").location(),
                    format!(
                        "The component `{}` carries the property `{}` twice.",
                        node.id, property.name
                    ),
                    format!("Keep one `{}` property.", property.name),
                ));
            }
            if property.value.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::MissingValue,
                    here.field("value").location(),
                    format!("The property `{}` carries an empty value.", property.name),
                    "Give the property a value, or remove it.".to_string(),
                ));
            }
        }

        for (position, child) in node.children.iter().enumerate() {
            Interface::check_node(
                child,
                screen,
                types,
                collections,
                identities,
                actions,
                workflows,
                views,
                events,
                widths,
                placements,
                &path.field("children").index(position),
                report,
            );
        }
    }
}

fn need_admits(need: BindingNeed, binding: &Binding) -> bool {
    match need {
        BindingNeed::Nothing => true,
        BindingNeed::Collection => matches!(
            binding,
            Binding::Collection(_) | Binding::View(_) | Binding::Event(_)
        ),
        BindingNeed::Field => matches!(binding, Binding::Field { .. } | Binding::Parameter { .. }),
        BindingNeed::Action => matches!(binding, Binding::Action(_)),
        BindingNeed::Workflow => matches!(binding, Binding::Workflow(_)),
    }
}

fn describe_need(need: BindingNeed) -> &'static str {
    match need {
        BindingNeed::Nothing => "model element",
        BindingNeed::Collection => "collection, view or event",
        BindingNeed::Field => "field or action parameter",
        BindingNeed::Action => "action",
        BindingNeed::Workflow => "workflow",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        actions::{Action, Effect},
        collections::{Collection, KeySelector, PurposeDeclaration},
        expression::BinaryOp,
        identities::RoleDef,
        types::{FieldDef, RecordDef, TypeDef, TypeId},
        views::View,
        workflows::{Transition, Workflow, WorkflowState},
    };
    use alloc::vec;

    fn types() -> Vec<TypeDef> {
        vec![TypeDef::Record(RecordDef {
            id: TypeId::new("parcel"),
            fields: vec![
                FieldDef::new("survey_number", TypeExpr::Text, "The survey number."),
                FieldDef::new("district", TypeExpr::Text, "The district."),
                FieldDef::new("area", TypeExpr::Decimal { scale: 4 }, "The area."),
                FieldDef::new("recorded_at", TypeExpr::Timestamp, "The recording time."),
            ],
            summary: "One land parcel.".to_string(),
        })]
    }

    fn collections() -> Vec<Collection> {
        let mut parcel =
            Collection::new("parcel", "parcel", "survey_number", "The parcel register.");
        parcel.purpose =
            PurposeDeclaration::new("The register records land holdings.", &["registration"]);
        vec![parcel]
    }

    fn identities() -> Identities {
        Identities {
            units: Vec::new(),
            roles: vec![
                RoleDef::new("registrar", None, "Registrar", "The registrar records."),
                RoleDef::new("holder", None, "Holder", "The holder reads."),
            ],
            credentials: Vec::new(),
            delegations: Vec::new(),
        }
    }

    fn actions() -> Vec<Action> {
        let mut action = Action::new("record_parcel", "Record one parcel.");
        action.roles = vec![RoleId::new("registrar")];
        action.effects = vec![Effect::Delete {
            collection: CollectionId::new("parcel"),
            key: KeySelector::Full,
        }];
        vec![action]
    }

    fn workflows() -> Vec<Workflow> {
        vec![Workflow {
            id: WorkflowId::new("parcel_life"),
            collection: CollectionId::new("parcel"),
            state_field: FieldPath::parse("district"),
            states: vec![
                WorkflowState::passing("draft", "Draft", "The parcel waits."),
                WorkflowState::resting("recorded", "Recorded", "The parcel rests."),
            ],
            initial: StateId::new("draft"),
            transitions: vec![Transition::new("record", "draft", "recorded")
                .run_by(&["registrar"])
                .carried_by("record_parcel")],
            timers: Vec::new(),
            deadlines: Vec::new(),
            cycles: crate::workflows::CyclePolicy::default(),
            summary: "The parcel moves from draft to recorded.".to_string(),
        }]
    }

    fn views() -> Vec<View> {
        Vec::new()
    }

    fn sample_screen() -> Screen {
        let mut screen = Screen::new(
            "parcel_list",
            ScreenKind::List,
            "/parcel",
            "screen.parcel.list.title",
        );
        screen.collection = Some(CollectionId::new("parcel"));
        screen.roles = vec![RoleId::new("registrar"), RoleId::new("holder")];
        screen.components = vec![
            ComponentNode::new("page", ComponentKind::Page).holding(vec![ComponentNode::bound(
                "parcel_table",
                ComponentKind::Table,
                Binding::Collection(CollectionId::new("parcel")),
            )
            .labelled("screen.parcel.list.table")]),
        ];
        screen
    }

    fn sample() -> Interface {
        let mut interface = Interface::new();
        interface.screens = vec![sample_screen()];
        interface.conflict_rules = vec![ConflictRule {
            collection: CollectionId::new("parcel"),
            resolution: ConflictResolution::LaterTimestamp {
                field: FieldPath::parse("recorded_at"),
            },
            statement: "The later recording stays, so the newest survey wins.".to_string(),
        }];
        let entries: Vec<(StringKey, String)> = interface
            .string_keys()
            .into_iter()
            .map(|key| {
                let sentence = alloc::format!("The words for {key}.");
                (key, sentence)
            })
            .collect();
        interface.translations = vec![Translation::new("en", entries)];
        interface
    }

    fn run(interface: &Interface) -> Report {
        let type_defs = types();
        let (type_registry, _) = TypeRegistry::build(&type_defs, &Path::root().field("types"));
        let held = collections();
        let (collection_registry, _) =
            CollectionRegistry::build(&held, &Path::root().field("collections"));
        let action_defs = actions();
        let action_registry = ActionRegistry::build(&action_defs);
        let workflow_defs = workflows();
        let workflow_registry = WorkflowRegistry::build(&workflow_defs);
        let view_defs = views();
        let view_registry = ViewRegistry::build(&view_defs);
        let events: BTreeSet<&str> = BTreeSet::new();
        interface.validate(
            &type_registry,
            &collection_registry,
            &identities(),
            &action_registry,
            &workflow_registry,
            &view_registry,
            &events,
            &Path::root().field("interface"),
        )
    }

    #[test]
    fn a_complete_interface_validates() {
        let report = run(&sample());
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn a_binding_naming_an_absent_collection_reports_its_location() {
        let mut interface = sample();
        interface.screens[0].components[0].children[0].binding =
            Some(Binding::Collection(CollectionId::new("absent_collection")));
        let report = run(&interface);
        let found = report
            .find("interface.screens[0].components[0].children[0].binding")
            .expect("the diagnostic sits on the binding");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_binding_naming_an_absent_field_reports_its_location() {
        let mut interface = sample();
        interface.screens[0].components[0].children[0] = ComponentNode::bound(
            "survey_field",
            ComponentKind::TextInput,
            Binding::Field {
                collection: CollectionId::new("parcel"),
                field: FieldPath::parse("absent_field"),
            },
        );
        let report = run(&interface);
        let found = report
            .find("interface.screens[0].components[0].children[0].binding.field")
            .expect("the diagnostic sits on the field");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_table_bound_to_an_action_is_caught() {
        let mut interface = sample();
        interface.screens[0].components[0].children[0].binding =
            Some(Binding::Action(ActionId::new("record_parcel")));
        let report = run(&interface);
        let found = report
            .find("interface.screens[0].components[0].children[0].binding")
            .expect("the diagnostic sits on the binding");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
    }

    #[test]
    fn a_screen_naming_an_absent_role_reports_its_location() {
        let mut interface = sample();
        interface.screens[0].roles = vec![RoleId::new("absent_role")];
        let report = run(&interface);
        let found = report
            .find("interface.screens[0].roles[0]")
            .expect("the diagnostic sits on the role");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_state_rule_naming_an_absent_state_is_caught() {
        let mut interface = sample();
        interface.screens[0].components[0].children[0].visibility =
            vec![VisibilityRule::InStates {
                workflow: WorkflowId::new("parcel_life"),
                states: vec![StateId::new("absent_state")],
            }];
        let report = run(&interface);
        assert!(report
            .find("interface.screens[0].components[0].children[0].visibility[0][0]")
            .is_some());
    }

    #[test]
    fn a_data_rule_carrying_a_value_other_than_a_truth_value_is_caught() {
        let mut interface = sample();
        interface.screens[0].components[0].children[0].visibility =
            vec![VisibilityRule::WhenValue(Expr::field(
                Expr::bound(ROW_BINDING),
                "district",
            ))];
        let report = run(&interface);
        let found = report
            .find("interface.screens[0].components[0].children[0].visibility[0]")
            .expect("the diagnostic sits on the rule");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
    }

    #[test]
    fn a_data_rule_reading_a_declared_field_validates() {
        let mut interface = sample();
        interface.screens[0].components[0].children[0].visibility =
            vec![VisibilityRule::WhenValue(Expr::binary(
                BinaryOp::Equal,
                Expr::field(Expr::bound(ROW_BINDING), "district"),
                Expr::text("kollam"),
            ))];
        let report = run(&interface);
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn a_role_rule_naming_a_role_outside_the_screen_is_caught() {
        let mut interface = sample();
        interface.screens[0].roles = vec![RoleId::new("registrar")];
        interface.screens[0].components[0].children[0].visibility =
            vec![VisibilityRule::ForRoles(vec![RoleId::new("holder")])];
        let report = run(&interface);
        assert!(report
            .find("interface.screens[0].components[0].children[0].visibility[0][0]")
            .is_some());
    }

    #[test]
    fn an_override_naming_an_absent_breakpoint_is_caught() {
        let mut interface = sample();
        interface.screens[0].components[0].children[0].overrides =
            vec![BreakpointOverride::new("watch", true, 4, 0)];
        let report = run(&interface);
        let found = report
            .find("interface.screens[0].components[0].children[0].overrides[0].breakpoint")
            .expect("the diagnostic sits on the override");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_theme_leaving_out_a_token_is_caught() {
        let mut interface = sample();
        interface
            .theme
            .colour_light
            .retain(|token| token.name.as_str() != "focus_ring");
        let report = run(&interface);
        let found = report
            .find("interface.theme.colour_light")
            .expect("the diagnostic sits on the colour set");
        assert_eq!(found.code, DiagnosticCode::MissingValue);
    }

    #[test]
    fn a_colour_token_carrying_other_text_is_caught() {
        let mut interface = sample();
        interface.theme.colour_light[0].value = "blue".to_string();
        let report = run(&interface);
        let found = report
            .find("interface.theme.colour_light[0].value")
            .expect("the diagnostic sits on the token");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
    }

    #[test]
    fn a_translation_leaving_out_a_key_is_caught() {
        let mut interface = sample();
        interface.translations[0].entries.pop();
        let report = run(&interface);
        let found = report
            .find("interface.translations[0].entries")
            .expect("the diagnostic sits on the entries");
        assert_eq!(found.code, DiagnosticCode::MissingValue);
    }

    #[test]
    fn a_conflict_rule_reading_a_field_of_another_type_is_caught() {
        let mut interface = sample();
        interface.conflict_rules[0].resolution = ConflictResolution::LaterTimestamp {
            field: FieldPath::parse("district"),
        };
        let report = run(&interface);
        let found = report
            .find("interface.conflict_rules[0].resolution")
            .expect("the diagnostic sits on the rule");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
    }

    #[test]
    fn two_screens_on_one_address_are_caught() {
        let mut interface = sample();
        let mut second = sample_screen();
        second.id = ScreenId::new("parcel_list_again");
        second.title_key = StringKey::new("screen.parcel.list.again");
        interface.screens.push(second);
        let entries: Vec<(StringKey, String)> = interface
            .string_keys()
            .into_iter()
            .map(|key| {
                let sentence = alloc::format!("The words for {key}.");
                (key, sentence)
            })
            .collect();
        interface.translations = vec![Translation::new("en", entries)];
        let report = run(&interface);
        let found = report
            .find("interface.screens[1].route")
            .expect("the diagnostic sits on the address");
        assert_eq!(found.code, DiagnosticCode::DuplicateIdentifier);
    }

    #[test]
    fn every_component_of_the_set_names_a_group_and_a_label() {
        let mut labels: BTreeSet<&str> = BTreeSet::new();
        for component in ComponentKind::every() {
            assert!(labels.insert(component.label()), "{}", component.label());
            let _ = component.group();
        }
        assert_eq!(labels.len(), 52);
    }

    #[test]
    fn every_primitive_type_reaches_an_input_component() {
        let held = [
            TypeExpr::Text,
            TypeExpr::Integer,
            TypeExpr::Decimal { scale: 2 },
            TypeExpr::Bool,
            TypeExpr::Timestamp,
            TypeExpr::Identity,
            TypeExpr::Hash,
            TypeExpr::Bytes,
            TypeExpr::Reference(CollectionId::new("parcel")),
            TypeExpr::Enumeration(TypeId::new("parcel_state")),
            TypeExpr::Sum(TypeId::new("decision")),
            TypeExpr::list(TypeExpr::Text),
            TypeExpr::map(TypeExpr::Text, TypeExpr::Text),
            TypeExpr::optional(TypeExpr::Text),
        ];
        for ty in held {
            let component = ComponentKind::for_type(&ty);
            assert_eq!(
                component.group(),
                ComponentGroup::Input,
                "{}",
                ty.describe()
            );
        }
    }

    #[test]
    fn the_string_keys_carry_every_label() {
        let interface = sample();
        let keys = interface.string_keys();
        assert!(keys.contains(&StringKey::new("screen.parcel.list.title")));
        assert!(keys.contains(&StringKey::new("screen.parcel.list.table")));
        assert!(keys.contains(&StringKey::new("breakpoint.phone")));
    }

    #[test]
    fn a_translation_key_holding_an_unexpected_character_is_caught() {
        let key = StringKey::new("screen.Parcel.title");
        let outcome = key.validate(&Path::root().field("title_key"));
        assert!(outcome.is_err());
    }

    #[test]
    fn a_language_tag_holding_an_unexpected_shape_is_caught() {
        assert!(LanguageTag::new("en")
            .validate(&Path::root().field("language"))
            .is_ok());
        assert!(LanguageTag::new("en-in")
            .validate(&Path::root().field("language"))
            .is_ok());
        assert!(LanguageTag::new("English")
            .validate(&Path::root().field("language"))
            .is_err());
    }

    #[test]
    fn a_property_written_twice_is_caught() {
        let mut interface = sample();
        interface.screens[0].components[0].children[0].properties = vec![
            Property::new("sort_direction", "ascending"),
            Property::new("sort_direction", "descending"),
        ];
        let report = run(&interface);
        let found = report
            .find("interface.screens[0].components[0].children[0].properties[1].name")
            .expect("the diagnostic sits on the property");
        assert_eq!(found.code, DiagnosticCode::DuplicateName);
    }

    #[test]
    fn a_property_carrying_an_empty_value_is_caught() {
        let mut interface = sample();
        interface.screens[0].components[0].children[0].properties =
            vec![Property::new("page_size", "")];
        let report = run(&interface);
        let found = report
            .find("interface.screens[0].components[0].children[0].properties[0].value")
            .expect("the diagnostic sits on the value");
        assert_eq!(found.code, DiagnosticCode::MissingValue);
    }

    #[test]
    fn the_section_round_trips_through_scale_and_json() {
        let interface = sample();
        let bytes = interface.encode();
        let back = Interface::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(interface, back);
        let text = serde_json::to_string(&interface).expect("the section serialises");
        let parsed: Interface = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(interface, parsed);
    }

    #[test]
    fn the_screens_for_one_role_leave_out_the_screens_it_lacks() {
        let mut interface = sample();
        interface.screens[0].roles = vec![RoleId::new("registrar")];
        assert_eq!(interface.screens_for(&RoleId::new("registrar")).len(), 1);
        assert_eq!(interface.screens_for(&RoleId::new("holder")).len(), 0);
    }
}

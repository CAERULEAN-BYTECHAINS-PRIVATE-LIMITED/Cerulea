//! View evaluation with projection, filter, sort, aggregate and join.
//!
//! A view is a declarative query. The interpreter runs it in seven passes,
//! in order.
//!
//! 1. **The source.** The records of the source collection reach the
//!    interpreter through the permission module, so a caller sees the
//!    rows and the fields its roles reach and nothing else.
//! 2. **The joins.** Each join reads its collection and matches the field
//!    on the source side against the field on the joined side. A matched
//!    join drops a row that finds nothing. An optional join keeps the row
//!    and binds nothing.
//! 3. **The filter.** The filter reads the source record under the name
//!    `record` and each joined record under its alias.
//! 4. **The projection.** Each projected field evaluates to one value.
//! 5. **The grouping.** A view naming group fields collects the rows
//!    sharing those values into one group.
//! 6. **The aggregates.** Each aggregate runs over one group. Count
//!    counts the rows. Total adds the values. Smallest and largest take
//!    the ends of the range.
//! 7. **The sort and the limit.** The rows order by the named fields and
//!    the result stops at the declared limit.
//!
//! Every pass is bounded. The source read stops at the row limit the
//! runtime sets, and each join reads inside the same limit.

use alloc::{vec, vec::Vec};
use cerulea_blueprint::{
    collections::KeyRange,
    expression::{Env, Meter},
    types::{Decimal, FieldName, ParamName, Value},
    views::{Aggregate, AggregateFunction, JoinKind, SortDirection, View},
    Path,
};
use frame_support::{ensure, traits::Get};
use pallet_cerulea_identity::IdentityRegistry;
use pallet_cerulea_permission::PermissionMatrix;
use pallet_cerulea_schema::primitives::{name_of, DeploymentId};
use pallet_cerulea_state::{value_at, RecordStore};
use sp_runtime::DispatchError;

use crate::{Config, Error, Pallet, StateView};

/// One row of a view result.
pub type Row = Vec<(FieldName, Value)>;

/// The bindings one row evaluates under.
pub type RowBindings = Vec<(ParamName, Value)>;

/// The name the source record binds to.
pub const SOURCE_BINDING: &str = "record";

/// Read one field of one row.
pub fn field_of<'a>(row: &'a Row, name: &FieldName) -> Option<&'a Value> {
    row.iter()
        .find(|(held, _)| held == name)
        .map(|(_, value)| value)
}

/// Compare two rows by one sort key.
pub fn compare_rows(left: &Row, right: &Row, name: &FieldName) -> core::cmp::Ordering {
    match (field_of(left, name), field_of(right, name)) {
        (Some(one), Some(other)) => one.cmp(other),
        (Some(_), None) => core::cmp::Ordering::Greater,
        (None, Some(_)) => core::cmp::Ordering::Less,
        (None, None) => core::cmp::Ordering::Equal,
    }
}

/// Add two values of the same numeric shape.
///
/// A pair the arithmetic declines answers with nothing, so the caller
/// reports the problem rather than mixing shapes.
pub fn add_values(left: &Value, right: &Value) -> Option<Value> {
    match (left, right) {
        (Value::Integer(one), Value::Integer(other)) => {
            Some(Value::Integer(one.checked_add(*other)?))
        }
        (Value::Decimal(one), Value::Decimal(other)) => one.add(other).ok().map(Value::Decimal),
        _ => None,
    }
}

/// The zero of one numeric shape.
pub fn zero_like(sample: &Value) -> Option<Value> {
    match sample {
        Value::Integer(_) => Some(Value::Integer(0)),
        Value::Decimal(held) => Some(Value::Decimal(Decimal::zero(held.scale))),
        _ => None,
    }
}

impl<T: Config> Pallet<T> {
    /// Evaluate one view and answer with its rows.
    pub fn run_view(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        view_name: &str,
    ) -> Result<Vec<Row>, DispatchError> {
        let blueprint = Self::installed(deployment)?;
        let view = blueprint
            .views
            .iter()
            .find(|held| held.id.as_str() == view_name)
            .cloned()
            .ok_or(Error::<T>::ViewUnknown)?;
        Self::evaluate_view(deployment, actor, &view)
    }

    /// Evaluate one view definition and answer with its rows.
    pub fn evaluate_view(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        view: &View,
    ) -> Result<Vec<Row>, DispatchError> {
        Self::check_view_roles(deployment, actor, view)?;
        let limit = T::MaxRowsPerRead::get();
        let mut meter = Self::meter();

        // Pass one. The source records the caller reaches.
        let source: Vec<Value> =
            T::Records::range(deployment, &view.source, &KeyRange::Full, limit)
                .into_iter()
                .map(|(_, record)| record)
                .collect();
        let visible = T::Permissions::visible_records(deployment, actor, &view.source, &source);

        let binding = ParamName::new(SOURCE_BINDING);
        let mut sets: Vec<RowBindings> = visible
            .into_iter()
            .map(|record| vec![(binding.clone(), record)])
            .collect();

        // Pass two. The joins.
        for join in view.joins.iter() {
            let right: Vec<Value> =
                T::Records::range(deployment, &join.collection, &KeyRange::Full, limit)
                    .into_iter()
                    .map(|(_, record)| record)
                    .collect();
            let right =
                T::Permissions::visible_records(deployment, actor, &join.collection, &right);
            let mut produced: Vec<RowBindings> = Vec::new();
            for set in sets.into_iter() {
                let left_value = set
                    .iter()
                    .find(|(name, _)| name == &binding)
                    .and_then(|(_, record)| value_at(record, &join.left));
                let mut matched = false;
                for candidate in right.iter() {
                    let right_value = value_at(candidate, &join.right);
                    if left_value.is_some() && left_value == right_value {
                        let mut held = set.clone();
                        held.push((join.alias.clone(), Self::join_value(join.kind, candidate)));
                        produced.push(held);
                        matched = true;
                    }
                }
                if !matched && join.kind == JoinKind::Optional {
                    let mut held = set.clone();
                    held.push((
                        join.alias.clone(),
                        Value::none(cerulea_blueprint::types::TypeExpr::Bool),
                    ));
                    produced.push(held);
                }
                ensure!(
                    produced.len() <= limit as usize,
                    Error::<T>::EffectLimitReached
                );
            }
            sets = produced;
        }

        // Pass three and pass four. The filter and the projection.
        let state = StateView::<T>::new(*deployment, limit);
        let base = Env::with_view(Self::identity_of(actor), Self::now(), &state);
        let mut rows: Vec<(Row, RowBindings)> = Vec::new();
        for set in sets.into_iter() {
            let mut env = base.clone();
            for (name, value) in set.iter() {
                env.bind(name, value.clone());
            }
            if let Some(filter) = view.filter.as_ref() {
                let held = Self::run_in(filter, &env, &mut meter, &Path::root().field("filter"))?;
                if held != Value::Bool(true) {
                    continue;
                }
            }
            let mut row: Row = Vec::new();
            for field in view.projection.iter() {
                let value = Self::run_in(
                    &field.value,
                    &env,
                    &mut meter,
                    &Path::root().field("projection"),
                )?;
                row.push((field.name.clone(), value));
            }
            rows.push((row, set));
        }

        // Pass five and pass six. The grouping and the aggregates.
        let mut produced = if view.aggregates.is_empty() {
            rows.into_iter().map(|(row, _)| row).collect()
        } else {
            Self::aggregate_rows(deployment, actor, view, rows, &mut meter)?
        };

        // Pass seven. The sort and the limit.
        for key in view.sort.iter().rev() {
            match key.direction {
                SortDirection::Ascending => {
                    produced.sort_by(|left, right| compare_rows(left, right, &key.name))
                }
                SortDirection::Descending => {
                    produced.sort_by(|left, right| compare_rows(right, left, &key.name))
                }
            }
        }
        if let Some(ceiling) = view.limit {
            produced.truncate(ceiling as usize);
        }
        produced.truncate(limit as usize);
        Ok(produced)
    }

    fn join_value(kind: JoinKind, record: &Value) -> Value {
        match kind {
            JoinKind::Matched => record.clone(),
            JoinKind::Optional => Value::some(record.infer_type(), record.clone()),
        }
    }

    fn check_view_roles(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        view: &View,
    ) -> Result<(), DispatchError> {
        if view.roles.is_empty() {
            return Ok(());
        }
        let now = T::Identity::now_millis();
        let held = T::Identity::roles_of(deployment, actor, now);
        for role in view.roles.iter() {
            if let Ok(named) = name_of(role.as_str()) {
                if held.contains(&named) {
                    return Ok(());
                }
            }
        }
        Err(Error::<T>::PermissionDenied.into())
    }

    fn aggregate_rows(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        view: &View,
        rows: Vec<(Row, RowBindings)>,
        meter: &mut Meter,
    ) -> Result<Vec<Row>, DispatchError> {
        let limit = T::MaxRowsPerRead::get();
        let state = StateView::<T>::new(*deployment, limit);
        let base = Env::with_view(Self::identity_of(actor), Self::now(), &state);

        // Collect the groups, keeping the order the rows arrived in.
        let mut keys: Vec<Vec<Value>> = Vec::new();
        let mut groups: Vec<Vec<(Row, RowBindings)>> = Vec::new();
        for (row, set) in rows.into_iter() {
            let key: Vec<Value> = view
                .group_by
                .iter()
                .map(|name| field_of(&row, name).cloned().unwrap_or(Value::Bool(false)))
                .collect();
            match keys.iter().position(|held| held == &key) {
                Some(slot) => groups[slot].push((row, set)),
                None => {
                    keys.push(key);
                    groups.push(vec![(row, set)]);
                }
            }
        }

        let mut produced: Vec<Row> = Vec::new();
        for (slot, group) in groups.into_iter().enumerate() {
            let mut row: Row = Vec::new();
            for (position, name) in view.group_by.iter().enumerate() {
                let value = keys[slot]
                    .get(position)
                    .cloned()
                    .unwrap_or(Value::Bool(false));
                row.push((name.clone(), value));
            }
            for aggregate in view.aggregates.iter() {
                let value = Self::fold_group(aggregate, &group, &base, meter)?;
                row.push((aggregate.name.clone(), value));
            }
            produced.push(row);
        }
        Ok(produced)
    }

    fn fold_group(
        aggregate: &Aggregate,
        group: &[(Row, RowBindings)],
        base: &Env,
        meter: &mut Meter,
    ) -> Result<Value, DispatchError> {
        if aggregate.function == AggregateFunction::Count {
            let count = i128::try_from(group.len()).unwrap_or(i128::MAX);
            return Ok(Value::Integer(count));
        }
        let expression = aggregate.value.as_ref().ok_or(Error::<T>::ResultMismatch)?;

        let mut held: Option<Value> = None;
        for (_, set) in group.iter() {
            let mut env = base.clone();
            for (name, value) in set.iter() {
                env.bind(name, value.clone());
            }
            let value = Self::run_in(expression, &env, meter, &Path::root().field("aggregate"))?;
            held = Some(match (held, aggregate.function) {
                (None, AggregateFunction::Total) => {
                    let start = zero_like(&value).ok_or(Error::<T>::ResultMismatch)?;
                    add_values(&start, &value).ok_or(Error::<T>::ResultMismatch)?
                }
                (None, _) => value,
                (Some(running), AggregateFunction::Total) => {
                    add_values(&running, &value).ok_or(Error::<T>::ResultMismatch)?
                }
                (Some(running), AggregateFunction::Smallest) => {
                    if value < running {
                        value
                    } else {
                        running
                    }
                }
                (Some(running), AggregateFunction::Largest) => {
                    if value > running {
                        value
                    } else {
                        running
                    }
                }
                (Some(running), AggregateFunction::Count) => running,
            });
        }
        held.ok_or_else(|| Error::<T>::ResultMismatch.into())
    }
}

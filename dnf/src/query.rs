use crate::{DnfEvaluable, Op, OpRegistry, Value};
use std::fmt;

/// Represents a single `field operator value` test in a DNF query.
///
/// Conditions are the leaves of a query: a [`Conjunction`] is an `AND` of
/// conditions, and a [`DnfQuery`] is an `OR` of conjunctions.
///
/// # Examples
///
/// ```
/// use dnf::{DnfQuery, Op};
///
/// let query = DnfQuery::builder()
///     .or(|c| c.and("age", Op::GT, 18))
///     .build();
///
/// let condition = &query.conjunctions()[0].conditions()[0];
/// assert_eq!(condition.field_name(), "age");
/// assert_eq!(condition.operator(), &Op::GT);
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Condition {
    field_name: Box<str>,
    operator: Op,
    value: Value,
}

impl Condition {
    pub(crate) fn new(
        field_name: impl Into<Box<str>>,
        operator: Op,
        value: impl Into<Value>,
    ) -> Self {
        Self {
            field_name: field_name.into(),
            operator,
            value: value.into(),
        }
    }

    /// Returns the field name this condition tests.
    pub fn field_name(&self) -> &str {
        &self.field_name
    }

    /// Returns the operator used by this condition.
    pub fn operator(&self) -> &Op {
        &self.operator
    }

    /// Returns the right-hand value this condition compares against.
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Evaluates the condition against a single target.
    ///
    /// Returns whatever [`DnfEvaluable::evaluate_field`] returns. Custom
    /// operators are *not* dispatched here — call
    /// [`DnfQuery::evaluate`] for that.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{Condition, DnfEvaluable, Op, Value};
    ///
    /// #[derive(DnfEvaluable)]
    /// struct User { age: u32 }
    ///
    /// let user = User { age: 30 };
    /// let cond = dnf::DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::GT, 18))
    ///     .build()
    ///     .conjunctions()[0]
    ///     .conditions()[0]
    ///     .clone();
    /// assert!(cond.evaluate(&user));
    /// ```
    pub fn evaluate<T: DnfEvaluable>(&self, target: &T) -> bool {
        target.evaluate_field(&self.field_name, &self.operator, &self.value)
    }
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.field_name, self.operator, self.value)
    }
}

/// An `AND` of [`Condition`]s — one clause inside a [`DnfQuery`].
///
/// Construct conjunctions through [`DnfQuery::builder`]; the public surface
/// here exists for inspection and serialization.
///
/// # Examples
///
/// ```
/// use dnf::{DnfQuery, Op};
///
/// let query = DnfQuery::builder()
///     .or(|c| c.and("age", Op::GT, 18).and("country", Op::EQ, "US"))
///     .build();
///
/// let conj = &query.conjunctions()[0];
/// assert_eq!(conj.len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Conjunction {
    conditions: Vec<Condition>,
}

impl Conjunction {
    pub(crate) fn from_conditions(conditions: Vec<Condition>) -> Self {
        Self { conditions }
    }

    /// Returns the conditions joined by `AND`.
    pub fn conditions(&self) -> &[Condition] {
        &self.conditions
    }

    /// Evaluates the conjunction against a target.
    ///
    /// Returns `true` if every condition matches. An empty conjunction is
    /// vacuously `true`; the builder filters those out, so a `true` result here
    /// only happens for an empty conjunction created by deserialization or by
    /// internal APIs.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{DnfEvaluable, DnfQuery, Op};
    ///
    /// #[derive(DnfEvaluable)]
    /// struct User { age: u32, premium: bool }
    ///
    /// let user = User { age: 30, premium: true };
    /// let query = DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::GT, 18).and("premium", Op::EQ, true))
    ///     .build();
    /// assert!(query.conjunctions()[0].evaluate(&user));
    /// ```
    pub fn evaluate<T: DnfEvaluable>(&self, target: &T) -> bool {
        if self.conditions.is_empty() {
            return true;
        }
        self.conditions
            .iter()
            .all(|condition| condition.evaluate(target))
    }

    /// Returns the number of conditions in this conjunction.
    pub fn len(&self) -> usize {
        self.conditions.len()
    }

    /// Returns `true` if this conjunction has no conditions.
    ///
    /// An empty conjunction is vacuously satisfied — see
    /// [`evaluate`](Self::evaluate).
    pub fn is_empty(&self) -> bool {
        self.conditions.is_empty()
    }
}

impl fmt::Display for Conjunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.conditions.is_empty() {
            write!(f, "true")
        } else {
            let conditions: Vec<String> = self.conditions.iter().map(|c| c.to_string()).collect();
            write!(f, "({})", conditions.join(" AND "))
        }
    }
}

/// A query in disjunctive normal form: an `OR` of [`Conjunction`]s.
///
/// Construct with [`DnfQuery::builder`] or, with the `parser` feature, with
/// [`QueryBuilder::from_query`](crate::QueryBuilder::from_query).
///
/// # Examples
///
/// ```
/// use dnf::{DnfEvaluable, DnfQuery, Op};
///
/// #[derive(DnfEvaluable)]
/// struct User { age: u32, country: String, premium: bool }
///
/// let user = User { age: 30, country: "US".into(), premium: false };
/// let query = DnfQuery::builder()
///     .or(|c| c.and("age", Op::GT, 18).and("country", Op::EQ, "US"))
///     .or(|c| c.and("premium", Op::EQ, true))
///     .build();
/// assert!(query.evaluate(&user));
/// ```
#[cfg_attr(not(feature = "parser"), allow(rustdoc::broken_intra_doc_links))]
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DnfQuery {
    conjunctions: Vec<Conjunction>,
    #[cfg_attr(feature = "serde", serde(skip))]
    custom_ops: Option<OpRegistry>,
}

impl PartialEq for DnfQuery {
    fn eq(&self, other: &Self) -> bool {
        // custom_ops holds function pointers, which have no equality.
        self.conjunctions == other.conjunctions
    }
}

impl DnfQuery {
    pub(crate) fn from_conjunctions(conjunctions: Vec<Conjunction>) -> Self {
        Self {
            conjunctions,
            custom_ops: None,
        }
    }

    pub(crate) fn set_custom_ops(mut self, registry: OpRegistry) -> Self {
        self.custom_ops = Some(registry);
        self
    }

    /// Returns the conjunctions joined by `OR`.
    pub fn conjunctions(&self) -> &[Conjunction] {
        &self.conjunctions
    }

    pub(crate) fn into_parts(self) -> (Vec<Conjunction>, Option<OpRegistry>) {
        (self.conjunctions, self.custom_ops)
    }

    #[cfg(feature = "parser")]
    pub(crate) fn into_conjunctions(self) -> Vec<Conjunction> {
        self.conjunctions
    }

    /// Returns a new [`QueryBuilder`](crate::QueryBuilder) for fluent construction.
    pub fn builder() -> crate::builder::QueryBuilder {
        crate::builder::QueryBuilder::new()
    }

    /// Returns the attached custom-operator registry, or `None` if none was set.
    pub fn custom_ops(&self) -> Option<&OpRegistry> {
        self.custom_ops.as_ref()
    }

    /// Returns `true` if a custom operator with `name` is registered.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{DnfQuery, Op, Value};
    ///
    /// let query = DnfQuery::builder()
    ///     .with_custom_op("IS_ADULT", true, |f, _| matches!(f, Value::Uint(n) if *n >= 18))
    ///     .or(|c| c.and("age", Op::custom("IS_ADULT"), Value::None))
    ///     .build();
    /// assert!(query.has_custom_op("IS_ADULT"));
    /// assert!(!query.has_custom_op("MISSING"));
    /// ```
    pub fn has_custom_op(&self, name: &str) -> bool {
        self.custom_ops.as_ref().is_some_and(|r| r.contains(name))
    }

    /// Verifies every custom operator used in the query is registered.
    ///
    /// # Errors
    ///
    /// Returns [`DnfError::UnregisteredCustomOp`](crate::DnfError::UnregisteredCustomOp)
    /// for the first condition referencing an operator that has not been
    /// registered.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{DnfQuery, Op, Value};
    ///
    /// let bad = DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::custom("IS_SENIOR"), Value::None))
    ///     .build();
    /// assert!(bad.validate_custom_ops().is_err());
    /// ```
    pub fn validate_custom_ops(&self) -> Result<(), crate::DnfError> {
        for conjunction in &self.conjunctions {
            for condition in &conjunction.conditions {
                if let Some(custom_name) = condition.operator.custom_name() {
                    if !self.has_custom_op(custom_name) {
                        return Err(crate::DnfError::UnregisteredCustomOp {
                            operator_name: custom_name.into(),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    /// Evaluates the query against a target.
    ///
    /// Returns `true` if any conjunction matches. Short-circuits on the first
    /// matching conjunction; standard operators compare directly without
    /// converting fields to [`Value`], while custom operators convert only the
    /// fields they touch.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{DnfEvaluable, DnfQuery, Op};
    ///
    /// #[derive(DnfEvaluable)]
    /// struct User { age: u32, premium: bool }
    ///
    /// let user = User { age: 30, premium: false };
    /// let query = DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::GT, 18))
    ///     .or(|c| c.and("premium", Op::EQ, true))
    ///     .build();
    /// assert!(query.evaluate(&user));
    /// ```
    pub fn evaluate<T: DnfEvaluable>(&self, target: &T) -> bool {
        self.conjunctions
            .iter()
            .any(|conjunction| self.evaluate_conjunction(conjunction, target))
    }

    fn evaluate_conjunction<T: DnfEvaluable>(&self, conj: &Conjunction, target: &T) -> bool {
        if conj.conditions.is_empty() {
            return true;
        }
        conj.conditions
            .iter()
            .all(|cond| self.evaluate_condition(cond, target))
    }

    fn evaluate_condition<T: DnfEvaluable>(&self, cond: &Condition, target: &T) -> bool {
        if let Some(custom_name) = cond.operator.custom_name() {
            if let Some(registry) = &self.custom_ops {
                if let Some(field_value) = target.field_value(&cond.field_name) {
                    let result = registry
                        .evaluate(custom_name, &field_value, &cond.value)
                        .unwrap_or(false);
                    return if cond.operator.is_inverse() {
                        !result
                    } else {
                        result
                    };
                }
            }
            return cond.operator.is_inverse();
        }

        target.evaluate_field(&cond.field_name, &cond.operator, &cond.value)
    }

    /// Returns the number of conjunctions in the query.
    pub fn len(&self) -> usize {
        self.conjunctions.len()
    }

    /// Returns `true` if the query has no conjunctions and therefore matches nothing.
    pub fn is_empty(&self) -> bool {
        self.conjunctions.is_empty()
    }

    /// Returns an iterator over every field name referenced by the query.
    ///
    /// Names appear with duplicates in the order they were added; collect into
    /// a `HashSet` if you need uniqueness.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{DnfQuery, Op};
    ///
    /// let query = DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::GT, 18).and("country", Op::EQ, "US"))
    ///     .build();
    /// let names: Vec<_> = query.field_names().collect();
    /// assert_eq!(names, vec!["age", "country"]);
    /// ```
    pub fn field_names(&self) -> impl Iterator<Item = &str> {
        self.conjunctions
            .iter()
            .flat_map(|conj| conj.conditions().iter().map(|c| c.field_name()))
    }

    /// Returns the total number of conditions across all conjunctions.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{DnfQuery, Op};
    ///
    /// let query = DnfQuery::builder()
    ///     .or(|c| c.and("a", Op::EQ, 1).and("b", Op::EQ, 2))
    ///     .or(|c| c.and("c", Op::EQ, 3))
    ///     .build();
    /// assert_eq!(query.condition_count(), 3);
    /// ```
    pub fn condition_count(&self) -> usize {
        self.conjunctions.iter().map(|c| c.len()).sum()
    }

    /// Returns `true` if any condition references `name`.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{DnfQuery, Op};
    ///
    /// let query = DnfQuery::builder().or(|c| c.and("age", Op::GT, 18)).build();
    /// assert!(query.uses_field("age"));
    /// assert!(!query.uses_field("name"));
    /// ```
    pub fn uses_field(&self, name: &str) -> bool {
        self.conjunctions
            .iter()
            .any(|conj| conj.conditions().iter().any(|c| c.field_name() == name))
    }

    /// Returns `true` if the query has no conjunctions and so always evaluates to `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::DnfQuery;
    ///
    /// assert!(DnfQuery::builder().build().is_always_false());
    /// ```
    pub fn is_always_false(&self) -> bool {
        self.conjunctions.is_empty()
    }

    /// Returns `true` if any conjunction is empty and so always evaluates to `true`.
    ///
    /// The builder filters empty conjunctions, so a `true` result here only
    /// happens for queries built via deserialization or internal APIs.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{DnfQuery, Op};
    ///
    /// let q = DnfQuery::builder().or(|c| c.and("age", Op::GT, 18)).build();
    /// assert!(!q.is_always_true());
    /// ```
    pub fn is_always_true(&self) -> bool {
        self.conjunctions.iter().any(|c| c.is_empty())
    }

    /// Validates field names and custom operators against type `T`.
    ///
    /// # Errors
    ///
    /// - [`DnfError::UnknownField`](crate::DnfError::UnknownField) for the first
    ///   condition that references a field not declared by `T::fields()`.
    /// - [`DnfError::UnregisteredCustomOp`](crate::DnfError::UnregisteredCustomOp)
    ///   for the first condition using a custom operator that has not been
    ///   registered.
    /// - [`DnfError::InvalidMapTarget`](crate::DnfError::InvalidMapTarget) when a
    ///   `@keys` / `@values` / `at_key` value is paired with a non-map field.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{DnfEvaluable, DnfQuery, Op};
    ///
    /// #[derive(DnfEvaluable)]
    /// struct User { age: u32, name: String }
    ///
    /// let query = DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::GT, 18))
    ///     .build()
    ///     .validate::<User>()?;
    /// # Ok::<(), dnf::DnfError>(())
    /// ```
    pub fn validate<T: crate::DnfEvaluable>(self) -> Result<Self, crate::DnfError> {
        use crate::FieldKind;

        self.validate_custom_ops()?;

        for conj in &self.conjunctions {
            for condition in conj.conditions() {
                let field_name = condition.field_name();
                let value = condition.value();

                let field_kind = T::validate_field_path(field_name).ok_or_else(|| {
                    crate::DnfError::UnknownField {
                        field_name: field_name.into(),
                        position: None,
                    }
                })?;

                if value.is_map_targeted() && field_kind != FieldKind::Map {
                    return Err(crate::DnfError::InvalidMapTarget {
                        field_name: field_name.into(),
                        field_kind,
                    });
                }
            }
        }

        Ok(self)
    }

    /// Merges another query into this one as an `OR` combination.
    ///
    /// Custom-operator registries from `other` are merged in; on name collision,
    /// `other` wins.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{DnfQuery, Op};
    ///
    /// let adults = DnfQuery::builder().or(|c| c.and("age", Op::GTE, 18)).build();
    /// let premium = DnfQuery::builder().or(|c| c.and("premium", Op::EQ, true)).build();
    /// let combined = adults.merge(premium);
    /// assert_eq!(combined.conjunctions().len(), 2);
    /// ```
    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        let (conjunctions, custom_ops) = other.into_parts();
        self.conjunctions.extend(conjunctions);
        if let Some(other_ops) = custom_ops {
            match &mut self.custom_ops {
                Some(ops) => {
                    ops.merge(other_ops);
                }
                None => self.custom_ops = Some(other_ops),
            }
        }
        self
    }
}

impl fmt::Display for DnfQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.conjunctions.is_empty() {
            write!(f, "false")
        } else {
            let conjunctions: Vec<String> =
                self.conjunctions.iter().map(|c| c.to_string()).collect();
            write!(f, "{}", conjunctions.join(" OR "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Mock Implementation ====================

    struct MockStruct {
        age: i32,
        name: String,
        active: bool,
    }

    impl DnfEvaluable for MockStruct {
        fn evaluate_field(&self, field_name: &str, operator: &Op, value: &Value) -> bool {
            use crate::DnfField;
            match field_name {
                "age" => (self.age as i64).evaluate(operator, value),
                "name" => self.name.evaluate(operator, value),
                "active" => self.active.evaluate(operator, value),
                _ => false,
            }
        }

        fn fields() -> impl Iterator<Item = crate::FieldInfo> {
            [
                crate::FieldInfo::new("age", "i32"),
                crate::FieldInfo::new("name", "String"),
                crate::FieldInfo::new("active", "bool"),
            ]
            .into_iter()
        }
    }

    fn mock(age: i32, name: &str, active: bool) -> MockStruct {
        MockStruct {
            age,
            name: name.to_string(),
            active,
        }
    }

    // ==================== Condition Evaluation Tests (Data-Driven) ====================

    #[test]
    fn test_condition_evaluation() {
        let obj = mock(25, "Alice", true);

        let test_cases = vec![
            // (field, operator, value, expected, description)
            ("age", Op::GT, Value::Int(18), true, "age > 18"),
            ("age", Op::LT, Value::Int(18), false, "age < 18 (false)"),
            ("age", Op::EQ, Value::Int(25), true, "age == 25"),
            ("age", Op::NE, Value::Int(25), false, "age != 25 (false)"),
            ("age", Op::GTE, Value::Int(25), true, "age >= 25"),
            ("age", Op::LTE, Value::Int(25), true, "age <= 25"),
            ("name", Op::EQ, Value::from("Alice"), true, "name == Alice"),
            (
                "name",
                Op::CONTAINS,
                Value::from("lic"),
                true,
                "name contains 'lic'",
            ),
            (
                "name",
                Op::STARTS_WITH,
                Value::from("Ali"),
                true,
                "name starts with 'Ali'",
            ),
            (
                "name",
                Op::ENDS_WITH,
                Value::from("ice"),
                true,
                "name ends with 'ice'",
            ),
            ("active", Op::EQ, Value::Bool(true), true, "active == true"),
            (
                "active",
                Op::NE,
                Value::Bool(true),
                false,
                "active != true (false)",
            ),
            ("unknown", Op::EQ, Value::Int(1), false, "unknown field"),
        ];

        for (field, op, value, expected, desc) in test_cases {
            let condition = Condition::new(field, op, value);
            assert_eq!(condition.evaluate(&obj), expected, "Failed: {}", desc);
        }
    }

    // ==================== Conjunction Evaluation Tests ====================

    #[test]
    fn test_conjunction_evaluation() {
        let test_cases = vec![
            // (mock_data, conditions, expected, description)
            (
                mock(25, "Alice", true),
                vec![("age", Op::GT, Value::Int(18))],
                true,
                "single passing condition",
            ),
            (
                mock(25, "Alice", true),
                vec![("age", Op::LT, Value::Int(18))],
                false,
                "single failing condition",
            ),
            (
                mock(25, "Alice", true),
                vec![
                    ("age", Op::GT, Value::Int(18)),
                    ("active", Op::EQ, Value::Bool(true)),
                ],
                true,
                "multiple passing conditions (AND)",
            ),
            (
                mock(25, "Alice", true),
                vec![
                    ("age", Op::GT, Value::Int(18)),
                    ("active", Op::EQ, Value::Bool(false)),
                ],
                false,
                "one failing in AND",
            ),
            (
                mock(25, "Alice", true),
                vec![],
                true,
                "empty conjunction (vacuously true)",
            ),
        ];

        for (obj, conditions, expected, desc) in test_cases {
            let conj = Conjunction::from_conditions(
                conditions
                    .into_iter()
                    .map(|(field, op, value)| Condition::new(field, op, value))
                    .collect(),
            );
            assert_eq!(conj.evaluate(&obj), expected, "Failed: {}", desc);
        }
    }

    // ==================== DnfQuery Evaluation Tests ====================

    #[test]
    fn test_dnf_query_evaluation() {
        // (age > 18 AND active) OR (name == "Alice")
        let query = DnfQuery::builder()
            .or(|c| c.and("age", Op::GT, 18).and("active", Op::EQ, true))
            .or(|c| c.and("name", Op::EQ, "Alice"))
            .build();

        let test_cases = vec![
            (mock(25, "Alice", true), true, "matches both conjunctions"),
            (
                mock(25, "Bob", true),
                true,
                "matches first conjunction only",
            ),
            (
                mock(15, "Alice", false),
                true,
                "matches second conjunction only",
            ),
            (mock(15, "Bob", false), false, "matches neither"),
            (
                mock(25, "Bob", false),
                false,
                "age ok but not active, name wrong",
            ),
        ];

        for (obj, expected, desc) in test_cases {
            assert_eq!(query.evaluate(&obj), expected, "Failed: {}", desc);
        }
    }

    #[test]
    fn test_empty_query_evaluation() {
        let query = DnfQuery::from_conjunctions(vec![]);
        assert!(!query.evaluate(&mock(25, "Alice", true)));
        assert!(!query.evaluate(&mock(0, "", false)));
    }

    #[test]
    fn test_query_with_empty_conjunction() {
        let query = DnfQuery::from_conjunctions(vec![Conjunction::from_conditions(vec![])]);
        assert!(query.evaluate(&mock(25, "Alice", true)));
        assert!(query.evaluate(&mock(0, "", false)));
    }

    // ==================== Display Tests (Data-Driven) ====================

    #[test]
    fn test_condition_display() {
        let test_cases = vec![
            ("age", Op::GT, Value::Int(18), "age > 18"),
            ("age", Op::LT, Value::Int(18), "age < 18"),
            ("age", Op::EQ, Value::Int(18), "age == 18"),
            ("age", Op::NE, Value::Int(18), "age != 18"),
            ("age", Op::GTE, Value::Int(18), "age >= 18"),
            ("age", Op::LTE, Value::Int(18), "age <= 18"),
            ("name", Op::EQ, Value::from("Alice"), "name == \"Alice\""),
            (
                "name",
                Op::CONTAINS,
                Value::from("x"),
                "name CONTAINS \"x\"",
            ),
            (
                "name",
                Op::STARTS_WITH,
                Value::from("A"),
                "name STARTS WITH \"A\"",
            ),
            (
                "name",
                Op::ENDS_WITH,
                Value::from("e"),
                "name ENDS WITH \"e\"",
            ),
            ("active", Op::EQ, Value::Bool(true), "active == true"),
            ("score", Op::EQ, Value::Float(3.34), "score == 3.34"),
        ];

        for (field, op, value, expected) in test_cases {
            let condition = Condition::new(field, op, value);
            assert_eq!(condition.to_string(), expected);
        }
    }

    #[test]
    fn test_conjunction_display() {
        let conj = Conjunction::from_conditions(vec![
            Condition::new("age", Op::GT, Value::Int(18)),
            Condition::new("active", Op::EQ, Value::Bool(true)),
        ]);

        let display = conj.to_string();
        assert!(display.contains("age > 18"));
        assert!(display.contains("active == true"));
        assert!(display.contains("AND"));
    }

    #[test]
    fn test_query_display() {
        let query = DnfQuery::builder()
            .or(|c| c.and("age", Op::GT, 18))
            .or(|c| c.and("active", Op::EQ, true))
            .build();

        let display = query.to_string();
        assert!(display.contains("age > 18"));
        assert!(display.contains("active == true"));
        assert!(display.contains("OR"));
    }

    // ==================== Introspection API Tests (Data-Driven) ====================

    #[test]
    fn test_field_names() {
        use std::collections::HashSet;

        type TestCase = (fn() -> DnfQuery, Vec<&'static str>, &'static str);
        let test_cases: Vec<TestCase> = vec![
            (
                || DnfQuery::from_conjunctions(vec![]),
                vec![],
                "empty query",
            ),
            (
                || DnfQuery::builder().or(|c| c.and("age", Op::GT, 18)).build(),
                vec!["age"],
                "single field",
            ),
            (
                || {
                    DnfQuery::builder()
                        .or(|c| c.and("age", Op::GT, 18).and("name", Op::EQ, "x"))
                        .build()
                },
                vec!["age", "name"],
                "multiple fields in one conjunction",
            ),
            (
                || {
                    DnfQuery::builder()
                        .or(|c| c.and("age", Op::GT, 18))
                        .or(|c| c.and("name", Op::EQ, "x"))
                        .build()
                },
                vec!["age", "name"],
                "fields across conjunctions",
            ),
            (
                || {
                    DnfQuery::builder()
                        .or(|c| c.and("age", Op::GT, 18).and("age", Op::LT, 65))
                        .build()
                },
                vec!["age"],
                "duplicate field (should appear once)",
            ),
        ];

        for (build_fn, expected_fields, desc) in test_cases {
            let query = build_fn();
            let fields: HashSet<_> = query.field_names().collect();
            let expected: HashSet<_> = expected_fields.into_iter().collect();
            assert_eq!(fields, expected, "Failed: {}", desc);
        }
    }

    #[test]
    fn test_condition_count() {
        type TestCase = (fn() -> DnfQuery, usize, &'static str);
        let test_cases: Vec<TestCase> = vec![
            (|| DnfQuery::from_conjunctions(vec![]), 0, "empty query"),
            (
                || DnfQuery::builder().or(|c| c.and("a", Op::EQ, 1)).build(),
                1,
                "single condition",
            ),
            (
                || {
                    DnfQuery::builder()
                        .or(|c| c.and("a", Op::EQ, 1).and("b", Op::EQ, 2))
                        .build()
                },
                2,
                "two conditions in one conjunction",
            ),
            (
                || {
                    DnfQuery::builder()
                        .or(|c| c.and("a", Op::EQ, 1))
                        .or(|c| c.and("b", Op::EQ, 2))
                        .build()
                },
                2,
                "two conditions across conjunctions",
            ),
            (
                || {
                    DnfQuery::builder()
                        .or(|c| c.and("a", Op::EQ, 1).and("b", Op::EQ, 2))
                        .or(|c| c.and("c", Op::EQ, 3))
                        .build()
                },
                3,
                "three conditions total",
            ),
            (
                || DnfQuery::from_conjunctions(vec![Conjunction::from_conditions(vec![])]),
                0,
                "empty conjunction",
            ),
        ];

        for (build_fn, expected, desc) in test_cases {
            let query = build_fn();
            assert_eq!(query.condition_count(), expected, "Failed: {}", desc);
        }
    }

    #[test]
    fn test_uses_field() {
        let query = DnfQuery::builder()
            .or(|c| c.and("age", Op::GT, 18).and("name", Op::EQ, "x"))
            .or(|c| c.and("active", Op::EQ, true))
            .build();

        let test_cases = vec![
            ("age", true),
            ("name", true),
            ("active", true),
            ("unknown", false),
            ("AGE", false), // case sensitive
            ("", false),
        ];

        for (field, expected) in test_cases {
            assert_eq!(
                query.uses_field(field),
                expected,
                "Failed for field: '{}'",
                field
            );
        }
    }

    #[test]
    fn test_is_always_false() {
        type TestCase = (fn() -> DnfQuery, bool, &'static str);
        let test_cases: Vec<TestCase> = vec![
            (|| DnfQuery::from_conjunctions(vec![]), true, "empty query"),
            (
                || DnfQuery::builder().or(|c| c.and("x", Op::EQ, 1)).build(),
                false,
                "query with condition",
            ),
            (
                || DnfQuery::from_conjunctions(vec![Conjunction::from_conditions(vec![])]),
                false,
                "query with empty conjunction (always true)",
            ),
        ];

        for (build_fn, expected, desc) in test_cases {
            let query = build_fn();
            assert_eq!(query.is_always_false(), expected, "Failed: {}", desc);
        }
    }

    #[test]
    fn test_is_always_true() {
        type TestCase = (fn() -> DnfQuery, bool, &'static str);
        let test_cases: Vec<TestCase> = vec![
            (
                || DnfQuery::from_conjunctions(vec![]),
                false,
                "empty query (always false, not true)",
            ),
            (
                || DnfQuery::builder().or(|c| c.and("x", Op::EQ, 1)).build(),
                false,
                "query with condition",
            ),
            (
                || DnfQuery::from_conjunctions(vec![Conjunction::from_conditions(vec![])]),
                true,
                "query with empty conjunction",
            ),
            (
                || {
                    DnfQuery::from_conjunctions(vec![
                        Conjunction::from_conditions(vec![]),
                        Conjunction::from_conditions(vec![Condition::new("x", Op::EQ, 1)]),
                    ])
                },
                true,
                "one empty + one non-empty conjunction",
            ),
        ];

        for (build_fn, expected, desc) in test_cases {
            let query = build_fn();
            assert_eq!(query.is_always_true(), expected, "Failed: {}", desc);
        }
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_unicode_field_names() {
        let query = DnfQuery::builder()
            .or(|c| c.and("名前", Op::EQ, "太郎"))
            .or(|c| c.and("émoji", Op::EQ, "🎉"))
            .build();

        assert!(query.uses_field("名前"));
        assert!(query.uses_field("émoji"));
        assert_eq!(query.condition_count(), 2);
    }

    #[test]
    fn test_empty_string_values() {
        let query = DnfQuery::builder()
            .or(|c| c.and("name", Op::EQ, ""))
            .build();

        assert_eq!(query.to_string(), "(name == \"\")");
    }

    #[test]
    fn test_special_characters_in_strings() {
        let query = DnfQuery::builder()
            .or(|c| c.and("text", Op::EQ, "hello \"world\""))
            .build();

        // Should contain escaped quotes
        let display = query.to_string();
        assert!(display.contains("hello"));
    }

    #[test]
    fn test_boundary_numeric_values() {
        let query = DnfQuery::builder()
            .or(|c| c.and("max_i64", Op::EQ, i64::MAX))
            .or(|c| c.and("min_i64", Op::EQ, i64::MIN))
            .or(|c| c.and("max_u64", Op::EQ, u64::MAX))
            .or(|c| c.and("zero", Op::EQ, 0i64))
            .build();

        assert_eq!(query.condition_count(), 4);
        assert!(query.uses_field("max_i64"));
        assert!(query.uses_field("min_i64"));
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_condition() {
        let condition = Condition::new("age", Op::GT, Value::Int(18));

        // Serialize
        let json = serde_json::to_string(&condition).unwrap();
        println!("Serialized Condition: {}", json);

        // Deserialize
        let deserialized: Condition = serde_json::from_str(&json).unwrap();
        assert_eq!(condition, deserialized);
        assert_eq!(deserialized.field_name(), "age");
        assert_eq!(deserialized.operator(), &Op::GT);
        assert_eq!(deserialized.value(), &Value::Int(18));
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_conjunction() {
        let conj = Conjunction::from_conditions(vec![
            Condition::new("age", Op::GT, Value::Int(18)),
            Condition::new("country", Op::EQ, Value::from("US")),
        ]);

        // Serialize
        let json = serde_json::to_string(&conj).unwrap();
        println!("Serialized Conjunction: {}", json);

        // Deserialize
        let deserialized: Conjunction = serde_json::from_str(&json).unwrap();
        assert_eq!(conj, deserialized);
        assert_eq!(deserialized.conditions().len(), 2);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_dnf_query() {
        // (age > 18 AND country == "US") OR (premium == true)
        let query = DnfQuery::from_conjunctions(vec![
            Conjunction::from_conditions(vec![
                Condition::new("age", Op::GT, Value::Int(18)),
                Condition::new("country", Op::EQ, Value::from("US")),
            ]),
            Conjunction::from_conditions(vec![Condition::new(
                "premium",
                Op::EQ,
                Value::Bool(true),
            )]),
        ]);

        // Serialize
        let json = serde_json::to_string(&query).unwrap();
        println!("Serialized DnfQuery: {}", json);

        // Deserialize
        let deserialized: DnfQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(query, deserialized);
        assert_eq!(deserialized.conjunctions().len(), 2);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_pretty_json() {
        let query = DnfQuery::builder()
            .or(|c| c.and("age", Op::GTE, 21).and("country", Op::EQ, "US"))
            .or(|c| c.and("premium", Op::EQ, true).and("verified", Op::EQ, true))
            .build();

        // Serialize with pretty print
        let json = serde_json::to_string_pretty(&query).unwrap();
        println!("Pretty JSON:\n{}", json);

        // Deserialize
        let deserialized: DnfQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(query, deserialized);

        // Test evaluation still works after deserialization
        let obj = MockStruct {
            age: 25,
            name: "Alice".to_string(),
            active: true,
        };

        // Should not match because no 'country', 'premium', or 'verified' fields
        assert!(!deserialized.evaluate(&obj));
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_all_value_types() {
        // Test all value types
        let query = DnfQuery::from_conjunctions(vec![Conjunction::from_conditions(vec![
            Condition::new("str_field", Op::EQ, Value::from("test")),
            Condition::new("int_field", Op::EQ, Value::Int(-42)),
            Condition::new("uint_field", Op::EQ, Value::Uint(42)),
            Condition::new("float_field", Op::EQ, Value::Float(3.04)),
            Condition::new("bool_field", Op::EQ, Value::Bool(true)),
        ])]);

        // Serialize and deserialize
        let json = serde_json::to_string_pretty(&query).unwrap();
        println!("All value types JSON:\n{}", json);

        let deserialized: DnfQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(query, deserialized);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_all_operators() {
        // Test all operators
        let query = DnfQuery::from_conjunctions(vec![Conjunction::from_conditions(vec![
            Condition::new("f1", Op::EQ, Value::Int(1)),
            Condition::new("f2", Op::NE, Value::Int(2)),
            Condition::new("f3", Op::GT, Value::Int(3)),
            Condition::new("f4", Op::LT, Value::Int(4)),
            Condition::new("f5", Op::GTE, Value::Int(5)),
            Condition::new("f6", Op::LTE, Value::Int(6)),
            Condition::new("f7", Op::CONTAINS, Value::from("test")),
            Condition::new("f8", Op::NOT_CONTAINS, Value::from("bad")),
            Condition::new("f9", Op::STARTS_WITH, Value::from("start")),
            Condition::new("f10", Op::ENDS_WITH, Value::from("end")),
        ])]);

        // Serialize and deserialize
        let json = serde_json::to_string(&query).unwrap();
        let deserialized: DnfQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(query, deserialized);
    }

    #[test]
    #[cfg(feature = "parser")]
    fn test_display_roundtrip() {
        // Create a complex query using builder
        let query = DnfQuery::builder()
            .or(|c| c.and("age", Op::GT, 18).and("country", Op::EQ, "US"))
            .or(|c| c.and("premium", Op::EQ, true).and("score", Op::GTE, 100.5))
            .build();

        // Convert to string
        let display_str = query.to_string();
        assert_eq!(
            display_str,
            "(age > 18 AND country == \"US\") OR (premium == true AND score >= 100.5)"
        );

        // Define fields for parsing
        let fields = vec![
            crate::FieldInfo::new("age", "i64"),
            crate::FieldInfo::new("country", "String"),
            crate::FieldInfo::new("premium", "bool"),
            crate::FieldInfo::new("score", "f64"),
        ];

        // Parse back and verify equality
        let parsed = crate::parser::parse_with_fields(
            &display_str,
            &fields,
            None::<std::iter::Empty<&str>>,
            None::<std::iter::Empty<&str>>,
        )
        .unwrap();
        assert_eq!(query, parsed);
    }

    #[test]
    #[cfg(feature = "parser")]
    fn test_display_roundtrip_single_condition() {
        let query = DnfQuery::builder()
            .or(|c| c.and("name", Op::EQ, "Alice"))
            .build();

        let display_str = query.to_string();
        assert_eq!(display_str, "(name == \"Alice\")");

        let fields = vec![crate::FieldInfo::new("name", "String")];
        let parsed = crate::parser::parse_with_fields(
            &display_str,
            &fields,
            None::<std::iter::Empty<&str>>,
            None::<std::iter::Empty<&str>>,
        )
        .unwrap();
        assert_eq!(query, parsed);
    }

    #[test]
    #[cfg(feature = "parser")]
    fn test_display_roundtrip_all_types() {
        let query = DnfQuery::builder()
            .or(|c| {
                c.and("int_field", Op::EQ, 42)
                    .and("float_field", Op::EQ, 3.04)
                    .and("bool_field", Op::EQ, true)
                    .and("str_field", Op::EQ, "hello")
            })
            .build();

        let display_str = query.to_string();

        let fields = vec![
            crate::FieldInfo::new("int_field", "i64"),
            crate::FieldInfo::new("float_field", "f64"),
            crate::FieldInfo::new("bool_field", "bool"),
            crate::FieldInfo::new("str_field", "String"),
        ];

        let parsed = crate::parser::parse_with_fields(
            &display_str,
            &fields,
            None::<std::iter::Empty<&str>>,
            None::<std::iter::Empty<&str>>,
        )
        .unwrap();
        assert_eq!(query, parsed);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_json_roundtrip() {
        // Create a complex query using builder
        let query = DnfQuery::builder()
            .or(|c| c.and("age", Op::GT, 18).and("country", Op::EQ, "US"))
            .or(|c| c.and("premium", Op::EQ, true).and("score", Op::GTE, 100.5))
            .build();

        // Serialize to JSON
        let json = serde_json::to_string(&query).unwrap();

        // Deserialize and verify equality
        let deserialized: DnfQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(query, deserialized);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_json_roundtrip_pretty() {
        let query = DnfQuery::builder()
            .or(|c| c.and("name", Op::EQ, "Bob").and("active", Op::EQ, true))
            .build();

        // Serialize to pretty JSON
        let json = serde_json::to_string_pretty(&query).unwrap();

        // Deserialize and verify equality
        let deserialized: DnfQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(query, deserialized);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_json_roundtrip_all_operators() {
        let query = DnfQuery::builder()
            .or(|c| {
                c.and("f1", Op::EQ, 1)
                    .and("f2", Op::NE, 2)
                    .and("f3", Op::GT, 3)
                    .and("f4", Op::LT, 4)
                    .and("f5", Op::GTE, 5)
                    .and("f6", Op::LTE, 6)
                    .and("f7", Op::CONTAINS, "test")
                    .and("f8", Op::NOT_CONTAINS, "bad")
                    .and("f9", Op::STARTS_WITH, "start")
                    .and("f10", Op::ENDS_WITH, "end")
            })
            .build();

        let json = serde_json::to_string(&query).unwrap();
        let deserialized: DnfQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(query, deserialized);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_json_roundtrip_empty_query() {
        let query = DnfQuery::from_conjunctions(vec![]);

        let json = serde_json::to_string(&query).unwrap();
        let deserialized: DnfQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(query, deserialized);
    }

    // ==================== Custom Operator Tests ====================

    // Mock with field_value support
    struct MockWithFieldValue {
        age: i64,
        score: f64,
        name: String,
    }

    impl DnfEvaluable for MockWithFieldValue {
        fn evaluate_field(&self, field_name: &str, operator: &Op, value: &Value) -> bool {
            use crate::DnfField;
            match field_name {
                "age" => self.age.evaluate(operator, value),
                "score" => self.score.evaluate(operator, value),
                "name" => self.name.evaluate(operator, value),
                _ => false,
            }
        }

        fn field_value(&self, field_name: &str) -> Option<Value> {
            match field_name {
                "age" => Some(Value::Int(self.age)),
                "score" => Some(Value::Float(self.score)),
                "name" => Some(Value::from(&self.name)),
                _ => None,
            }
        }

        fn fields() -> impl Iterator<Item = crate::FieldInfo> {
            [
                crate::FieldInfo::new("age", "i64"),
                crate::FieldInfo::new("score", "f64"),
                crate::FieldInfo::new("name", "String"),
            ]
            .into_iter()
        }
    }

    #[test]
    fn test_custom_operator_basic() {
        let user = MockWithFieldValue {
            age: 25,
            score: 85.0,
            name: "Alice".to_string(),
        };

        // Custom operator: IS_ADULT (age >= 18)
        let query = DnfQuery::builder()
            .with_custom_op(
                "IS_ADULT",
                false,
                |field, _| matches!(field, Value::Int(n) if *n >= 18),
            )
            .or(|c| c.and("age", Op::custom("IS_ADULT"), Value::None))
            .build();

        assert!(query.evaluate(&user));
    }

    #[test]
    fn test_custom_operator_with_query_value() {
        let user = MockWithFieldValue {
            age: 25,
            score: 85.0,
            name: "Alice".to_string(),
        };

        // Custom operator: BETWEEN (value is [min, max])
        let between_op = |field: &Value, query: &Value| {
            let Value::FloatArray(range) = query else {
                return false;
            };
            if range.len() < 2 {
                return false;
            }
            match field {
                Value::Float(n) => *n >= range[0] && *n <= range[1],
                Value::Int(n) => (*n as f64) >= range[0] && (*n as f64) <= range[1],
                _ => false,
            }
        };

        let query = DnfQuery::builder()
            .with_custom_op("BETWEEN", false, between_op)
            .or(|c| c.and("score", Op::custom("BETWEEN"), vec![80.0, 100.0]))
            .build();

        assert!(query.evaluate(&user));

        // Out of range
        let query = DnfQuery::builder()
            .with_custom_op("BETWEEN", false, between_op)
            .or(|c| c.and("score", Op::custom("BETWEEN"), vec![90.0, 100.0]))
            .build();

        assert!(!query.evaluate(&user));
    }

    #[test]
    fn test_custom_operator_not() {
        let user = MockWithFieldValue {
            age: 15,
            score: 85.0,
            name: "Bob".to_string(),
        };

        // NOT IS_ADULT (age < 18)
        let query = DnfQuery::builder()
            .with_custom_op(
                "IS_ADULT",
                false,
                |field, _| matches!(field, Value::Int(n) if *n >= 18),
            )
            .or(|c| c.and("age", Op::not_custom("IS_ADULT"), Value::None))
            .build();

        assert!(query.evaluate(&user)); // 15 is NOT adult
    }

    #[test]
    fn test_custom_operator_without_registry() {
        let user = MockWithFieldValue {
            age: 25,
            score: 85.0,
            name: "Alice".to_string(),
        };

        // No custom op registered - should return false
        let query = DnfQuery::builder()
            .or(|c| c.and("age", Op::custom("UNKNOWN_OP"), Value::None))
            .build();

        assert!(!query.evaluate(&user));
    }

    #[test]
    fn test_custom_operator_combined_with_standard() {
        let user = MockWithFieldValue {
            age: 25,
            score: 85.0,
            name: "Alice".to_string(),
        };

        // Mix standard and custom operators
        let query = DnfQuery::builder()
            .with_custom_op(
                "IS_ADULT",
                false,
                |field, _| matches!(field, Value::Int(n) if *n >= 18),
            )
            .or(|c| {
                c.and("name", Op::STARTS_WITH, "Ali") // Standard op
                    .and("age", Op::custom("IS_ADULT"), Value::None) // Custom op
            })
            .build();

        assert!(query.evaluate(&user));
    }

    #[test]
    fn test_custom_operator_registry() {
        let user = MockWithFieldValue {
            age: 25,
            score: 85.0,
            name: "Alice".to_string(),
        };

        // Use a registry
        let mut registry = crate::OpRegistry::new();
        registry.register(
            "IS_ADULT",
            true,
            |field, _| matches!(field, Value::Int(n) if *n >= 18),
        );
        registry.register(
            "IS_PASSING",
            true,
            |field, _| matches!(field, Value::Float(n) if *n >= 60.0),
        );

        let query = DnfQuery::builder()
            .with_custom_ops(registry)
            .or(|c| {
                c.and("age", Op::custom("IS_ADULT"), Value::None).and(
                    "score",
                    Op::custom("IS_PASSING"),
                    Value::None,
                )
            })
            .build();

        assert!(query.evaluate(&user));
    }
}

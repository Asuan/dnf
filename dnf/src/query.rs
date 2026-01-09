use crate::{DnfEvaluable, Op, OpRegistry, Value};
use std::fmt;

/// Represents a single condition in a DNF query.
///
/// A condition checks if a field satisfies a comparison with a value.
///
/// # Example
///
/// ```rust
/// use dnf::{DnfQuery, Op};
///
/// let query = DnfQuery::builder()
///     .or(|c| c.and("age", Op::GT, 18))
///     .build();
///
/// // Access condition details
/// let condition = &query.conjunctions()[0].conditions()[0];
/// assert_eq!(condition.field_name(), "age");
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Condition {
    /// The name of the field to check
    field_name: Box<str>,
    /// The comparison operator
    operator: Op,
    /// The value to compare against
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

    pub fn field_name(&self) -> &str {
        &self.field_name
    }

    pub fn operator(&self) -> &Op {
        &self.operator
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn evaluate<T: DnfEvaluable>(&self, target: &T) -> bool {
        target.evaluate_field(&self.field_name, &self.operator, &self.value)
    }
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.field_name, self.operator, self.value)
    }
}

/// Conjunction (AND) of conditions. Use builder API for construction.
///
/// # Recommended Alternative
///
/// ```rust
/// use dnf::{DnfQuery, Op};
///
/// let query = DnfQuery::builder()
///     .or(|c| c.and("age", Op::GT, 18)
///              .and("country", Op::EQ, "US"))
///     .build();
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Conjunction {
    /// The conditions in this conjunction
    conditions: Vec<Condition>,
}

impl Conjunction {
    /// Create a conjunction from a vector of conditions (internal use only).
    ///
    pub(crate) fn from_conditions(conditions: Vec<Condition>) -> Self {
        Self { conditions }
    }

    pub fn conditions(&self) -> &[Condition] {
        &self.conditions
    }

    /// Returns true if ALL conditions satisfied (empty = true)
    pub fn evaluate<T: DnfEvaluable>(&self, target: &T) -> bool {
        if self.conditions.is_empty() {
            return true;
        }
        self.conditions
            .iter()
            .all(|condition| condition.evaluate(target))
    }

    pub fn len(&self) -> usize {
        self.conditions.len()
    }

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

/// DNF query: OR of AND clauses. Use builder API for construction.
///
/// ```rust
/// use dnf::{DnfQuery, Op};
///
/// let query = DnfQuery::builder()
///     .or(|c| c.and("age", Op::GT, 18)
///              .and("country", Op::EQ, "US"))
///     .or(|c| c.and("premium", Op::EQ, true))
///     .build();
/// ```
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DnfQuery {
    /// The conjunctions in this DNF query
    conjunctions: Vec<Conjunction>,
    /// Custom operator registry (skipped in serialization and comparison)
    #[cfg_attr(feature = "serde", serde(skip))]
    custom_ops: Option<OpRegistry>,
}

impl PartialEq for DnfQuery {
    fn eq(&self, other: &Self) -> bool {
        // Compare only conjunctions, not custom_ops (functions can't be compared)
        self.conjunctions == other.conjunctions
    }
}

impl DnfQuery {
    /// Create a DNF query from a vector of conjunctions (internal use only).
    ///
    /// Use [`DnfQuery::builder()`](DnfQuery::builder) for constructing queries.
    pub(crate) fn from_conjunctions(conjunctions: Vec<Conjunction>) -> Self {
        Self {
            conjunctions,
            custom_ops: None,
        }
    }

    /// Set custom operators (internal use only - called from builder).
    ///
    /// Use [`DnfQuery::builder()`](DnfQuery::builder) for custom_ops.
    pub(crate) fn set_custom_ops(mut self, registry: OpRegistry) -> Self {
        self.custom_ops = Some(registry);
        self
    }

    /// Get the conjunctions in this query.
    pub fn conjunctions(&self) -> &[Conjunction] {
        &self.conjunctions
    }

    /// Take ownership of the conjunctions and custom ops.
    pub(crate) fn into_parts(self) -> (Vec<Conjunction>, Option<OpRegistry>) {
        (self.conjunctions, self.custom_ops)
    }

    /// Take ownership of the conjunctions (internal use).
    #[cfg(feature = "parser")]
    pub(crate) fn into_conjunctions(self) -> Vec<Conjunction> {
        self.conjunctions
    }

    /// Create a new query builder.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::{DnfQuery, Op};
    ///
    /// let query = DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::GT, 18)
    ///              .and("country", Op::EQ, "US"))
    ///     .or(|c| c.and("premium", Op::EQ, true))
    ///     .build();
    /// ```
    pub fn builder() -> crate::builder::QueryBuilder {
        crate::builder::QueryBuilder::new()
    }

    /// Get the custom operator registry, if any.
    pub fn custom_ops(&self) -> Option<&OpRegistry> {
        self.custom_ops.as_ref()
    }

    /// Check if a custom operator is registered.
    pub fn has_custom_op(&self, name: &str) -> bool {
        self.custom_ops.as_ref().is_some_and(|r| r.contains(name))
    }

    /// Validate that all custom operators used in the query are registered.
    ///
    /// Returns `Ok(())` if all custom operators are registered,
    /// or `Err(DnfError::UnregisteredCustomOp)` with the first unregistered operator found.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::{DnfQuery, Op, Value};
    ///
    /// let query = DnfQuery::builder()
    ///     .with_custom_op("IS_ADULT", true, |field, _| {
    ///         matches!(field, Value::Uint(n) if *n >= 18)
    ///     })
    ///     .or(|c| c.and("age", Op::custom("IS_ADULT"), Value::None))
    ///     .build();
    ///
    /// // Validation passes - operator is registered
    /// assert!(query.validate_custom_ops().is_ok());
    ///
    /// let bad_query = DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::custom("IS_SENIOR"), Value::None))
    ///     .build();
    ///
    /// // Validation fails - operator not registered
    /// assert!(bad_query.validate_custom_ops().is_err());
    /// ```
    pub fn validate_custom_ops(&self) -> Result<(), crate::DnfError> {
        for conjunction in &self.conjunctions {
            for condition in &conjunction.conditions {
                if let Some(custom_name) = condition.operator.custom_name() {
                    // Check if operator is registered
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

    /// Evaluate this query against a target object.
    ///
    /// Returns `true` if ANY conjunction is satisfied, `false` otherwise.
    ///
    /// # Custom Operators
    ///
    /// If the query uses custom operators (via `Op::custom("name")`), they are
    /// evaluated using functions registered via `with_custom_op` or `with_custom_ops`.
    /// Unregistered custom operators evaluate to `false`.
    ///
    /// # Performance
    ///
    /// - Uses short-circuit evaluation (stops on first matching conjunction)
    /// - Standard operators use direct comparison (no Value conversion)
    /// - Custom operators convert field values only when evaluated
    pub fn evaluate<T: DnfEvaluable>(&self, target: &T) -> bool {
        self.conjunctions
            .iter()
            .any(|conjunction| self.evaluate_conjunction(conjunction, target))
    }

    /// Evaluate a conjunction against a target.
    fn evaluate_conjunction<T: DnfEvaluable>(&self, conj: &Conjunction, target: &T) -> bool {
        if conj.conditions.is_empty() {
            return true;
        }
        conj.conditions
            .iter()
            .all(|cond| self.evaluate_condition(cond, target))
    }

    /// Evaluate a condition against a target.
    fn evaluate_condition<T: DnfEvaluable>(&self, cond: &Condition, target: &T) -> bool {
        // Check for custom operator
        if let Some(custom_name) = cond.operator.custom_name() {
            if let Some(registry) = &self.custom_ops {
                // Only convert field to Value when we have a custom op with registry
                if let Some(field_value) = target.get_field_value(&cond.field_name) {
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
            // No registry or field not found
            return cond.operator.is_inverse();
        }

        // Standard operators - direct evaluation without Value conversion
        target.evaluate_field(&cond.field_name, &cond.operator, &cond.value)
    }

    /// Returns the number of conjunctions in this query.
    pub fn len(&self) -> usize {
        self.conjunctions.len()
    }

    /// Check if this query has no conjunctions.
    pub fn is_empty(&self) -> bool {
        self.conjunctions.is_empty()
    }

    /// Get all field names used in this query.
    ///
    /// Returns an iterator over field names. Use `.collect::<HashSet<_>>()` for unique names.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::{DnfQuery, Op};
    /// use std::collections::HashSet;
    ///
    /// let query = DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::GT, 18)
    ///              .and("country", Op::EQ, "US"))
    ///     .or(|c| c.and("premium", Op::EQ, true))
    ///     .build();
    ///
    /// let fields: HashSet<_> = query.field_names().collect();
    /// assert!(fields.contains("age"));
    /// assert!(fields.contains("country"));
    /// assert!(fields.contains("premium"));
    /// ```
    pub fn field_names(&self) -> impl Iterator<Item = &str> {
        self.conjunctions
            .iter()
            .flat_map(|conj| conj.conditions().iter().map(|c| c.field_name()))
    }

    /// Get the total number of conditions across all conjunctions.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::{DnfQuery, Op};
    ///
    /// let query = DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::GT, 18)
    ///              .and("country", Op::EQ, "US"))
    ///     .or(|c| c.and("premium", Op::EQ, true))
    ///     .build();
    ///
    /// assert_eq!(query.condition_count(), 3);
    /// ```
    pub fn condition_count(&self) -> usize {
        self.conjunctions.iter().map(|c| c.len()).sum()
    }

    /// Check if this query uses a specific field.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::{DnfQuery, Op};
    ///
    /// let query = DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::GT, 18))
    ///     .build();
    ///
    /// assert!(query.uses_field("age"));
    /// assert!(!query.uses_field("name"));
    /// ```
    pub fn uses_field(&self, name: &str) -> bool {
        self.conjunctions
            .iter()
            .any(|conj| conj.conditions().iter().any(|c| c.field_name() == name))
    }

    /// Check if this query is always false (empty query with no conjunctions).
    ///
    /// An empty DNF query evaluates to false because there are no conjunctions
    /// that could possibly match.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::{DnfQuery, Op};
    ///
    /// let empty = DnfQuery::default();
    /// assert!(empty.is_always_false());
    ///
    /// let non_empty = DnfQuery::builder()
    ///     .or(|c| c.and("x", Op::EQ, 1))
    ///     .build();
    /// assert!(!non_empty.is_always_false());
    /// ```
    pub fn is_always_false(&self) -> bool {
        self.conjunctions.is_empty()
    }

    /// Check if this query has an empty conjunction (always true).
    ///
    /// A query with an empty conjunction (no conditions) always evaluates to true
    /// because an empty conjunction is trivially satisfied.
    ///
    /// Note: The builder API filters out empty conjunctions, so this only occurs
    /// with queries constructed via deserialization or internal APIs.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::{DnfQuery, Op};
    ///
    /// // Normal queries are not always true
    /// let query = DnfQuery::builder()
    ///     .or(|c| c.and("x", Op::EQ, 1))
    ///     .build();
    /// assert!(!query.is_always_true());
    ///
    /// // Empty queries (default) are not always true either
    /// let empty = DnfQuery::default();
    /// assert!(!empty.is_always_true());
    /// ```
    pub fn is_always_true(&self) -> bool {
        self.conjunctions.iter().any(|c| c.is_empty())
    }

    /// Validate that all field names in this query exist in the target type.
    ///
    /// Returns `Ok(self)` if all fields are valid, allowing method chaining.
    /// Returns `Err` with the first unknown field name if validation fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::{DnfEvaluable, DnfQuery, Op};
    ///
    /// #[derive(DnfEvaluable)]
    /// struct User {
    ///     age: u32,
    ///     name: String,
    /// }
    ///
    /// // Valid query
    /// let query = DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::GT, 18))
    ///     .build()
    ///     .validate::<User>()
    ///     .unwrap();
    ///
    /// // Invalid query - unknown field
    /// let result = DnfQuery::builder()
    ///     .or(|c| c.and("unknown", Op::GT, 18))
    ///     .build()
    ///     .validate::<User>();
    /// assert!(result.is_err());
    /// ```
    pub fn validate<T: crate::DnfEvaluable>(self) -> Result<Self, crate::DnfError> {
        use crate::FieldKind;
        use std::collections::HashMap;

        // First, validate custom operators are registered
        self.validate_custom_ops()?;

        let field_info: HashMap<&str, FieldKind> = T::fields().map(|f| (f.name, f.kind)).collect();

        for conj in &self.conjunctions {
            for condition in conj.conditions() {
                let field_name = condition.field_name();
                let value = condition.value();

                // Handle nested fields - check the root field
                let root_field = field_name.split('.').next().unwrap_or(field_name);

                let field_kind =
                    field_info
                        .get(root_field)
                        .ok_or_else(|| crate::DnfError::UnknownField {
                            field_name: field_name.into(),
                            position: 0,
                        })?;

                // Validate map target values are only used with Map fields
                let is_map_value = matches!(
                    value,
                    Value::AtKey(_, _) | Value::Keys(_) | Value::Values(_)
                );

                if is_map_value && *field_kind != FieldKind::Map {
                    return Err(crate::DnfError::InvalidMapTarget {
                        field_name: field_name.into(),
                        field_kind: *field_kind,
                    });
                }
            }
        }

        Ok(self)
    }

    /// Merge another query into this one (OR combination).
    ///
    /// Combines conjunctions from both queries. The result matches if
    /// either original query would match.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::{DnfQuery, Op};
    ///
    /// let adults = DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::GTE, 18))
    ///     .build();
    ///
    /// let premium = DnfQuery::builder()
    ///     .or(|c| c.and("premium", Op::EQ, true))
    ///     .build();
    ///
    /// // (age >= 18) OR (premium == true)
    /// let combined = adults.merge(premium);
    /// assert_eq!(combined.conjunctions().len(), 2);
    /// ```
    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        let (conjunctions, custom_ops) = other.into_parts();
        self.conjunctions.extend(conjunctions);
        // Merge custom ops if other has them
        if let Some(other_ops) = custom_ops {
            match &mut self.custom_ops {
                Some(ops) => ops.merge(other_ops),
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

    // Mock with get_field_value support
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

        fn get_field_value(&self, field_name: &str) -> Option<Value> {
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

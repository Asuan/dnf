use crate::{Condition, Conjunction, DnfQuery, Op, OpRegistry, Value};

/// Builder for constructing DNF queries with a fluent API.
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
#[derive(Default, Debug)]
pub struct QueryBuilder {
    conjunctions: Vec<Conjunction>,
    custom_ops: Option<OpRegistry>,
}

impl QueryBuilder {
    pub(crate) fn new() -> Self {
        Self {
            conjunctions: Vec::new(),
            custom_ops: None,
        }
    }

    /// Parse query string using type's field metadata
    #[cfg(feature = "parser")]
    pub fn from_query<T: crate::DnfEvaluable>(query: &str) -> Result<DnfQuery, crate::DnfError> {
        let fields: Vec<_> = T::fields().collect();
        crate::parser::parse_with_fields(
            query,
            &fields,
            None::<std::iter::Empty<&str>>,
            None::<std::iter::Empty<&str>>,
        )
    }

    /// Parse a query string and add its conditions to this builder.
    ///
    /// Uses any custom operators registered via `with_custom_op()` or `with_custom_ops()`.
    /// Parsed conjunctions are added to existing ones, allowing mixed builder/parser usage.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::{DnfEvaluable, DnfQuery, Op, Value};
    ///
    /// #[derive(DnfEvaluable)]
    /// struct User {
    ///     age: u32,
    ///     premium: bool,
    /// }
    ///
    /// // Mix parsed and builder conditions
    /// let query = DnfQuery::builder()
    ///     .with_custom_op("IS_ADULT", true, |field, _| {
    ///         matches!(field, Value::Uint(n) if *n >= 18)
    ///     })
    ///     .parse::<User>("age IS_ADULT")?
    ///     .or(|c| c.and("premium", Op::EQ, true))  // Add more conditions
    ///     .build();
    /// # Ok::<(), dnf::DnfError>(())
    /// ```
    #[cfg(feature = "parser")]
    pub fn parse<T: crate::DnfEvaluable>(mut self, query: &str) -> Result<Self, crate::DnfError> {
        let fields: Vec<_> = T::fields().collect();
        let custom_op_names = self.custom_ops.as_ref().map(|reg| reg.operator_names());
        let novalue_ops = self.custom_ops.as_ref().map(|reg| reg.novalue_ops());
        let parsed_query =
            crate::parser::parse_with_fields(query, &fields, custom_op_names, novalue_ops)?;

        // Add parsed conjunctions to this builder
        for conj in parsed_query.into_conjunctions() {
            self.conjunctions.push(conj);
        }

        Ok(self)
    }

    /// Add a conjunction (OR clause) to the query using a closure.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::{DnfQuery, Op};
    ///
    /// let query = DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::GT, 18)
    ///              .and("active", Op::EQ, true))
    ///     .build();
    /// ```
    #[must_use]
    pub fn or<F>(mut self, f: F) -> Self
    where
        F: FnOnce(ConjunctionBuilder) -> ConjunctionBuilder,
    {
        let builder = ConjunctionBuilder::default();
        let builder = f(builder);
        self.conjunctions.push(builder.build());
        self
    }

    /// Merge another query's conjunctions into this builder.
    ///
    /// Useful for combining parsed queries with builder-constructed ones.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::{DnfQuery, Op};
    ///
    /// let parsed = DnfQuery::builder()
    ///     .or(|c| c.and("premium", Op::EQ, true))
    ///     .build();
    ///
    /// let query = DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::GT, 18))
    ///     .or_query(parsed)  // Add parsed query's conditions
    ///     .build();
    ///
    /// // Result: (age > 18) OR (premium == true)
    /// assert_eq!(query.conjunctions().len(), 2);
    /// ```
    #[must_use]
    pub fn or_query(mut self, query: DnfQuery) -> Self {
        let (conjunctions, custom_ops) = query.into_parts();
        self.conjunctions.extend(conjunctions);
        // Merge custom ops from the query
        if let Some(other_ops) = custom_ops {
            match &mut self.custom_ops {
                Some(ops) => ops.merge(other_ops),
                None => self.custom_ops = Some(other_ops),
            }
        }
        self
    }

    /// Attach a custom operator registry to the query being built.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::{DnfQuery, Op, Value, OpRegistry};
    ///
    /// let mut registry = OpRegistry::new();
    /// registry.register("IS_ADULT", true, |field, _| {
    ///     matches!(field, Value::Int(n) if *n >= 18)
    /// });
    ///
    /// let query = DnfQuery::builder()
    ///     .with_custom_ops(registry)
    ///     .or(|c| c.and("age", Op::custom("IS_ADULT"), Value::None))
    ///     .build();
    /// ```
    pub fn with_custom_ops(mut self, registry: OpRegistry) -> Self {
        self.custom_ops = Some(registry);
        self
    }

    /// Register a custom operator with optional novalue flag.
    ///
    /// # Arguments
    ///
    /// * `name` - The operator name
    /// * `novalue` - If true, the operator doesn't require a value in the query
    /// * `f` - The evaluation function
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Requires `parser` feature
    /// use dnf::{DnfQuery, DnfEvaluable, Value};
    ///
    /// #[derive(DnfEvaluable)]
    /// struct Data {
    ///     answer: u32,
    ///     age: u32,
    /// }
    ///
    /// // Operator that needs a value
    /// let query = DnfQuery::builder()
    ///     .with_custom_op("EQUALS_ANSWER", false, |field, value| {
    ///         matches!((field, value), (Value::Uint(42), Value::Uint(42)))
    ///     })
    ///     .parse::<Data>("answer EQUALS_ANSWER 42")
    ///     .unwrap();
    ///
    /// // Operator that doesn't need a value
    /// let query2 = DnfQuery::builder()
    ///     .with_custom_op("IS_ADULT", true, |field, _| {
    ///         matches!(field, Value::Uint(n) if *n >= 18)
    ///     })
    ///     .parse::<Data>("age IS_ADULT")  // No value needed!
    ///     .unwrap();
    /// ```
    pub fn with_custom_op<F>(mut self, name: impl Into<Box<str>>, novalue: bool, f: F) -> Self
    where
        F: Fn(&Value, &Value) -> bool + Send + Sync + 'static,
    {
        self.custom_ops
            .get_or_insert_with(OpRegistry::new)
            .register(name, novalue, f);
        self
    }

    /// Validate field names against type T.
    ///
    /// Returns `Ok(self)` if all fields exist, `Err` otherwise.
    /// Call before `build()` to catch typos early.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::{DnfEvaluable, DnfQuery, Op};
    ///
    /// #[derive(DnfEvaluable)]
    /// struct User { age: u32, name: String }
    ///
    /// // Valid fields
    /// let query = DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::GT, 18))
    ///     .validate::<User>()?
    ///     .build();
    ///
    /// // Typo caught early
    /// let result = DnfQuery::builder()
    ///     .or(|c| c.and("agee", Op::GT, 18))  // Typo!
    ///     .validate::<User>();
    /// assert!(result.is_err());
    /// # Ok::<(), dnf::DnfError>(())
    /// ```
    pub fn validate<T: crate::DnfEvaluable>(self) -> Result<Self, crate::DnfError> {
        use crate::FieldKind;
        use std::collections::HashMap;

        // Validate custom operators
        for conj in &self.conjunctions {
            for cond in conj.conditions() {
                if let Some(custom_name) = cond.operator().custom_name() {
                    let registered = self
                        .custom_ops
                        .as_ref()
                        .is_some_and(|r| r.contains(custom_name));
                    if !registered {
                        return Err(crate::DnfError::UnregisteredCustomOp {
                            operator_name: custom_name.into(),
                        });
                    }
                }
            }
        }

        // Validate field names and map targets
        let field_info: HashMap<&str, FieldKind> = T::fields().map(|f| (f.name, f.kind)).collect();

        for conj in &self.conjunctions {
            for cond in conj.conditions() {
                let field_name = cond.field_name();
                let value = cond.value();

                let root_field = field_name.split('.').next().unwrap_or(field_name);

                let field_kind =
                    field_info
                        .get(root_field)
                        .ok_or_else(|| crate::DnfError::UnknownField {
                            field_name: field_name.into(),
                            position: 0,
                        })?;

                let is_map_value = matches!(
                    value,
                    crate::Value::AtKey(_, _) | crate::Value::Keys(_) | crate::Value::Values(_)
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

    /// Build the final DnfQuery.
    ///
    /// Empty conjunctions (those with no conditions) are filtered out.
    #[must_use]
    pub fn build(self) -> DnfQuery {
        let conjunctions = self
            .conjunctions
            .into_iter()
            .filter(|c| !c.is_empty())
            .collect();
        let mut query = DnfQuery::from_conjunctions(conjunctions);
        if let Some(custom_ops) = self.custom_ops {
            query = query.set_custom_ops(custom_ops);
        }
        query
    }
}

/// Builder for constructing a conjunction (AND clause).
///
/// This is typically used within a closure passed to `QueryBuilder::or()`.
///
/// **Note:** This type is an implementation detail of the builder API.
/// Users receive it as a parameter in closures but don't construct it directly.
#[doc(hidden)]
#[derive(Default)]
pub struct ConjunctionBuilder {
    conditions: Vec<Condition>,
}

impl ConjunctionBuilder {
    /// Add a condition using field name, operator, and value directly.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::{DnfQuery, Op};
    ///
    /// let query = DnfQuery::builder()
    ///     .or(|c| c.and("age", Op::GT, 18)
    ///                .and("country", Op::EQ, "US"))
    ///     .build();
    /// ```
    #[must_use]
    pub fn and(
        mut self,
        field_name: impl Into<Box<str>>,
        operator: Op,
        value: impl Into<Value>,
    ) -> Self {
        self.conditions
            .push(Condition::new(field_name, operator, value));
        self
    }

    pub(crate) fn build(self) -> Conjunction {
        Conjunction::from_conditions(self.conditions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Op;

    // ==================== Data-Driven Structure Tests ====================

    /// Test case for query builder structure validation
    struct BuilderStructureCase {
        name: &'static str,
        build_fn: fn() -> DnfQuery,
        expected_conjunctions: usize,
        expected_conditions_per_conjunction: Vec<usize>,
    }

    #[test]
    fn test_builder_structure() {
        let cases = vec![
            BuilderStructureCase {
                name: "empty query",
                build_fn: || QueryBuilder::new().build(),
                expected_conjunctions: 0,
                expected_conditions_per_conjunction: vec![],
            },
            BuilderStructureCase {
                name: "single condition",
                build_fn: || QueryBuilder::new().or(|c| c.and("age", Op::GT, 18)).build(),
                expected_conjunctions: 1,
                expected_conditions_per_conjunction: vec![1],
            },
            BuilderStructureCase {
                name: "two conditions in one conjunction (AND)",
                build_fn: || {
                    QueryBuilder::new()
                        .or(|c| c.and("age", Op::GT, 18).and("country", Op::EQ, "US"))
                        .build()
                },
                expected_conjunctions: 1,
                expected_conditions_per_conjunction: vec![2],
            },
            BuilderStructureCase {
                name: "two conjunctions (OR)",
                build_fn: || {
                    QueryBuilder::new()
                        .or(|c| c.and("age", Op::GT, 18))
                        .or(|c| c.and("premium", Op::EQ, true))
                        .build()
                },
                expected_conjunctions: 2,
                expected_conditions_per_conjunction: vec![1, 1],
            },
            BuilderStructureCase {
                name: "complex: (A AND B) OR (C AND D)",
                build_fn: || {
                    QueryBuilder::new()
                        .or(|c| c.and("age", Op::GT, 18).and("country", Op::EQ, "US"))
                        .or(|c| c.and("premium", Op::EQ, true).and("verified", Op::EQ, true))
                        .build()
                },
                expected_conjunctions: 2,
                expected_conditions_per_conjunction: vec![2, 2],
            },
            BuilderStructureCase {
                name: "many conditions in single conjunction",
                build_fn: || {
                    QueryBuilder::new()
                        .or(|c| {
                            c.and("a", Op::EQ, 1)
                                .and("b", Op::EQ, 2)
                                .and("c", Op::EQ, 3)
                                .and("d", Op::EQ, 4)
                                .and("e", Op::EQ, 5)
                        })
                        .build()
                },
                expected_conjunctions: 1,
                expected_conditions_per_conjunction: vec![5],
            },
            BuilderStructureCase {
                name: "many conjunctions with single condition each",
                build_fn: || {
                    QueryBuilder::new()
                        .or(|c| c.and("a", Op::EQ, 1))
                        .or(|c| c.and("b", Op::EQ, 2))
                        .or(|c| c.and("c", Op::EQ, 3))
                        .or(|c| c.and("d", Op::EQ, 4))
                        .or(|c| c.and("e", Op::EQ, 5))
                        .build()
                },
                expected_conjunctions: 5,
                expected_conditions_per_conjunction: vec![1, 1, 1, 1, 1],
            },
            BuilderStructureCase {
                name: "empty conjunction (filtered out)",
                build_fn: || QueryBuilder::new().or(|c| c).build(),
                expected_conjunctions: 0,
                expected_conditions_per_conjunction: vec![],
            },
        ];

        for case in cases {
            let query = (case.build_fn)();

            assert_eq!(
                query.conjunctions().len(),
                case.expected_conjunctions,
                "Failed '{}': conjunction count mismatch",
                case.name
            );

            for (i, &expected_count) in case.expected_conditions_per_conjunction.iter().enumerate()
            {
                assert_eq!(
                    query.conjunctions()[i].conditions().len(),
                    expected_count,
                    "Failed '{}': condition count mismatch in conjunction {}",
                    case.name,
                    i
                );
            }
        }
    }

    // ==================== Operator Coverage Tests ====================

    #[test]
    fn test_builder_all_operators() {
        // Test that all operators work correctly with the builder
        let test_cases: Vec<(&str, Op, Value)> = vec![
            ("eq", Op::EQ, Value::Int(1)),
            ("ne", Op::NE, Value::Int(2)),
            ("gt", Op::GT, Value::Int(3)),
            ("lt", Op::LT, Value::Int(4)),
            ("gte", Op::GTE, Value::Int(5)),
            ("lte", Op::LTE, Value::Int(6)),
            ("contains", Op::CONTAINS, Value::from("test")),
            ("not_contains", Op::NOT_CONTAINS, Value::from("bad")),
            ("starts_with", Op::STARTS_WITH, Value::from("hello")),
            ("not_starts_with", Op::NOT_STARTS_WITH, Value::from("x")),
            ("ends_with", Op::ENDS_WITH, Value::from("world")),
            ("not_ends_with", Op::NOT_ENDS_WITH, Value::from("y")),
            ("all_of", Op::ALL_OF, Value::from(vec![1, 2, 3])),
            ("any_of", Op::ANY_OF, Value::from(vec![4, 5, 6])),
        ];

        for (name, op, value) in test_cases {
            let query = QueryBuilder::new()
                .or(|c| c.and("field", op.clone(), value.clone()))
                .build();

            assert_eq!(
                query.conjunctions().len(),
                1,
                "Failed for operator: {}",
                name
            );
            let condition = &query.conjunctions()[0].conditions()[0];
            assert_eq!(
                condition.field_name(),
                "field",
                "Failed for operator: {}",
                name
            );
            assert_eq!(condition.operator(), &op, "Failed for operator: {}", name);
        }
    }

    // ==================== Value Type Tests ====================

    #[test]
    fn test_builder_value_types() {
        type TestCase = (&'static str, fn() -> DnfQuery);

        // Test various value types can be used with the builder
        let test_cases: Vec<TestCase> = vec![
            ("i64", || {
                QueryBuilder::new()
                    .or(|c| c.and("field", Op::EQ, 42i64))
                    .build()
            }),
            ("i32", || {
                QueryBuilder::new()
                    .or(|c| c.and("field", Op::EQ, 42i32))
                    .build()
            }),
            ("u64", || {
                QueryBuilder::new()
                    .or(|c| c.and("field", Op::EQ, 42u64))
                    .build()
            }),
            ("u32", || {
                QueryBuilder::new()
                    .or(|c| c.and("field", Op::EQ, 42u32))
                    .build()
            }),
            ("f64", || {
                QueryBuilder::new()
                    .or(|c| c.and("field", Op::EQ, 3.34f64))
                    .build()
            }),
            ("f32", || {
                QueryBuilder::new()
                    .or(|c| c.and("field", Op::EQ, 3.34f32))
                    .build()
            }),
            ("bool true", || {
                QueryBuilder::new()
                    .or(|c| c.and("field", Op::EQ, true))
                    .build()
            }),
            ("bool false", || {
                QueryBuilder::new()
                    .or(|c| c.and("field", Op::EQ, false))
                    .build()
            }),
            ("&str", || {
                QueryBuilder::new()
                    .or(|c| c.and("field", Op::EQ, "hello"))
                    .build()
            }),
            ("String", || {
                QueryBuilder::new()
                    .or(|c| c.and("field", Op::EQ, String::from("hello")))
                    .build()
            }),
            ("empty string", || {
                QueryBuilder::new()
                    .or(|c| c.and("field", Op::EQ, ""))
                    .build()
            }),
            ("Vec<i64>", || {
                QueryBuilder::new()
                    .or(|c| c.and("field", Op::ANY_OF, vec![1i64, 2, 3]))
                    .build()
            }),
            ("Vec<&str>", || {
                QueryBuilder::new()
                    .or(|c| c.and("field", Op::ANY_OF, vec!["a", "b", "c"]))
                    .build()
            }),
        ];

        for (type_name, build_fn) in test_cases {
            let query = build_fn();
            assert_eq!(
                query.conjunctions().len(),
                1,
                "Failed for type: {}",
                type_name
            );
            assert_eq!(
                query.conjunctions()[0].conditions().len(),
                1,
                "Failed for type: {}",
                type_name
            );
        }
    }

    // ==================== Field Name Edge Cases ====================

    #[test]
    fn test_builder_field_name_edge_cases() {
        let test_cases: Vec<(&str, &str)> = vec![
            ("simple", "age"),
            ("underscore", "user_name"),
            ("camelCase", "userName"),
            ("with numbers", "field123"),
            ("single char", "x"),
            ("unicode", "名前"),
            ("emoji", "🎉"),
            ("dots (nested)", "user.address.city"),
            ("spaces", "field with spaces"),
            ("empty string", ""),
        ];

        for (name, field_name) in test_cases {
            let query = QueryBuilder::new()
                .or(|c| c.and(field_name, Op::EQ, 1))
                .build();

            assert_eq!(
                query.conjunctions()[0].conditions()[0].field_name(),
                field_name,
                "Failed for field name case: {}",
                name
            );
        }
    }

    // ==================== Condition Content Verification ====================

    #[test]
    fn test_builder_condition_content() {
        // Test integer condition
        let query = QueryBuilder::new()
            .or(|c| c.and("age", Op::GT, 18i64))
            .build();
        let cond = &query.conjunctions()[0].conditions()[0];
        assert_eq!(cond.field_name(), "age");
        assert_eq!(cond.operator(), &Op::GT);
        assert_eq!(cond.value(), &Value::Int(18));

        // Test string condition
        let query = QueryBuilder::new()
            .or(|c| c.and("name", Op::EQ, "Alice"))
            .build();
        let cond = &query.conjunctions()[0].conditions()[0];
        assert_eq!(cond.field_name(), "name");
        assert_eq!(cond.operator(), &Op::EQ);
        assert_eq!(cond.value(), &Value::from("Alice"));

        // Test bool condition
        let query = QueryBuilder::new()
            .or(|c| c.and("active", Op::NE, false))
            .build();
        let cond = &query.conjunctions()[0].conditions()[0];
        assert_eq!(cond.field_name(), "active");
        assert_eq!(cond.operator(), &Op::NE);
        assert_eq!(cond.value(), &Value::Bool(false));

        // Test multiple conditions preserve order
        let query = QueryBuilder::new()
            .or(|c| {
                c.and("a", Op::EQ, 1i64)
                    .and("b", Op::GT, 2i64)
                    .and("c", Op::LT, 3i64)
            })
            .build();
        let conditions = query.conjunctions()[0].conditions();
        assert_eq!(conditions[0].field_name(), "a");
        assert_eq!(conditions[1].field_name(), "b");
        assert_eq!(conditions[2].field_name(), "c");
    }

    // ==================== Custom Operator Tests ====================

    #[test]
    fn test_builder_with_custom_ops() {
        use crate::{DnfEvaluable, FieldInfo};

        // Define test struct
        struct TestData {
            age: i64,
            score: i64,
        }

        impl DnfEvaluable for TestData {
            fn evaluate_field(&self, field_name: &str, operator: &Op, value: &Value) -> bool {
                use crate::DnfField;
                match field_name {
                    "age" => self.age.evaluate(operator, value),
                    "score" => self.score.evaluate(operator, value),
                    _ => false,
                }
            }

            fn get_field_value(&self, field_name: &str) -> Option<Value> {
                match field_name {
                    "age" => Some(Value::Int(self.age)),
                    "score" => Some(Value::Int(self.score)),
                    _ => None,
                }
            }

            fn fields() -> impl Iterator<Item = FieldInfo> {
                [FieldInfo::new("age", "i64"), FieldInfo::new("score", "i64")].into_iter()
            }
        }

        // Test with_custom_ops using registry
        let mut registry = OpRegistry::new();
        registry.register(
            "IS_ADULT",
            true,
            |field, _| matches!(field, Value::Int(n) if *n >= 18),
        );

        let query = DnfQuery::builder()
            .with_custom_ops(registry)
            .or(|c| c.and("age", Op::custom("IS_ADULT"), Value::None))
            .build();

        let adult = TestData {
            age: 25,
            score: 100,
        };
        let child = TestData { age: 10, score: 90 };

        assert!(query.evaluate(&adult), "Adult should match IS_ADULT");
        assert!(!query.evaluate(&child), "Child should not match IS_ADULT");

        // Test with_custom_op (single operator)
        let query = DnfQuery::builder()
            .with_custom_op(
                "HIGH_SCORE",
                false,
                |field, query| matches!((field, query), (Value::Int(f), Value::Int(q)) if f >= q),
            )
            .or(|c| c.and("score", Op::custom("HIGH_SCORE"), 95))
            .build();

        assert!(
            query.evaluate(&adult),
            "Adult with score 100 should match HIGH_SCORE >= 95"
        );
        assert!(
            !query.evaluate(&child),
            "Child with score 90 should not match HIGH_SCORE >= 95"
        );

        // Test multiple custom operators
        let query = DnfQuery::builder()
            .with_custom_op(
                "IS_ADULT",
                false,
                |field, _| matches!(field, Value::Int(n) if *n >= 18),
            )
            .with_custom_op(
                "HIGH_SCORE",
                false,
                |field, query| matches!((field, query), (Value::Int(f), Value::Int(q)) if f >= q),
            )
            .or(|c| c.and("age", Op::custom("IS_ADULT"), Value::None))
            .or(|c| c.and("score", Op::custom("HIGH_SCORE"), 95))
            .build();

        assert!(
            query.evaluate(&adult),
            "Adult should match (adult OR high score)"
        );
        assert!(
            !query.evaluate(&child),
            "Child should not match (adult OR high score)"
        );

        // Test custom ops with standard ops
        let query = DnfQuery::builder()
            .with_custom_op(
                "IS_ADULT",
                false,
                |field, _| matches!(field, Value::Int(n) if *n >= 18),
            )
            .or(|c| {
                c.and("age", Op::custom("IS_ADULT"), Value::None)
                    .and("score", Op::GT, 80)
            })
            .build();

        let young_high_scorer = TestData { age: 16, score: 95 };

        assert!(
            query.evaluate(&adult),
            "Adult with score 100 should match (IS_ADULT AND score > 80)"
        );
        assert!(
            !query.evaluate(&young_high_scorer),
            "Young high scorer should not match (IS_ADULT AND score > 80)"
        );
    }

    // ==================== Parser Integration (feature-gated) ====================

    #[cfg(feature = "parser")]
    mod parser_tests {
        use super::*;
        use crate::{DnfEvaluable, FieldInfo, Op, Value};

        struct TestUser {
            age: u32,
            name: String,
            active: bool,
        }

        impl DnfEvaluable for TestUser {
            fn evaluate_field(&self, field_name: &str, operator: &Op, value: &Value) -> bool {
                use crate::DnfField;
                match field_name {
                    "age" => self.age.evaluate(operator, value),
                    "name" => self.name.evaluate(operator, value),
                    "active" => self.active.evaluate(operator, value),
                    _ => false,
                }
            }

            fn fields() -> impl Iterator<Item = FieldInfo> {
                [
                    FieldInfo::new("age", "u32"),
                    FieldInfo::new("name", "String"),
                    FieldInfo::new("active", "bool"),
                ]
                .into_iter()
            }
        }

        #[test]
        fn test_from_query() {
            let test_cases = vec![
                ("age > 18", 1, vec![1]),
                ("age > 18 AND name == \"Alice\"", 1, vec![2]),
                ("age > 18 OR active == true", 2, vec![1, 1]),
                (
                    "(age > 18 AND name == \"Alice\") OR active == true",
                    2,
                    vec![2, 1],
                ),
            ];

            for (query_str, expected_conj, expected_conds) in test_cases {
                let query = QueryBuilder::from_query::<TestUser>(query_str)
                    .unwrap_or_else(|e| panic!("Failed to parse '{}': {:?}", query_str, e));

                assert_eq!(
                    query.conjunctions().len(),
                    expected_conj,
                    "Conjunction count mismatch for: {}",
                    query_str
                );

                for (i, &expected_count) in expected_conds.iter().enumerate() {
                    assert_eq!(
                        query.conjunctions()[i].conditions().len(),
                        expected_count,
                        "Condition count mismatch for '{}' conjunction {}",
                        query_str,
                        i
                    );
                }
            }
        }

        #[test]
        fn test_from_query_errors() {
            let error_cases = vec![
                ("unknown_field > 18", "unknown field"),
                ("age > \"not a number\"", "type mismatch"),
                ("age >", "incomplete expression"),
                ("", "empty query"),
            ];

            for (query_str, desc) in error_cases {
                let result = QueryBuilder::from_query::<TestUser>(query_str);
                assert!(
                    result.is_err(),
                    "Expected error for '{}' ({}), got: {:?}",
                    query_str,
                    desc,
                    result
                );
            }
        }
    }

    // ==================== Builder Validate Tests ====================

    mod validate_tests {
        use super::*;
        use crate::{DnfEvaluable, FieldInfo, FieldKind};

        struct TestDoc {
            title: String,
            count: u32,
        }

        impl DnfEvaluable for TestDoc {
            fn evaluate_field(&self, field_name: &str, operator: &Op, value: &Value) -> bool {
                use crate::DnfField;
                match field_name {
                    "title" => self.title.evaluate(operator, value),
                    "count" => self.count.evaluate(operator, value),
                    _ => false,
                }
            }

            fn fields() -> impl Iterator<Item = FieldInfo> {
                [
                    FieldInfo::new("title", "String"),
                    FieldInfo::new("count", "u32"),
                ]
                .into_iter()
            }
        }

        #[test]
        fn test_builder_validate_fields() {
            // (field, expected_ok, description)
            let test_cases: Vec<(&str, bool, &str)> = vec![
                ("title", true, "valid field"),
                ("count", true, "valid field"),
                ("unknown", false, "unknown field"),
                ("titl", false, "typo in field"),
            ];

            for (field, expected_ok, desc) in test_cases {
                let result = QueryBuilder::new()
                    .or(|c| c.and(field, Op::EQ, "test"))
                    .validate::<TestDoc>();

                assert_eq!(result.is_ok(), expected_ok, "Failed: {}", desc);
            }
        }

        #[test]
        fn test_builder_validate_custom_ops() {
            // (has_registration, expected_ok, description)
            let test_cases: Vec<(bool, bool, &str)> = vec![
                (true, true, "registered custom op"),
                (false, false, "unregistered custom op"),
            ];

            for (has_registration, expected_ok, desc) in test_cases {
                let mut builder = QueryBuilder::new();

                if has_registration {
                    builder = builder.with_custom_op("CUSTOM", true, |_, _| true);
                }

                let result = builder
                    .or(|c| c.and("title", Op::custom("CUSTOM"), Value::None))
                    .validate::<TestDoc>();

                assert_eq!(result.is_ok(), expected_ok, "Failed: {}", desc);
            }
        }

        #[test]
        fn test_builder_validate_chaining() {
            // Validate returns Self for chaining
            let query = QueryBuilder::new()
                .or(|c| c.and("title", Op::EQ, "test"))
                .validate::<TestDoc>()
                .expect("should validate")
                .or(|c| c.and("count", Op::GT, 0)) // Can add more after validate
                .build();

            assert_eq!(query.conjunctions().len(), 2);
        }

        #[test]
        fn test_builder_validate_map_target_on_non_map() {
            // Map target on non-map field should fail
            let result = QueryBuilder::new()
                .or(|c| c.and("title", Op::EQ, Value::at_key("k", "v")))
                .validate::<TestDoc>();

            assert!(result.is_err());
            match result.unwrap_err() {
                crate::DnfError::InvalidMapTarget {
                    field_name,
                    field_kind,
                } => {
                    assert_eq!(&*field_name, "title");
                    assert_eq!(field_kind, FieldKind::Scalar);
                }
                err => panic!("Expected InvalidMapTarget, got {:?}", err),
            }
        }

        #[test]
        fn test_builder_or_query() {
            // (query1_conjs, query2_conjs, expected_total)
            let test_cases: Vec<(usize, usize, usize)> = vec![
                (1, 1, 2), // 1 + 1 = 2
                (2, 1, 3), // 2 + 1 = 3
                (0, 2, 2), // 0 + 2 = 2
                (3, 0, 3), // 3 + 0 = 3
            ];

            for (q1_count, q2_count, expected) in test_cases {
                let mut builder1 = QueryBuilder::new();
                for i in 0..q1_count {
                    builder1 = builder1.or(|c| c.and(format!("field{}", i), Op::EQ, i as i64));
                }

                let mut builder2 = QueryBuilder::new();
                for i in 0..q2_count {
                    builder2 =
                        builder2.or(|c| c.and(format!("other{}", i), Op::EQ, (i + 10) as i64));
                }
                let query2 = builder2.build();

                let combined = builder1.or_query(query2).build();

                assert_eq!(
                    combined.conjunctions().len(),
                    expected,
                    "Failed: {} + {} = {}",
                    q1_count,
                    q2_count,
                    expected
                );
            }
        }

        #[test]
        fn test_builder_or_query_with_validate() {
            // Chain: or().or_query().validate().build()
            let parsed = QueryBuilder::new()
                .or(|c| c.and("count", Op::GT, 0))
                .build();

            let query = QueryBuilder::new()
                .or(|c| c.and("title", Op::EQ, "test"))
                .or_query(parsed)
                .validate::<TestDoc>()
                .expect("should validate")
                .build();

            assert_eq!(query.conjunctions().len(), 2);
        }
    }

    // ==================== Merge Tests ====================

    mod merge_tests {
        use super::*;

        #[test]
        fn test_query_merge() {
            // (q1_conjs, q2_conjs, expected_total)
            let test_cases: Vec<(usize, usize, usize)> =
                vec![(1, 1, 2), (2, 3, 5), (0, 1, 1), (1, 0, 1)];

            for (q1_count, q2_count, expected) in test_cases {
                let mut builder1 = QueryBuilder::new();
                for i in 0..q1_count {
                    builder1 = builder1.or(|c| c.and(format!("a{}", i), Op::EQ, i as i64));
                }
                let q1 = builder1.build();

                let mut builder2 = QueryBuilder::new();
                for i in 0..q2_count {
                    builder2 = builder2.or(|c| c.and(format!("b{}", i), Op::EQ, i as i64));
                }
                let q2 = builder2.build();

                let merged = q1.merge(q2);

                assert_eq!(
                    merged.conjunctions().len(),
                    expected,
                    "Failed: {} + {} = {}",
                    q1_count,
                    q2_count,
                    expected
                );
            }
        }

        #[test]
        fn test_query_merge_preserves_custom_ops() {
            let q1 = QueryBuilder::new()
                .with_custom_op("OP1", true, |_, _| true)
                .or(|c| c.and("x", Op::custom("OP1"), Value::None))
                .build();

            let q2 = QueryBuilder::new()
                .with_custom_op("OP2", true, |_, _| false)
                .or(|c| c.and("y", Op::custom("OP2"), Value::None))
                .build();

            let merged = q1.merge(q2);

            assert!(merged.has_custom_op("OP1"));
            assert!(merged.has_custom_op("OP2"));
        }
    }
}

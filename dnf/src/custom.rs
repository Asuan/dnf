//! Custom operator support for DNF queries.
//!
//! # Example
//!
//! ```rust
//! use dnf::{DnfQuery, Op, Value, OpRegistry};
//!
//! let query = DnfQuery::builder()
//!     .with_custom_op("IS_ADULT", false, |field, _| matches!(field, Value::Int(n) if *n >= 18))
//!     .or(|c| c.and("age", Op::custom("IS_ADULT"), Value::None))
//!     .build();
//!
//! // Or create a registry: let mut reg = OpRegistry::new(); reg.register(...); builder.with_custom_ops(reg)
//! ```

use crate::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Type alias for custom operator evaluation functions.
///
/// The function receives:
/// - `field_value`: The field's value converted to `Value`
/// - `query_value`: The value specified in the query condition
///
/// Returns `true` if the condition matches, `false` otherwise.
pub type CustomOpFn = Arc<dyn Fn(&Value, &Value) -> bool + Send + Sync>;

/// Registry of custom operators.
///
/// Thread-safe (`Clone` + `Arc` internally). Returns `false` for unknown operators.
///
/// # Example
///
/// ```rust
/// use dnf::{OpRegistry, Value};
///
/// let mut registry = OpRegistry::new();
/// registry.register("IS_EMPTY", true, |field, _| {
///     matches!(field, Value::String(s) if s.is_empty())
/// });
/// ```
#[derive(Clone, Default)]
pub struct OpRegistry {
    ops: HashMap<Box<str>, CustomOpFn>,
    novalue_ops: HashSet<Box<str>>, // Operators that don't need a value
}

impl OpRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a custom operator.
    ///
    /// # Arguments
    ///
    /// * `name` - The operator name
    /// * `novalue` - If true, the operator doesn't require a value in the query
    /// * `f` - The evaluation function
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::{OpRegistry, Value};
    ///
    /// let mut registry = OpRegistry::new();
    ///
    /// // Operator that needs a value
    /// registry.register("BETWEEN", false, |field, range| {
    ///     // ... comparison logic
    ///     true
    /// });
    ///
    /// // Operator that doesn't need a value
    /// registry.register("IS_ADULT", true, |field, _| {
    ///     matches!(field, Value::Uint(n) if *n >= 18)
    /// });
    /// // Usage: "age IS_ADULT" (no value needed)
    /// ```
    pub fn register<F>(&mut self, name: impl Into<Box<str>>, novalue: bool, f: F) -> &mut Self
    where
        F: Fn(&Value, &Value) -> bool + Send + Sync + 'static,
    {
        let name = name.into();
        if novalue {
            self.novalue_ops.insert(name.clone());
        } else {
            self.novalue_ops.remove(&name);
        }
        self.ops.insert(name, Arc::new(f));
        self
    }

    /// Evaluate a custom operator. Returns `Some(result)` if found, `None` otherwise.
    pub fn evaluate(&self, name: &str, field_value: &Value, query_value: &Value) -> Option<bool> {
        self.ops.get(name).map(|f| f(field_value, query_value))
    }

    /// Check if an operator is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.ops.contains_key(name)
    }

    /// Get the number of registered operators.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Get an iterator over operator names.
    pub fn operator_names(&self) -> impl Iterator<Item = &str> {
        self.ops.keys().map(|s| s.as_ref())
    }

    /// Check if an operator is novalue (doesn't require a value in the query).
    pub fn is_novalue(&self, name: &str) -> bool {
        self.novalue_ops.contains(name)
    }

    /// Get an iterator over novalue operator names.
    pub fn novalue_ops(&self) -> impl Iterator<Item = &str> {
        self.novalue_ops.iter().map(|s| s.as_ref())
    }

    /// Merge another registry into this one.
    ///
    /// Operators from `other` overwrite existing operators with the same name.
    pub fn merge(&mut self, other: Self) {
        self.ops.extend(other.ops);
        self.novalue_ops.extend(other.novalue_ops);
    }
}

impl std::fmt::Debug for OpRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpRegistry")
            .field("operators", &self.ops.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_evaluate() {
        let mut registry = OpRegistry::new();

        registry.register(
            "IS_POSITIVE",
            true,
            |field, _| matches!(field, Value::Int(n) if *n > 0),
        );

        assert!(registry.contains("IS_POSITIVE"));
        assert!(!registry.contains("UNKNOWN"));

        let result = registry.evaluate("IS_POSITIVE", &Value::Int(42), &Value::None);
        assert_eq!(result, Some(true));

        let result = registry.evaluate("IS_POSITIVE", &Value::Int(-5), &Value::None);
        assert_eq!(result, Some(false));

        let result = registry.evaluate("UNKNOWN", &Value::Int(42), &Value::None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_between_operator() {
        let mut registry = OpRegistry::new();

        registry.register("BETWEEN", false, |field, query| {
            let Value::FloatArray(range) = query else {
                return false;
            };
            if range.len() < 2 {
                return false;
            }
            match field {
                Value::Int(n) => (*n as f64) >= range[0] && (*n as f64) <= range[1],
                Value::Float(n) => *n >= range[0] && *n <= range[1],
                _ => false,
            }
        });

        let range = Value::from(vec![10.0, 100.0]);

        assert_eq!(
            registry.evaluate("BETWEEN", &Value::Int(50), &range),
            Some(true)
        );
        assert_eq!(
            registry.evaluate("BETWEEN", &Value::Int(5), &range),
            Some(false)
        );
        assert_eq!(
            registry.evaluate("BETWEEN", &Value::Float(50.5), &range),
            Some(true)
        );
    }
}

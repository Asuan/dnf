//! Custom operator support for DNF queries.
//!
//! Custom operators extend a [`DnfQuery`](crate::DnfQuery) with
//! user-supplied predicates that are dispatched through an [`OpRegistry`].
//! Register them via the builder ([`with_custom_op`](crate::QueryBuilder::with_custom_op)
//! or [`with_custom_ops`](crate::QueryBuilder::with_custom_ops)) and refer
//! to them in queries with [`Op::custom`](crate::Op::custom).
//!
//! # Examples
//!
//! ```
//! use dnf::{DnfQuery, Op, Value};
//!
//! let query = DnfQuery::builder()
//!     .with_custom_op("IS_ADULT", true, |field, _| {
//!         matches!(field, Value::Int(n) if *n >= 18)
//!     })
//!     .or(|c| c.and("age", Op::custom("IS_ADULT"), Value::None))
//!     .build();
//!
//! assert!(query.has_custom_op("IS_ADULT"));
//! ```

use crate::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// A reference-counted custom operator evaluation function.
///
/// The closure receives `(field_value, query_value)` and returns `true`
/// when the condition matches. Wrapped in [`Arc`] so registries can be
/// cloned cheaply across queries and threads.
pub type CustomOpFn = Arc<dyn Fn(&Value, &Value) -> bool + Send + Sync>;

/// A thread-safe registry of custom operators.
///
/// Cheap to clone (operators are stored behind [`Arc`]). Looking up an
/// operator that has not been registered returns `None`.
///
/// # Examples
///
/// ```
/// use dnf::{OpRegistry, Value};
///
/// let mut registry = OpRegistry::new();
/// registry.register("IS_EMPTY", true, |field, _| {
///     matches!(field, Value::String(s) if s.is_empty())
/// });
///
/// assert!(registry.contains("IS_EMPTY"));
/// assert_eq!(registry.len(), 1);
/// ```
#[derive(Clone, Default)]
pub struct OpRegistry {
    ops: HashMap<Box<str>, CustomOpFn>,
    novalue_ops: HashSet<Box<str>>,
}

impl OpRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a custom operator under `name`.
    ///
    /// Set `novalue` to `true` for operators that take no right-hand value
    /// in the source query (e.g. `age IS_ADULT` instead of
    /// `age IS_ADULT 1`); the parser then accepts the bare operator name.
    /// Re-registering an existing name replaces the previous entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{OpRegistry, Value};
    ///
    /// let mut registry = OpRegistry::new();
    ///
    /// registry.register("BETWEEN_F64", false, |field, range| {
    ///     match (field, range) {
    ///         (Value::Float(n), Value::FloatArray(r)) if r.len() == 2 => {
    ///             *n >= r[0] && *n <= r[1]
    ///         }
    ///         _ => false,
    ///     }
    /// });
    ///
    /// registry.register("IS_ADULT", true, |field, _| {
    ///     matches!(field, Value::Int(n) if *n >= 18)
    /// });
    ///
    /// assert!(registry.contains("BETWEEN_F64"));
    /// assert!(registry.is_novalue("IS_ADULT"));
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

    /// Evaluates a registered operator against `field_value` and `query_value`.
    ///
    /// Returns [`Some`] with the operator's result if `name` is registered,
    /// or [`None`] otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{OpRegistry, Value};
    ///
    /// let mut registry = OpRegistry::new();
    /// registry.register("IS_POSITIVE", true, |field, _| {
    ///     matches!(field, Value::Int(n) if *n > 0)
    /// });
    ///
    /// assert_eq!(registry.evaluate("IS_POSITIVE", &Value::Int(42), &Value::None), Some(true));
    /// assert_eq!(registry.evaluate("IS_POSITIVE", &Value::Int(-5), &Value::None), Some(false));
    /// assert_eq!(registry.evaluate("UNKNOWN", &Value::Int(0), &Value::None), None);
    /// ```
    pub fn evaluate(&self, name: &str, field_value: &Value, query_value: &Value) -> Option<bool> {
        self.ops.get(name).map(|f| f(field_value, query_value))
    }

    /// Returns `true` if an operator with the given `name` is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.ops.contains_key(name)
    }

    /// Returns the number of registered operators.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Returns `true` if no operators are registered.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Returns an iterator over the names of all registered operators.
    ///
    /// The order is unspecified.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::OpRegistry;
    ///
    /// let mut registry = OpRegistry::new();
    /// registry.register("A", true, |_, _| true);
    /// registry.register("B", true, |_, _| true);
    ///
    /// let mut names: Vec<&str> = registry.operator_names().collect();
    /// names.sort();
    /// assert_eq!(names, vec!["A", "B"]);
    /// ```
    pub fn operator_names(&self) -> impl Iterator<Item = &str> {
        self.ops.keys().map(|s| s.as_ref())
    }

    /// Returns `true` if the operator was registered with `novalue = true`.
    pub fn is_novalue(&self, name: &str) -> bool {
        self.novalue_ops.contains(name)
    }

    /// Returns an iterator over the names of all novalue operators.
    ///
    /// The order is unspecified.
    pub fn novalue_ops(&self) -> impl Iterator<Item = &str> {
        self.novalue_ops.iter().map(|s| s.as_ref())
    }

    /// Merges `other` into this registry, returning `&mut Self` for chaining.
    ///
    /// On name collision, the entry from `other` replaces the existing one.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::OpRegistry;
    ///
    /// let mut a = OpRegistry::new();
    /// a.register("X", true, |_, _| true);
    ///
    /// let mut b = OpRegistry::new();
    /// b.register("Y", false, |_, _| true);
    ///
    /// a.merge(b);
    /// assert!(a.contains("X"));
    /// assert!(a.contains("Y"));
    /// ```
    pub fn merge(&mut self, other: Self) -> &mut Self {
        self.ops.extend(other.ops);
        self.novalue_ops.extend(other.novalue_ops);
        self
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

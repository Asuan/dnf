//! # DNF Query Library
//!
//! Build and evaluate DNF (Disjunctive Normal Form) queries against Rust structs.
//! DNF queries are OR-ed ANDs: `(a AND b) OR (c AND d) OR ...`
//!
//! ## Quick Start
//!
//! ```rust
//! use dnf::{DnfEvaluable, DnfQuery, Op};
//!
//! #[derive(DnfEvaluable)]
//! struct User { age: u32, premium: bool }
//!
//! let query = DnfQuery::builder()
//!     .or(|c| c.and("age", Op::GTE, 18))
//!     .or(|c| c.and("premium", Op::EQ, true))
//!     .build();
//!
//! let user = User { age: 25, premium: false };
//! assert!(query.evaluate(&user));
//! ```
//!
//! ## Validation
//!
//! The builder API doesn't check field names at compile time — typos silently return `false`.
//! Use `validate()` to catch mistakes early:
//!
//! ```rust
//! # use dnf::{DnfEvaluable, DnfQuery, Op};
//! # #[derive(DnfEvaluable)]
//! # struct User { age: u32 }
//! let query = DnfQuery::builder()
//!     .or(|c| c.and("age", Op::GT, 18))
//!     .validate::<User>()   // catches typos before build
//!     .unwrap()
//!     .build();
//! ```
//!
//! Or use the parser — it validates automatically:
//!
//! ```rust,ignore
//! let query = DnfQuery::parse::<User>("age > 18")?;
//! ```
//!
//! ## Features
//!
//! | Feature | What it does |
//! |---------|--------------|
//! | `derive` | `#[derive(DnfEvaluable)]` macro (default) |
//! | `serde` | Serialization support |
//! | `parser` | Parse queries from strings |
//!
//! ## Operators
//!
//! | Category | Operators |
//! |----------|-----------|
//! | Comparison | `EQ` `NE` `GT` `GTE` `LT` `LTE` |
//! | String | `CONTAINS` `STARTS_WITH` `ENDS_WITH` (+ NOT variants) |
//! | Collection | `ANY_OF` `ALL_OF` (+ NOT variants) |
//! | Range | `BETWEEN` `NOT_BETWEEN` |
//! | Custom | `Op::custom("NAME")` |
//!
//! ## Collections & Range
//!
//! ```rust
//! use dnf::{DnfEvaluable, DnfQuery, Op, Value};
//!
//! #[derive(DnfEvaluable)]
//! struct User { tags: Vec<String>, score: f64 }
//!
//! let q = DnfQuery::builder()
//!     .or(|c| c.and("tags", Op::CONTAINS, "rust"))
//!     .or(|c| c.and("score", Op::BETWEEN, vec![60.0, 100.0]))
//!     .build();
//! ```
//!
//! ## Custom Operators
//!
//! ```rust
//! use dnf::{DnfQuery, Op, Value};
//!
//! let q = DnfQuery::builder()
//!     .with_custom_op("IS_ADULT", true, |f, _| matches!(f, Value::Uint(n) if *n >= 18))
//!     .or(|c| c.and("age", Op::custom("IS_ADULT"), Value::None))
//!     .build();
//! ```
//!
//! ## Nested Structs
//!
//! | Field type | Attribute | Query syntax |
//! |------------|-----------|--------------|
//! | Single struct | `#[dnf(nested)]` required | `"address.city"` |
//! | `Vec<T>` | Auto-detected | `"offices.city"` (any match) |
//! | `HashMap` | N/A | `Value::at_key("k", "v")` |
//!
//! ```rust
//! use dnf::{DnfEvaluable, DnfQuery, Op};
//!
//! #[derive(DnfEvaluable)]
//! struct Address { city: String }
//!
//! #[derive(DnfEvaluable)]
//! struct User {
//!     #[dnf(nested)]         // required
//!     address: Address,
//!     offices: Vec<Address>, // auto-detected
//! }
//! ```
//!
//! ## Derive Attributes
//!
//! | Attribute | What it does |
//! |-----------|--------------|
//! | `#[dnf(rename = "x")]` | Use different name in queries |
//! | `#[dnf(skip)]` | Exclude field from queries |
//! | `#[dnf(nested)]` | Enable dot notation for nested struct |
//!
//! ## Map Queries
//!
//! ```rust
//! use dnf::{DnfEvaluable, DnfQuery, Op, Value};
//! use std::collections::HashMap;
//!
//! #[derive(DnfEvaluable)]
//! struct Doc { meta: HashMap<String, String> }
//!
//! let q = DnfQuery::builder()
//!     .or(|c| c.and("meta", Op::EQ, Value::at_key("author", "Alice")))
//!     .build();
//! ```
//!
//! | Operation | Code |
//! |-----------|------|
//! | Key's value | `Value::at_key("key", value)` |
//! | Key exists | `Value::keys("key")` |
//! | Value exists | `Value::values(value)` |
//!
//! ## Supported Types
//!
//! | Category | Types |
//! |----------|-------|
//! | Integers | `i8`–`i64`, `isize`, `u8`–`u64`, `usize` |
//! | Floats | `f32`, `f64` |
//! | Strings | `String`, `&str`, `Box<str>`, `Cow<str>` |
//! | Other | `bool` |
//! | Collections | `Vec<T>`, `HashSet<T>` |
//! | Maps | `HashMap<String, V>`, `BTreeMap<String, V>` |
//! | Wrappers | `Option<T>` |
//!
//! ## Manual Implementation
//!
//! For computed fields or custom logic:
//!
//! ```rust
//! use dnf::{DnfEvaluable, DnfField, FieldInfo, Op, Value};
//!
//! struct Doc { title: String, tags: Vec<String> }
//!
//! impl DnfEvaluable for Doc {
//!     fn evaluate_field(&self, field: &str, op: &Op, value: &Value) -> bool {
//!         match field {
//!             "title" => self.title.evaluate(op, value),
//!             "tag_count" => self.tags.len().evaluate(op, value), // computed
//!             _ => false,
//!         }
//!     }
//!     fn fields() -> impl Iterator<Item = FieldInfo> {
//!         [FieldInfo::new("title", "String"), FieldInfo::new("tag_count", "usize")].into_iter()
//!     }
//! }
//! ```

mod builder;
mod custom;
mod error;
mod field;
mod operator;
#[cfg(feature = "parser")]
pub(crate) mod parser;
mod query;
mod value;

pub use builder::QueryBuilder;
pub use custom::OpRegistry;
pub use error::DnfError;
pub use field::DnfField;
pub use operator::Op;

// Hidden but available for advanced use cases
#[doc(hidden)]
pub use operator::{BaseOperator, ComparisonOrdering};
#[cfg(feature = "parser")]
pub use parser::ParseError;
pub use query::{Condition, Conjunction, DnfQuery};
pub use value::Value;

#[cfg(feature = "derive")]
pub use dnf_derive::DnfEvaluable;

/// The kind of a field for query evaluation purposes.
///
/// This enum helps the derive macro and parser distinguish between
/// different field types that require different evaluation strategies.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FieldKind {
    /// Scalar field (primitives, String, bool)
    #[default]
    Scalar,
    /// Iterator-based field (`Vec<T>`, `HashSet<T>`, etc.) - evaluated with `any()`
    Iter,
    /// `HashMap<K, V>` or `BTreeMap<K, V>` field - supports @keys/@values access
    Map,
}

/// Represents metadata about a field in a struct.
///
/// This is used by the `DnfEvaluable` trait to provide introspection
/// capabilities for queryable fields.
///
/// This type is zero-cost - all fields are static string slices or enums with no allocation.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct FieldInfo {
    /// The name of the field (or renamed name if using #[dnf(rename = "...")])
    pub name: &'static str,

    /// The Rust type of the field as a string (e.g., `"u32"`, `"String"`, `"Option<bool>"`)
    pub field_type: &'static str,

    /// The kind of field for evaluation purposes
    pub kind: FieldKind,
}

impl FieldInfo {
    /// Create a new FieldInfo with default (Scalar) kind
    pub const fn new(name: &'static str, field_type: &'static str) -> Self {
        Self {
            name,
            field_type,
            kind: FieldKind::Scalar,
        }
    }

    /// Create a new FieldInfo with a specific kind
    pub const fn with_kind(name: &'static str, field_type: &'static str, kind: FieldKind) -> Self {
        Self {
            name,
            field_type,
            kind,
        }
    }
}

impl std::fmt::Display for FieldInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.field_type)
    }
}

/// Trait for types that can be evaluated against DNF queries.
///
/// Typically derived via `#[derive(DnfEvaluable)]`. Implement manually for
/// custom evaluation logic or complex types (Vec, HashMap, enums).
///
/// # Example
///
/// ```rust
/// use dnf::DnfEvaluable;
///
/// #[derive(DnfEvaluable)]
/// struct User {
///     age: u32,
///     name: String,
/// }
///
/// // Access field metadata
/// let fields: Vec<_> = User::fields().collect();
/// assert_eq!(fields[0].name, "age");
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be evaluated against DNF queries",
    label = "this type does not implement `DnfEvaluable`",
    note = "use `#[derive(DnfEvaluable)]` for structs with supported field types",
    note = "or implement `DnfEvaluable` manually for custom types"
)]
pub trait DnfEvaluable {
    /// Evaluate a field condition against this instance.
    ///
    /// Returns `true` if the condition matches, `false` otherwise.
    /// Returns `false` for unknown fields or type mismatches.
    fn evaluate_field(&self, field_name: &str, operator: &Op, value: &Value) -> bool;

    /// Get a field's value as `Value` for custom operator evaluation.
    ///
    /// This method is only called when evaluating custom operators.
    /// Standard operators use `evaluate_field` directly without conversion.
    ///
    /// Returns `None` if the field doesn't exist.
    /// Default implementation returns `None` (override in derive macro).
    fn get_field_value(&self, field_name: &str) -> Option<Value> {
        let _ = field_name;
        None
    }

    /// Get metadata about queryable fields.
    ///
    /// Returns an iterator over field names and types. Fields with `#[dnf(skip)]` are excluded.
    /// This method avoids allocation - collect into a `Vec` if needed.
    fn fields() -> impl Iterator<Item = FieldInfo>;
}

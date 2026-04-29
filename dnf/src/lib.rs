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

/// Fluent builder for [`DnfQuery`].
pub use builder::QueryBuilder;
/// Registry of custom operator implementations.
pub use custom::OpRegistry;
/// Unified error type returned by builder, parser, and evaluation APIs.
pub use error::DnfError;
/// Trait implemented by every type that can appear as a field value.
pub use field::DnfField;
/// Operator constructed for a [`Condition`] (e.g. [`Op::EQ`], [`Op::custom`]).
pub use operator::Op;

#[doc(hidden)]
pub use operator::{BaseOperator, ComparisonOrdering};
/// Public query types: a [`Condition`] groups into a [`Conjunction`], conjunctions into a [`DnfQuery`].
pub use query::{Condition, Conjunction, DnfQuery};
/// Typed value used inside a [`Condition`].
pub use value::Value;

/// Derive macro that generates a [`DnfEvaluable`] implementation for a struct.
#[cfg(feature = "derive")]
pub use dnf_derive::DnfEvaluable;

/// Classifies a field by how it must be evaluated against a query.
///
/// Returned as part of [`FieldInfo`] so the parser and validator can reject
/// queries that mix incompatible operators with a field's shape (for example,
/// `@keys` on a scalar).
///
/// # Examples
///
/// ```
/// use dnf::FieldKind;
///
/// assert_eq!(FieldKind::default(), FieldKind::Scalar);
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum FieldKind {
    /// Scalar field — primitives, `String`, `bool`, or any single value.
    #[default]
    Scalar,
    /// Iterator-based field such as `Vec<T>` or `HashSet<T>`; evaluated with `Op::any`.
    Iter,
    /// Map field (`HashMap<K, V>` or `BTreeMap<K, V>`); supports `@keys` / `@values` / `at_key`.
    Map,
}

impl std::fmt::Display for FieldKind {
    /// Renders the kind as a lowercase tag: `scalar`, `iter`, or `map`.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::FieldKind;
    ///
    /// assert_eq!(FieldKind::Scalar.to_string(), "scalar");
    /// assert_eq!(FieldKind::Iter.to_string(), "iter");
    /// assert_eq!(FieldKind::Map.to_string(), "map");
    /// ```
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            FieldKind::Scalar => "scalar",
            FieldKind::Iter => "iter",
            FieldKind::Map => "map",
        };
        f.write_str(s)
    }
}

/// Static metadata about one queryable field on a [`DnfEvaluable`] type.
///
/// All members are `&'static str` or [`FieldKind`], so a `FieldInfo` is `Copy`
/// and can be returned by `const fn`. Construct via [`FieldInfo::new`] or
/// [`FieldInfo::with_kind`]; read via the [`name`](Self::name) /
/// [`field_type`](Self::field_type) / [`kind`](Self::kind) accessors.
///
/// # Examples
///
/// ```
/// use dnf::{FieldInfo, FieldKind};
///
/// let info = FieldInfo::new("age", "u32");
/// assert_eq!(info.name(), "age");
/// assert_eq!(info.field_type(), "u32");
/// assert_eq!(info.kind(), FieldKind::Scalar);
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct FieldInfo {
    name: &'static str,
    field_type: &'static str,
    kind: FieldKind,
}

impl FieldInfo {
    /// Constructs a [`FieldInfo`] with [`FieldKind::Scalar`].
    pub const fn new(name: &'static str, field_type: &'static str) -> Self {
        Self {
            name,
            field_type,
            kind: FieldKind::Scalar,
        }
    }

    /// Constructs a [`FieldInfo`] with an explicit [`FieldKind`].
    pub const fn with_kind(name: &'static str, field_type: &'static str, kind: FieldKind) -> Self {
        Self {
            name,
            field_type,
            kind,
        }
    }

    /// Returns the field's name as it appears in queries
    /// (post-`#[dnf(rename = "…")]`).
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the Rust type of the field as a string,
    /// e.g. `"u32"`, `"String"`, `"Option<bool>"`.
    pub const fn field_type(&self) -> &'static str {
        self.field_type
    }

    /// Returns how the field must be evaluated. See [`FieldKind`].
    pub const fn kind(&self) -> FieldKind {
        self.kind
    }
}

impl std::fmt::Display for FieldInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.field_type)
    }
}

/// Types that can be evaluated against a [`DnfQuery`].
///
/// Normally derived with `#[derive(DnfEvaluable)]`. Implement manually for
/// computed fields, custom value types, or types that the derive macro does not
/// support (enums, types with non-`DnfField` fields, etc.).
///
/// # Examples
///
/// ```
/// use dnf::{DnfEvaluable, DnfQuery, Op};
///
/// #[derive(DnfEvaluable)]
/// struct User { age: u32, name: String }
///
/// let user = User { age: 25, name: "Alice".into() };
/// let query = DnfQuery::builder()
///     .or(|c| c.and("age", Op::GTE, 18))
///     .build();
/// assert!(query.evaluate(&user));
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be evaluated against DNF queries",
    label = "this type does not implement `DnfEvaluable`",
    note = "use `#[derive(DnfEvaluable)]` for structs with supported field types",
    note = "or implement `DnfEvaluable` manually for custom types"
)]
pub trait DnfEvaluable {
    /// Evaluates one condition against `self`.
    ///
    /// Returns `true` if `field_name` exists and the comparison succeeds.
    /// Returns `false` for unknown fields or type mismatches — *no* error is
    /// raised, since a query that cannot match silently fails the row.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{DnfEvaluable, Op, Value};
    ///
    /// #[derive(DnfEvaluable)]
    /// struct User { age: u32 }
    ///
    /// let user = User { age: 30 };
    /// assert!(user.evaluate_field("age", &Op::GT, &Value::Uint(18)));
    /// assert!(!user.evaluate_field("missing", &Op::EQ, &Value::Uint(0)));
    /// ```
    fn evaluate_field(&self, field_name: &str, operator: &Op, value: &Value) -> bool;

    /// Returns the field's value as a [`Value`] for custom-operator evaluation.
    ///
    /// Standard operators use [`evaluate_field`](Self::evaluate_field) directly
    /// without converting to [`Value`]; this method is only called for
    /// custom operators registered through [`OpRegistry`].
    ///
    /// Returns `None` if the field does not exist. The default implementation
    /// always returns `None`; the derive macro overrides it.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{DnfEvaluable, Value};
    ///
    /// #[derive(DnfEvaluable)]
    /// struct User { age: u32 }
    ///
    /// let user = User { age: 30 };
    /// assert_eq!(user.field_value("age"), Some(Value::Uint(30)));
    /// assert_eq!(user.field_value("missing"), None);
    /// ```
    fn field_value(&self, field_name: &str) -> Option<Value> {
        let _ = field_name;
        None
    }

    /// Returns metadata for every queryable field on the type.
    ///
    /// Fields marked `#[dnf(skip)]` are excluded. The iterator does not
    /// allocate — collect into a `Vec` if you need a snapshot.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::DnfEvaluable;
    ///
    /// #[derive(DnfEvaluable)]
    /// struct User { age: u32, name: String }
    ///
    /// let names: Vec<_> = User::fields().map(|f| f.name()).collect();
    /// assert_eq!(names, vec!["age", "name"]);
    /// ```
    fn fields() -> impl Iterator<Item = FieldInfo>;

    /// Returns the [`FieldKind`] of the field at `path`, or `None` if the
    /// path does not name a known field.
    ///
    /// `path` may be a dotted nested path (e.g., `address.city`). The
    /// default implementation only validates the root segment; the derive
    /// macro overrides this to recurse into nested struct fields, so a
    /// typo such as `address.zity` is detected when `address` is annotated
    /// with `#[dnf(nested)]`.
    ///
    /// Iterator/map field boundaries (`Vec<T>`, `HashMap<K, V>`, …) are
    /// treated as opaque: validation accepts the field's kind and stops
    /// recursing, since trailing segments use runtime-interpreted syntax
    /// such as `@values` or `["key"]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{DnfEvaluable, FieldKind};
    ///
    /// #[derive(DnfEvaluable)]
    /// struct User { age: u32 }
    ///
    /// assert_eq!(User::validate_field_path("age"), Some(FieldKind::Scalar));
    /// assert_eq!(User::validate_field_path("missing"), None);
    /// ```
    fn validate_field_path(path: &str) -> Option<FieldKind> {
        let root = path.split('.').next().unwrap_or(path);
        Self::fields().find(|f| f.name() == root).map(|f| f.kind())
    }
}

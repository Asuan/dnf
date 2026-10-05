use crate::Value;
use std::cmp::Ordering;
use std::fmt;

/// Direction of an ordered comparison.
///
/// Combined with [`Op::is_inverse`] to form the four ordering operators:
/// `Greater` + `false` = `>`, `Greater` + `true` = `<=`, `Less` + `false` =
/// `<`, `Less` + `true` = `>=`.
///
/// # Examples
///
/// ```
/// use dnf::{ComparisonOrdering, Op};
/// use dnf::BaseOperator;
///
/// assert_eq!(Op::GT.base(), &BaseOperator::Comparison(ComparisonOrdering::Greater));
/// assert_eq!(Op::LT.base(), &BaseOperator::Comparison(ComparisonOrdering::Less));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ComparisonOrdering {
    /// `>` when not inverse, `<=` when inverse.
    Greater,
    /// `<` when not inverse, `>=` when inverse.
    Less,
}

/// Underlying operator without negation.
///
/// Pair with [`Op::is_inverse`] to recover the surface-level operator name
/// (`CONTAINS` vs `NOT CONTAINS`, etc.). Use [`Op::base`] to extract from an
/// existing [`Op`].
///
/// This enum is `#[non_exhaustive]`: new variants may be added in future
/// versions without a major bump.
///
/// # Examples
///
/// ```
/// use dnf::{BaseOperator, Op};
///
/// assert_eq!(Op::EQ.base(), &BaseOperator::Eq);
/// assert_eq!(Op::NE.base(), &BaseOperator::Eq);
/// assert!(Op::NE.is_inverse());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum BaseOperator {
    /// Equality (`==`, or `!=` when inverse).
    Eq,
    /// Ordered comparison along the given direction.
    Comparison(ComparisonOrdering),
    /// Substring containment for strings.
    Contains,
    /// Prefix match for strings.
    StartsWith,
    /// Suffix match for strings.
    EndsWith,
    /// Collection contains every required value.
    AllOf,
    /// Collection contains any required value.
    AnyOf,
    /// Inclusive range check `[min, max]`.
    Between,
    /// User-supplied operator resolved via [`OpRegistry`](crate::OpRegistry).
    Custom(Box<str>),
}

/// A query operator with an optional negation flag.
///
/// Pairs a [`BaseOperator`] with an `inverse` bit so that negations like
/// `NOT CONTAINS` and `!=` share their evaluation logic with their
/// non-negated counterpart. Construct via the associated constants
/// ([`Op::EQ`], [`Op::GT`], etc.) or [`Op::custom`] for a registered
/// operator.
///
/// # Examples
///
/// ```
/// use dnf::Op;
///
/// assert!(!Op::EQ.is_inverse());
/// assert!(Op::NE.is_inverse());
/// assert_eq!(Op::EQ.base(), Op::NE.base());
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Op {
    pub(crate) base: BaseOperator,
    pub(crate) inverse: bool,
}

impl Op {
    /// The equality operator (`==`).
    pub const EQ: Self = Self {
        base: BaseOperator::Eq,
        inverse: false,
    };
    /// The inequality operator (`!=`).
    pub const NE: Self = Self {
        base: BaseOperator::Eq,
        inverse: true,
    };
    /// The greater-than operator (`>`).
    pub const GT: Self = Self {
        base: BaseOperator::Comparison(ComparisonOrdering::Greater),
        inverse: false,
    };
    /// The less-than-or-equal operator (`<=`).
    pub const LTE: Self = Self {
        base: BaseOperator::Comparison(ComparisonOrdering::Greater),
        inverse: true,
    };
    /// The less-than operator (`<`).
    pub const LT: Self = Self {
        base: BaseOperator::Comparison(ComparisonOrdering::Less),
        inverse: false,
    };
    /// The greater-than-or-equal operator (`>=`).
    pub const GTE: Self = Self {
        base: BaseOperator::Comparison(ComparisonOrdering::Less),
        inverse: true,
    };
    /// Substring containment for strings (`CONTAINS`).
    pub const CONTAINS: Self = Self {
        base: BaseOperator::Contains,
        inverse: false,
    };
    /// Negated [`CONTAINS`](Self::CONTAINS) (`NOT CONTAINS`).
    pub const NOT_CONTAINS: Self = Self {
        base: BaseOperator::Contains,
        inverse: true,
    };
    /// Prefix match for strings (`STARTS WITH`).
    pub const STARTS_WITH: Self = Self {
        base: BaseOperator::StartsWith,
        inverse: false,
    };
    /// Negated [`STARTS_WITH`](Self::STARTS_WITH) (`NOT STARTS WITH`).
    pub const NOT_STARTS_WITH: Self = Self {
        base: BaseOperator::StartsWith,
        inverse: true,
    };
    /// Suffix match for strings (`ENDS WITH`).
    pub const ENDS_WITH: Self = Self {
        base: BaseOperator::EndsWith,
        inverse: false,
    };
    /// Negated [`ENDS_WITH`](Self::ENDS_WITH) (`NOT ENDS WITH`).
    pub const NOT_ENDS_WITH: Self = Self {
        base: BaseOperator::EndsWith,
        inverse: true,
    };
    /// Subset check: every required value is present in the field (`ALL OF`).
    pub const ALL_OF: Self = Self {
        base: BaseOperator::AllOf,
        inverse: false,
    };
    /// Negated [`ALL_OF`](Self::ALL_OF) (`NOT ALL OF`).
    pub const NOT_ALL_OF: Self = Self {
        base: BaseOperator::AllOf,
        inverse: true,
    };
    /// Membership check: the field contains at least one of the values (`ANY OF`, `IN`).
    pub const ANY_OF: Self = Self {
        base: BaseOperator::AnyOf,
        inverse: false,
    };
    /// Negated [`ANY_OF`](Self::ANY_OF) (`NOT ANY OF`, `NOT IN`).
    pub const NOT_ANY_OF: Self = Self {
        base: BaseOperator::AnyOf,
        inverse: true,
    };
    /// Inclusive range check (`BETWEEN [min, max]`).
    pub const BETWEEN: Self = Self {
        base: BaseOperator::Between,
        inverse: false,
    };
    /// Negated [`BETWEEN`](Self::BETWEEN) (`NOT BETWEEN [min, max]`).
    pub const NOT_BETWEEN: Self = Self {
        base: BaseOperator::Between,
        inverse: true,
    };

    /// Constructs a custom operator referenced by `name`.
    ///
    /// The operator must be registered on the query's
    /// [`OpRegistry`](crate::OpRegistry) before evaluation; an unregistered
    /// custom operator evaluates to `false`.
    pub fn custom(name: impl Into<Box<str>>) -> Self {
        Self {
            base: BaseOperator::Custom(name.into()),
            inverse: false,
        }
    }

    /// Constructs the negated form of a custom operator.
    ///
    /// Equivalent to [`custom`](Self::custom) with the inverse flag set; the
    /// result of the registered evaluator is negated.
    pub fn not_custom(name: impl Into<Box<str>>) -> Self {
        Self {
            base: BaseOperator::Custom(name.into()),
            inverse: true,
        }
    }

    /// Returns the custom operator's name, or [`None`] for built-in operators.
    pub fn custom_name(&self) -> Option<&str> {
        match &self.base {
            BaseOperator::Custom(name) => Some(name),
            _ => None,
        }
    }

    /// Returns `true` if this is a custom operator.
    pub fn is_custom(&self) -> bool {
        matches!(self.base, BaseOperator::Custom(_))
    }

    /// Returns the underlying [`BaseOperator`].
    ///
    /// The base operator strips the negation flag; for example, both
    /// [`Op::EQ`] and [`Op::NE`] return [`BaseOperator::Eq`].
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{BaseOperator, Op};
    ///
    /// assert_eq!(Op::EQ.base(), &BaseOperator::Eq);
    /// assert_eq!(Op::NE.base(), &BaseOperator::Eq);
    /// ```
    pub fn base(&self) -> &BaseOperator {
        &self.base
    }

    /// Returns `true` if the operator is the negated form (e.g.
    /// [`Op::NE`], [`Op::NOT_CONTAINS`]).
    pub fn is_inverse(&self) -> bool {
        self.inverse
    }

    // ==================== Type-specific scalar functions ====================

    /// Resolves an ordered comparison, or `None` when the operands are incomparable.
    ///
    /// Callers must bail out with `false` on `None` *before* applying `inverse`;
    /// otherwise `>=` (the inverse of `<`) would hold for operands that have no order.
    #[inline]
    fn ordered(cmp: Option<Ordering>, which: &ComparisonOrdering) -> Option<bool> {
        let want = match which {
            ComparisonOrdering::Greater => Ordering::Greater,
            ComparisonOrdering::Less => Ordering::Less,
        };
        cmp.map(|ord| ord == want)
    }

    /// Returns `true` if `value` is a well-formed range operand: a numeric array
    /// with at least two bounds.
    ///
    /// This separates an *inapplicable* BETWEEN operand (a non-array, or a
    /// non-numeric/too-short array) from a well-formed range that merely clamps
    /// to *empty* for a given field type (e.g. a negative bound on an unsigned
    /// field). The former makes BETWEEN false regardless of inversion; the latter
    /// is a real empty range, so NOT BETWEEN is true.
    #[inline]
    fn is_numeric_range(value: &Value) -> bool {
        match value {
            Value::IntArray(arr) => arr.len() >= 2,
            Value::UintArray(arr) => arr.len() >= 2,
            Value::FloatArray(arr) => arr.len() >= 2,
            _ => false,
        }
    }

    /// Extract range bounds from a Value for BETWEEN operator.
    /// Returns (min, max) as f64 for cross-type comparison.
    #[inline]
    fn extract_range_f64(value: &Value) -> Option<(f64, f64)> {
        match value {
            Value::IntArray(arr) if arr.len() >= 2 => Some((arr[0] as f64, arr[1] as f64)),
            Value::UintArray(arr) if arr.len() >= 2 => Some((arr[0] as f64, arr[1] as f64)),
            Value::FloatArray(arr) if arr.len() >= 2 => Some((arr[0], arr[1])),
            _ => None,
        }
    }

    /// Extract range bounds as i64 for integer comparison (avoids precision loss).
    #[inline]
    fn extract_range_i64(value: &Value) -> Option<(i64, i64)> {
        match value {
            Value::IntArray(arr) if arr.len() >= 2 => Some((arr[0], arr[1])),
            Value::UintArray(arr) if arr.len() >= 2 => {
                // A lower bound above i64::MAX leaves no i64 in range; an upper
                // bound above it is clamped.
                let min = i64::try_from(arr[0]).ok()?;
                let max = i64::try_from(arr[1]).unwrap_or(i64::MAX);
                Some((min, max))
            }
            Value::FloatArray(arr) if arr.len() >= 2 => {
                // Convert to i64 (truncates towards zero)
                Some((arr[0] as i64, arr[1] as i64))
            }
            _ => None,
        }
    }

    /// Extract range bounds as u64 for unsigned integer comparison.
    #[inline]
    fn extract_range_u64(value: &Value) -> Option<(u64, u64)> {
        match value {
            Value::UintArray(arr) if arr.len() >= 2 => Some((arr[0], arr[1])),
            Value::IntArray(arr) if arr.len() >= 2 => {
                // A negative lower bound is clamped to 0; a negative upper bound
                // leaves no u64 in range.
                let min = u64::try_from(arr[0]).unwrap_or(0);
                let max = u64::try_from(arr[1]).ok()?;
                Some((min, max))
            }
            Value::FloatArray(arr) if arr.len() >= 2 => {
                // `as u64` saturates, so a negative lower bound becomes 0.
                if arr[1] >= 0.0 {
                    Some((arr[0] as u64, arr[1] as u64))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Compare a string field against a query value.
    #[inline]
    pub(crate) fn scalar_str(&self, field: &str, value: &Value) -> bool {
        let result = match &self.base {
            BaseOperator::Eq => field == value,
            BaseOperator::Comparison(ord) => match Self::ordered(field.partial_cmp(value), ord) {
                Some(r) => r,
                None => return false,
            },
            BaseOperator::Contains => {
                // If the query value is None, return false (unresolvable reference -> false)
                if let Value::None = value {
                    false
                } else {
                    let needle = value.to_string_repr();
                    field.contains(needle.as_ref())
                }
            }
            BaseOperator::StartsWith => {
                // If the query value is None, return false (unresolvable reference -> false)
                if let Value::None = value {
                    false
                } else {
                    let prefix = value.to_string_repr();
                    field.starts_with(prefix.as_ref())
                }
            }
            BaseOperator::EndsWith => {
                // If the query value is None, return false (unresolvable reference -> false)
                if let Value::None = value {
                    false
                } else {
                    let suffix = value.to_string_repr();
                    field.ends_with(suffix.as_ref())
                }
            }
            BaseOperator::AnyOf => Self::scalar_str_any_of(field, value),
            BaseOperator::AllOf => Self::scalar_str_all_of(field, value),
            BaseOperator::Between => return false,
            BaseOperator::Custom(_) => return false, // Custom ops handled at query level
        };
        self.inverse ^ result
    }

    /// Compare a signed integer field against a query value.
    #[inline]
    pub(crate) fn scalar_int(&self, field: i64, value: &Value) -> bool {
        let result = match &self.base {
            BaseOperator::Eq => field == *value,
            BaseOperator::Comparison(ord) => match Self::ordered(field.partial_cmp(value), ord) {
                Some(r) => r,
                None => return false,
            },
            BaseOperator::Contains => {
                // If the query value is None, return false (unresolvable reference -> false)
                if let Value::None = value {
                    false
                } else {
                    let needle = value.to_string_repr();
                    field.to_string().contains(needle.as_ref())
                }
            }
            BaseOperator::StartsWith => {
                // If the query value is None, return false (unresolvable reference -> false)
                if let Value::None = value {
                    false
                } else {
                    let prefix = value.to_string_repr();
                    field.to_string().starts_with(prefix.as_ref())
                }
            }
            BaseOperator::EndsWith => {
                // If the query value is None, return false (unresolvable reference -> false)
                if let Value::None = value {
                    false
                } else {
                    let suffix = value.to_string_repr();
                    field.to_string().ends_with(suffix.as_ref())
                }
            }
            BaseOperator::Between => {
                // An inapplicable operand (not a range) is false regardless of
                // inversion; a well-formed range that is empty after clamping
                // (e.g. a lower bound above `i64::MAX`) yields `false` here, which
                // the `inverse` xor below turns into `true` for NOT BETWEEN.
                if !Self::is_numeric_range(value) {
                    return false;
                }
                match Self::extract_range_i64(value) {
                    Some((min, max)) => field >= min && field <= max,
                    None => false,
                }
            }
            BaseOperator::AnyOf => Self::scalar_any_of(&field, value),
            BaseOperator::AllOf => Self::scalar_all_of(&field, value),
            BaseOperator::Custom(_) => return false,
        };
        self.inverse ^ result
    }

    /// Compare an unsigned integer field against a query value.
    #[inline]
    pub(crate) fn scalar_uint(&self, field: u64, value: &Value) -> bool {
        let result = match &self.base {
            BaseOperator::Eq => field == *value,
            BaseOperator::Comparison(ord) => match Self::ordered(field.partial_cmp(value), ord) {
                Some(r) => r,
                None => return false,
            },
            BaseOperator::Contains => {
                // If the query value is None, return false (unresolvable reference -> false)
                if let Value::None = value {
                    false
                } else {
                    let needle = value.to_string_repr();
                    field.to_string().contains(needle.as_ref())
                }
            }
            BaseOperator::StartsWith => {
                // If the query value is None, return false (unresolvable reference -> false)
                if let Value::None = value {
                    false
                } else {
                    let prefix = value.to_string_repr();
                    field.to_string().starts_with(prefix.as_ref())
                }
            }
            BaseOperator::EndsWith => {
                // If the query value is None, return false (unresolvable reference -> false)
                if let Value::None = value {
                    false
                } else {
                    let suffix = value.to_string_repr();
                    field.to_string().ends_with(suffix.as_ref())
                }
            }
            BaseOperator::Between => {
                // An inapplicable operand (not a range) is false regardless of
                // inversion; a well-formed range that is empty after clamping
                // (e.g. a negative upper bound on this unsigned field) yields
                // `false` here, which the `inverse` xor below turns into `true`
                // for NOT BETWEEN.
                if !Self::is_numeric_range(value) {
                    return false;
                }
                match Self::extract_range_u64(value) {
                    Some((min, max)) => field >= min && field <= max,
                    None => false,
                }
            }
            BaseOperator::AnyOf => Self::scalar_any_of(&field, value),
            BaseOperator::AllOf => Self::scalar_all_of(&field, value),
            BaseOperator::Custom(_) => return false,
        };
        self.inverse ^ result
    }

    /// Compare a float field against a query value.
    #[inline]
    pub(crate) fn scalar_float(&self, field: f64, value: &Value) -> bool {
        let result = match &self.base {
            BaseOperator::Eq => field == *value,
            BaseOperator::Comparison(ord) => match Self::ordered(field.partial_cmp(value), ord) {
                Some(r) => r,
                None => return false,
            },
            BaseOperator::Contains => {
                // If the query value is None, return false (unresolvable reference -> false)
                if let Value::None = value {
                    false
                } else {
                    let needle = value.to_string_repr();
                    field.to_string().contains(needle.as_ref())
                }
            }
            BaseOperator::StartsWith => {
                // If the query value is None, return false (unresolvable reference -> false)
                if let Value::None = value {
                    false
                } else {
                    let prefix = value.to_string_repr();
                    field.to_string().starts_with(prefix.as_ref())
                }
            }
            BaseOperator::EndsWith => {
                // If the query value is None, return false (unresolvable reference -> false)
                if let Value::None = value {
                    false
                } else {
                    let suffix = value.to_string_repr();
                    field.to_string().ends_with(suffix.as_ref())
                }
            }
            BaseOperator::Between => {
                // An inapplicable operand (not a range) is false regardless of
                // inversion. A well-formed numeric range always extracts here, so
                // the `None` arm is unreachable but kept for symmetry.
                if !Self::is_numeric_range(value) {
                    return false;
                }
                match Self::extract_range_f64(value) {
                    Some((min, max)) => field >= min && field <= max,
                    None => false,
                }
            }
            BaseOperator::AnyOf => Self::scalar_any_of(&field, value),
            BaseOperator::AllOf => Self::scalar_all_of(&field, value),
            BaseOperator::Custom(_) => return false,
        };
        self.inverse ^ result
    }

    /// Compare a boolean field against a query value.
    #[inline]
    pub(crate) fn scalar_bool(&self, field: bool, value: &Value) -> bool {
        let result = match &self.base {
            BaseOperator::Eq => field == *value,
            // Bool comparison: true > false
            BaseOperator::Comparison(ord) => match Self::ordered(field.partial_cmp(value), ord) {
                Some(r) => r,
                None => return false,
            },
            // String ops on bool don't make sense
            BaseOperator::Contains | BaseOperator::StartsWith | BaseOperator::EndsWith => {
                return false;
            }
            BaseOperator::AnyOf => Self::scalar_any_of(&field, value),
            BaseOperator::AllOf => Self::scalar_all_of(&field, value),
            BaseOperator::Between => return false,
            BaseOperator::Custom(_) => return false,
        };
        self.inverse ^ result
    }

    /// `ANY OF` / `IN` for a string scalar field.
    ///
    /// For a `StringSet`, membership is an O(1) `contains` lookup against the
    /// borrowed field. For a `StringArray`, the field's [`Value`] is built once
    /// before the scan, so membership is decided without allocating a `Value`
    /// for every element (the generic [`scalar_any_of`](Self::scalar_any_of)
    /// would wrap each element in turn). Non-string operands fall back to that
    /// generic path so cross-type matches (e.g. `"5" IN [5]`) keep working.
    fn scalar_str_any_of(field: &str, value: &Value) -> bool {
        match value {
            Value::StringArray(opts) => {
                let needle = Value::from(field);
                opts.iter().any(|s| **s == needle)
            }
            Value::StringSet(opts) => opts.contains(field),
            _ => Self::scalar_any_of(&field, value),
        }
    }

    /// `ALL OF` for a string scalar field.
    ///
    /// The string-operand mirror of [`scalar_str_any_of`](Self::scalar_str_any_of).
    /// Set elements are unique, so `ALL OF` can hold only for an empty set
    /// (vacuously true) or a single element equal to the field — decided with an
    /// O(1) `contains`. For an array, the field's [`Value`] is built once and
    /// every element compared against it without per-element allocation.
    /// Non-string operands fall back to the generic
    /// [`scalar_all_of`](Self::scalar_all_of).
    fn scalar_str_all_of(field: &str, value: &Value) -> bool {
        match value {
            Value::StringArray(req) => {
                let needle = Value::from(field);
                req.iter().all(|s| **s == needle)
            }
            Value::StringSet(req) => match req.len() {
                0 => true,
                1 => req.contains(field),
                _ => false,
            },
            _ => Self::scalar_all_of(&field, value),
        }
    }

    /// Returns `true` if `field` equals any element of the collection `value`.
    ///
    /// Backs `ANY OF` / `IN` on a scalar field: the field matches when it is a
    /// member of the operand array or set. Numeric elements are compared
    /// cross-type, so an integer field can match a float-array element and vice
    /// versa. A non-collection operand degrades to an equality check.
    fn scalar_any_of<T>(field: &T, value: &Value) -> bool
    where
        T: PartialEq<Value>,
    {
        match value {
            Value::StringArray(opts) => opts.iter().any(|s| *field == Value::from(s.as_ref())),
            Value::StringSet(opts) => opts.iter().any(|s| *field == Value::from(s.as_ref())),
            Value::IntArray(opts) => opts.iter().any(|&x| *field == Value::Int(x)),
            Value::IntSet(opts) => opts.iter().any(|&x| *field == Value::Int(x)),
            Value::UintArray(opts) => opts.iter().any(|&x| *field == Value::Uint(x)),
            Value::UintSet(opts) => opts.iter().any(|&x| *field == Value::Uint(x)),
            Value::FloatArray(opts) => opts.iter().any(|&x| *field == Value::Float(x)),
            Value::BoolArray(opts) => opts.iter().any(|&x| *field == Value::Bool(x)),
            Value::BoolSet(opts) => opts.iter().any(|&x| *field == Value::Bool(x)),
            other => *field == *other,
        }
    }

    /// Returns `true` if `field` equals every element of the collection `value`.
    ///
    /// Backs `ALL OF` on a scalar field: a scalar holds a single value, so this
    /// holds only when every operand element equals the field. An empty operand
    /// is vacuously `true`, matching the collection `ALL OF`. A non-collection
    /// operand degrades to an equality check.
    fn scalar_all_of<T>(field: &T, value: &Value) -> bool
    where
        T: PartialEq<Value>,
    {
        match value {
            Value::StringArray(req) => req.iter().all(|s| *field == Value::from(s.as_ref())),
            Value::StringSet(req) => req.iter().all(|s| *field == Value::from(s.as_ref())),
            Value::IntArray(req) => req.iter().all(|&x| *field == Value::Int(x)),
            Value::IntSet(req) => req.iter().all(|&x| *field == Value::Int(x)),
            Value::UintArray(req) => req.iter().all(|&x| *field == Value::Uint(x)),
            Value::UintSet(req) => req.iter().all(|&x| *field == Value::Uint(x)),
            Value::FloatArray(req) => req.iter().all(|&x| *field == Value::Float(x)),
            Value::BoolArray(req) => req.iter().all(|&x| *field == Value::Bool(x)),
            Value::BoolSet(req) => req.iter().all(|&x| *field == Value::Bool(x)),
            other => *field == *other,
        }
    }

    /// Applies the operator to an iterator of field values.
    ///
    /// Used by [`DnfField`](crate::DnfField) implementations for collection
    /// fields (such as `Vec<T>` and `HashSet<T>`); the iterator yields
    /// references to the contained items, which must be directly comparable
    /// against [`Value`] without allocating an intermediate
    /// [`Value`] wrapper.
    ///
    /// `BETWEEN` returns `false` for collections (it is a scalar operator).
    /// Custom operators are not applied here — they are dispatched at the
    /// [`DnfQuery`](crate::DnfQuery) level via the registry.
    ///
    /// # Examples
    ///
    /// ```
    /// use dnf::{Op, Value};
    ///
    /// let tags = ["rust", "queries", "dnf"];
    /// assert!(Op::ANY_OF.any(tags.iter(), &Value::from(vec!["rust", "go"])));
    /// assert!(!Op::ANY_OF.any(tags.iter(), &Value::from(vec!["python", "java"])));
    /// ```
    pub fn any<'a, I, T>(&self, mut field_iter: I, query_value: &Value) -> bool
    where
        I: ExactSizeIterator<Item = &'a T> + Clone,
        T: 'a + PartialEq<Value> + PartialOrd<Value>,
    {
        let result = match &self.base {
            BaseOperator::Eq => Self::iter_eq(field_iter, query_value),
            BaseOperator::Comparison(ordering) => {
                match Self::iter_cmp(field_iter, query_value, ordering) {
                    Some(r) => r,
                    None => return false,
                }
            }
            BaseOperator::Contains => field_iter.any(|item| item == query_value),
            BaseOperator::StartsWith => Self::iter_starts_with(field_iter, query_value),
            BaseOperator::EndsWith => Self::iter_ends_with(field_iter, query_value),
            BaseOperator::AllOf => Self::iter_all_of(field_iter, query_value),
            BaseOperator::AnyOf => Self::iter_any_of(field_iter, query_value),
            BaseOperator::Between => return false, // BETWEEN doesn't apply to collections
            BaseOperator::Custom(_) => return false,
        };

        self.inverse ^ result
    }

    /// Eq operator: collection equals single value only if it has exactly one matching element
    fn iter_eq<'a, I, T>(mut field_iter: I, query_value: &Value) -> bool
    where
        I: Iterator<Item = &'a T>,
        T: 'a + PartialEq<Value> + PartialOrd<Value>,
    {
        if let Some(first) = field_iter.next() {
            // Multiple elements - can't equal single value
            if field_iter.next().is_some() {
                false
            } else {
                first == query_value
            }
        } else {
            false
        }
    }

    /// Comparison operator: use first element for comparison.
    ///
    /// Returns `None` for an empty collection or incomparable operands.
    fn iter_cmp<'a, I, T>(
        mut field_iter: I,
        query_value: &Value,
        ordering: &ComparisonOrdering,
    ) -> Option<bool>
    where
        I: Iterator<Item = &'a T>,
        T: 'a + PartialEq<Value> + PartialOrd<Value>,
    {
        let first = field_iter.next()?;
        Self::ordered(first.partial_cmp(query_value), ordering)
    }

    /// StartsWith operator: check if first element equals query_value
    fn iter_starts_with<'a, I, T>(mut field_iter: I, query_value: &Value) -> bool
    where
        I: Iterator<Item = &'a T>,
        T: 'a + PartialEq<Value> + PartialOrd<Value>,
    {
        if let Some(first) = field_iter.next() {
            first == query_value
        } else {
            false
        }
    }

    /// EndsWith operator: check if last element equals query_value
    fn iter_ends_with<'a, I, T>(field_iter: I, query_value: &Value) -> bool
    where
        I: Iterator<Item = &'a T>,
        T: 'a + PartialEq<Value> + PartialOrd<Value>,
    {
        if let Some(last) = field_iter.last() {
            last == query_value
        } else {
            false
        }
    }

    /// AllOf operator: all distinct values in query must exist in field.
    ///
    /// Uses bitmask tracking for numeric types - no allocation, O(n*m) where
    /// n = field size and m = required size. Optimized for small m (typical case).
    /// Uses as_*() methods for fast direct comparison when field type matches.
    ///
    /// Examples:
    /// - ALL OF ["a", "b", "a"] is equivalent to ALL OF ["a", "b"]
    /// - Field ["a", "b", "c"] matches both queries (contains all distinct values)
    /// - Field ["a", "a", "a"] does NOT match ALL OF ["a", "b"] (missing "b")
    fn iter_all_of<'a, I, T>(field_iter: I, query_value: &Value) -> bool
    where
        I: ExactSizeIterator<Item = &'a T> + Clone,
        T: 'a + PartialEq<Value> + PartialOrd<Value>,
    {
        match query_value {
            // Strings
            Value::StringArray(req) => Self::all_of_strs(field_iter, req.iter()),
            Value::StringSet(req) => Self::all_of_strs(field_iter, req.iter()),

            // Integers
            Value::IntArray(req) => Self::all_of_ints(field_iter, req.iter().copied()),
            Value::IntSet(req) => Self::all_of_ints(field_iter, req.iter().copied()),
            Value::UintArray(req) => Self::all_of_uints(field_iter, req.iter().copied()),
            Value::UintSet(req) => Self::all_of_uints(field_iter, req.iter().copied()),

            // Bools
            Value::BoolArray(req) => Self::all_of_bools(field_iter, req.iter().copied()),
            Value::BoolSet(req) => Self::all_of_bools(field_iter, req.iter().copied()),

            // Single value - check if it's in the field values
            _ => field_iter.clone().any(|item| item == query_value),
        }
    }

    /// Helper for ALL OF with string values.
    ///
    /// TODO: allocates a `Box<str>` per option via `Value::from`; a zero-alloc
    /// path (extraction with `O(1)` set lookups) remains possible.
    fn all_of_strs<'a, 'b, I, T, R, S>(field_iter: I, mut required: R) -> bool
    where
        I: ExactSizeIterator<Item = &'a T> + Clone,
        T: 'a + PartialEq<Value> + PartialOrd<Value>,
        R: Iterator<Item = &'b S>,
        S: 'b + AsRef<str> + ?Sized,
    {
        required.all(|req| {
            let v = Value::from(req.as_ref());
            field_iter.clone().any(|item| item == &v)
        })
    }

    /// Helper for ALL OF with i64 values.
    fn all_of_ints<'a, I, T, R>(field_iter: I, mut required: R) -> bool
    where
        I: ExactSizeIterator<Item = &'a T> + Clone,
        T: 'a + PartialEq<Value> + PartialOrd<Value>,
        R: Iterator<Item = i64>,
    {
        required.all(|req| field_iter.clone().any(|item| item == &Value::Int(req)))
    }

    /// Helper for ALL OF with u64 values.
    fn all_of_uints<'a, I, T, R>(field_iter: I, mut required: R) -> bool
    where
        I: ExactSizeIterator<Item = &'a T> + Clone,
        T: 'a + PartialEq<Value> + PartialOrd<Value>,
        R: Iterator<Item = u64>,
    {
        required.all(|req| field_iter.clone().any(|item| item == &Value::Uint(req)))
    }

    /// Helper for ALL OF with bool values.
    fn all_of_bools<'a, I, T, R>(field_iter: I, required: R) -> bool
    where
        I: Iterator<Item = &'a T>,
        T: 'a + PartialEq<Value> + PartialOrd<Value>,
        R: Iterator<Item = bool>,
    {
        // Bool: at most 2 distinct values
        let mut need_true = false;
        let mut need_false = false;
        for b in required {
            if b {
                need_true = true;
            } else {
                need_false = true;
            }
            if need_false && need_true {
                break;
            }
        }
        if !need_true && !need_false {
            return true; // empty required
        }

        let mut found_true = !need_true;
        let mut found_false = !need_false;

        for field_item in field_iter {
            if need_true && !found_true && field_item == &Value::Bool(true) {
                found_true = true;
            }
            if need_false && !found_false && field_item == &Value::Bool(false) {
                found_false = true;
            }
            if found_true && found_false {
                return true;
            }
        }
        found_true && found_false
    }

    /// AnyOf operator: returns true if at least one field value matches any of the options.
    ///
    /// Semantics: "field contains at least one of the search values"
    ///
    /// TODO: allocates a `Box<str>` per option via `Value::from`; a zero-alloc
    /// path (extraction with `O(1)` set lookups) remains possible.
    fn iter_any_of<'a, I, T>(mut field_iter: I, query_value: &Value) -> bool
    where
        I: Iterator<Item = &'a T> + Clone,
        T: 'a + PartialEq<Value> + PartialOrd<Value>,
    {
        match query_value {
            Value::StringArray(opts) => opts.iter().any(|opt| {
                let v = Value::from(opt.as_ref());
                field_iter.clone().any(|item| item == &v)
            }),
            Value::StringSet(opts) => opts.iter().any(|opt| {
                let v = Value::from(opt.as_ref());
                field_iter.clone().any(|item| item == &v)
            }),

            Value::IntArray(opts) => {
                field_iter.any(|item| opts.iter().any(|&opt| item == &Value::Int(opt)))
            }
            Value::IntSet(opts) => {
                field_iter.any(|item| opts.iter().any(|&opt| item == &Value::Int(opt)))
            }
            Value::UintArray(opts) => {
                field_iter.any(|item| opts.iter().any(|&opt| item == &Value::Uint(opt)))
            }
            Value::UintSet(opts) => {
                field_iter.any(|item| opts.iter().any(|&opt| item == &Value::Uint(opt)))
            }

            Value::BoolArray(opts) => {
                field_iter.any(|item| opts.iter().any(|&opt| item == &Value::Bool(opt)))
            }
            Value::BoolSet(opts) => {
                field_iter.any(|item| opts.iter().any(|&opt| item == &Value::Bool(opt)))
            }

            // Single value
            _ => field_iter.any(|item| item == query_value),
        }
    }
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.base {
            BaseOperator::Eq if !self.inverse => write!(f, "=="),
            BaseOperator::Eq => write!(f, "!="),
            BaseOperator::Comparison(ComparisonOrdering::Greater) if !self.inverse => {
                write!(f, ">")
            }
            BaseOperator::Comparison(ComparisonOrdering::Greater) => write!(f, "<="),
            BaseOperator::Comparison(ComparisonOrdering::Less) if !self.inverse => write!(f, "<"),
            BaseOperator::Comparison(ComparisonOrdering::Less) => write!(f, ">="),
            BaseOperator::Custom(name) => {
                if self.inverse {
                    write!(f, "NOT {}", name)
                } else {
                    write!(f, "{}", name)
                }
            }
            base => {
                let prefix = if self.inverse { "NOT " } else { "" };
                match base {
                    BaseOperator::Contains => write!(f, "{}CONTAINS", prefix),
                    BaseOperator::StartsWith => write!(f, "{}STARTS WITH", prefix),
                    BaseOperator::EndsWith => write!(f, "{}ENDS WITH", prefix),
                    BaseOperator::AllOf => write!(f, "{}ALL OF", prefix),
                    BaseOperator::AnyOf => write!(f, "{}IN", prefix),
                    BaseOperator::Between => write!(f, "{}BETWEEN", prefix),
                    _ => unreachable!(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_comparison_operators() {
        use crate::DnfField;

        let test_cases: Vec<(Op, Value, Value, bool, &str)> = vec![
            // ===== Equality =====
            (
                Op::EQ,
                Value::Int(42),
                Value::Int(42),
                true,
                "eq: equal values",
            ),
            (
                Op::EQ,
                Value::Int(42),
                Value::Int(43),
                false,
                "eq: different values",
            ),
            (
                Op::NE,
                Value::Int(42),
                Value::Int(43),
                true,
                "ne: different values",
            ),
            (
                Op::NE,
                Value::Int(42),
                Value::Int(42),
                false,
                "ne: equal values",
            ),
            // ===== Greater Than =====
            (Op::GT, Value::Int(10), Value::Int(5), true, "gt: greater"),
            (Op::GT, Value::Int(5), Value::Int(10), false, "gt: less"),
            (Op::GT, Value::Int(5), Value::Int(5), false, "gt: equal"),
            // ===== Less Than =====
            (Op::LT, Value::Int(5), Value::Int(10), true, "lt: less"),
            (Op::LT, Value::Int(10), Value::Int(5), false, "lt: greater"),
            (Op::LT, Value::Int(5), Value::Int(5), false, "lt: equal"),
            // ===== Greater Than or Equal =====
            (Op::GTE, Value::Int(10), Value::Int(5), true, "gte: greater"),
            (Op::GTE, Value::Int(5), Value::Int(5), true, "gte: equal"),
            (Op::GTE, Value::Int(5), Value::Int(10), false, "gte: less"),
            // ===== Less Than or Equal =====
            (Op::LTE, Value::Int(5), Value::Int(10), true, "lte: less"),
            (Op::LTE, Value::Int(5), Value::Int(5), true, "lte: equal"),
            (
                Op::LTE,
                Value::Int(10),
                Value::Int(5),
                false,
                "lte: greater",
            ),
        ];

        for (op, left, right, expected, desc) in test_cases {
            assert_eq!(left.evaluate(&op, &right), expected, "Failed: {}", desc);
        }
    }

    #[test]
    fn test_string_operators() {
        use crate::DnfField;

        let test_cases: Vec<(Op, Value, Value, bool, &str)> = vec![
            // ===== CONTAINS =====
            (
                Op::CONTAINS,
                Value::from("hello world"),
                Value::from("world"),
                true,
                "contains: found",
            ),
            (
                Op::CONTAINS,
                Value::from("hello world"),
                Value::from("xyz"),
                false,
                "contains: not found",
            ),
            (
                Op::NOT_CONTAINS,
                Value::from("hello world"),
                Value::from("xyz"),
                true,
                "not_contains: missing",
            ),
            (
                Op::NOT_CONTAINS,
                Value::from("hello world"),
                Value::from("world"),
                false,
                "not_contains: found",
            ),
            // ===== STARTS WITH =====
            (
                Op::STARTS_WITH,
                Value::from("hello"),
                Value::from("hel"),
                true,
                "starts_with: match",
            ),
            (
                Op::STARTS_WITH,
                Value::from("hello"),
                Value::from("bye"),
                false,
                "starts_with: no match",
            ),
            (
                Op::NOT_STARTS_WITH,
                Value::from("hello"),
                Value::from("bye"),
                true,
                "not_starts_with: no match",
            ),
            (
                Op::NOT_STARTS_WITH,
                Value::from("hello"),
                Value::from("hel"),
                false,
                "not_starts_with: match",
            ),
            (
                Op::NOT_STARTS_WITH,
                Value::from("hello"),
                Value::from("hello"),
                false,
                "not_starts_with: exact match",
            ),
            // ===== ENDS WITH =====
            (
                Op::ENDS_WITH,
                Value::from("hello"),
                Value::from("llo"),
                true,
                "ends_with: match",
            ),
            (
                Op::ENDS_WITH,
                Value::from("hello"),
                Value::from("xyz"),
                false,
                "ends_with: no match",
            ),
            (
                Op::NOT_ENDS_WITH,
                Value::from("hello"),
                Value::from("xyz"),
                true,
                "not_ends_with: no match",
            ),
            (
                Op::NOT_ENDS_WITH,
                Value::from("hello"),
                Value::from("llo"),
                false,
                "not_ends_with: match",
            ),
            (
                Op::NOT_ENDS_WITH,
                Value::from("hello"),
                Value::from("hello"),
                false,
                "not_ends_with: exact match",
            ),
            // ===== Numeric string conversion =====
            (
                Op::NOT_STARTS_WITH,
                Value::Int(142),
                Value::Int(99),
                true,
                "numeric not_starts_with: no match",
            ),
            (
                Op::NOT_STARTS_WITH,
                Value::Int(142),
                Value::Int(14),
                false,
                "numeric not_starts_with: match",
            ),
            (
                Op::NOT_STARTS_WITH,
                Value::Float(3.04),
                Value::from("2"),
                true,
                "float not_starts_with: no match",
            ),
            (
                Op::NOT_ENDS_WITH,
                Value::Int(142),
                Value::Int(99),
                true,
                "numeric not_ends_with: no match",
            ),
            (
                Op::NOT_ENDS_WITH,
                Value::Int(142),
                Value::Int(42),
                false,
                "numeric not_ends_with: match",
            ),
            (
                Op::NOT_ENDS_WITH,
                Value::Float(3.00),
                Value::from("5"),
                true,
                "float not_ends_with: no match",
            ),
            (
                Op::NOT_STARTS_WITH,
                Value::from("hello"),
                Value::from(""),
                false,
                "not_starts_with: everything starts with empty",
            ),
            (
                Op::NOT_STARTS_WITH,
                Value::from(""),
                Value::from("x"),
                true,
                "not_starts_with: empty doesn't start with x",
            ),
            (
                Op::NOT_ENDS_WITH,
                Value::from("hello"),
                Value::from(""),
                false,
                "not_ends_with: everything ends with empty",
            ),
            (
                Op::NOT_ENDS_WITH,
                Value::from(""),
                Value::from("x"),
                true,
                "not_ends_with: empty doesn't end with x",
            ),
        ];

        for (op, left, right, expected, desc) in test_cases {
            assert_eq!(left.evaluate(&op, &right), expected, "Failed: {}", desc);
        }
    }

    #[test]
    fn test_between_operators() {
        use crate::DnfField;

        let int_range = |min: i64, max: i64| Value::from(vec![min, max]);
        let uint_range = |min: u64, max: u64| Value::from(vec![min, max]);
        let float_range = |min: f64, max: f64| Value::from(vec![min, max]);

        let test_cases: Vec<(Op, Value, Value, bool, &str)> = vec![
            // ===== Integer BETWEEN =====
            (
                Op::BETWEEN,
                Value::Int(50),
                int_range(10, 100),
                true,
                "i64: middle of range",
            ),
            (
                Op::BETWEEN,
                Value::Int(10),
                int_range(10, 100),
                true,
                "i64: at min (inclusive)",
            ),
            (
                Op::BETWEEN,
                Value::Int(100),
                int_range(10, 100),
                true,
                "i64: at max (inclusive)",
            ),
            (
                Op::BETWEEN,
                Value::Int(5),
                int_range(10, 100),
                false,
                "i64: below min",
            ),
            (
                Op::BETWEEN,
                Value::Int(101),
                int_range(10, 100),
                false,
                "i64: above max",
            ),
            // ===== Unsigned Integer BETWEEN =====
            (
                Op::BETWEEN,
                Value::Uint(50),
                uint_range(10, 100),
                true,
                "u64: middle of range",
            ),
            (
                Op::BETWEEN,
                Value::Uint(10),
                uint_range(10, 100),
                true,
                "u64: at min",
            ),
            (
                Op::BETWEEN,
                Value::Uint(100),
                uint_range(10, 100),
                true,
                "u64: at max",
            ),
            (
                Op::BETWEEN,
                Value::Uint(5),
                uint_range(10, 100),
                false,
                "u64: below min",
            ),
            (
                Op::BETWEEN,
                Value::Uint(101),
                uint_range(10, 100),
                false,
                "u64: above max",
            ),
            // ===== Float BETWEEN =====
            (
                Op::BETWEEN,
                Value::Float(50.5),
                float_range(10.0, 100.0),
                true,
                "f64: middle of range",
            ),
            (
                Op::BETWEEN,
                Value::Float(10.0),
                float_range(10.0, 100.0),
                true,
                "f64: at min",
            ),
            (
                Op::BETWEEN,
                Value::Float(100.0),
                float_range(10.0, 100.0),
                true,
                "f64: at max",
            ),
            (
                Op::BETWEEN,
                Value::Float(9.99),
                float_range(10.0, 100.0),
                false,
                "f64: below min",
            ),
            (
                Op::BETWEEN,
                Value::Float(100.01),
                float_range(10.0, 100.0),
                false,
                "f64: above max",
            ),
            // ===== NOT BETWEEN =====
            (
                Op::NOT_BETWEEN,
                Value::Int(50),
                int_range(10, 100),
                false,
                "NOT BETWEEN: inside range",
            ),
            (
                Op::NOT_BETWEEN,
                Value::Int(5),
                int_range(10, 100),
                true,
                "NOT BETWEEN: below min",
            ),
            (
                Op::NOT_BETWEEN,
                Value::Int(101),
                int_range(10, 100),
                true,
                "NOT BETWEEN: above max",
            ),
            (
                Op::NOT_BETWEEN,
                Value::Float(50.5),
                float_range(10.0, 100.0),
                false,
                "NOT BETWEEN f64: inside",
            ),
            (
                Op::NOT_BETWEEN,
                Value::Float(9.99),
                float_range(10.0, 100.0),
                true,
                "NOT BETWEEN f64: outside",
            ),
            // ===== Cross-type =====
            (
                Op::BETWEEN,
                Value::Int(50),
                float_range(10.5, 100.5),
                true,
                "i64 with FloatArray range",
            ),
            (
                Op::BETWEEN,
                Value::Float(50.5),
                int_range(10, 100),
                true,
                "f64 with IntArray range",
            ),
            (
                Op::BETWEEN,
                Value::Uint(50),
                int_range(10, 100),
                true,
                "u64 with IntArray range",
            ),
            // ===== Single value range =====
            (
                Op::BETWEEN,
                Value::Int(50),
                int_range(50, 50),
                true,
                "single value range: exact match",
            ),
            (
                Op::BETWEEN,
                Value::Int(49),
                int_range(50, 50),
                false,
                "single value range: below",
            ),
            (
                Op::BETWEEN,
                Value::Int(51),
                int_range(50, 50),
                false,
                "single value range: above",
            ),
            // ===== Negative ranges =====
            (
                Op::BETWEEN,
                Value::Int(-50),
                int_range(-100, -10),
                true,
                "negative range: inside",
            ),
            (
                Op::BETWEEN,
                Value::Int(-5),
                int_range(-100, -10),
                false,
                "negative range: above max",
            ),
            (
                Op::BETWEEN,
                Value::Int(-101),
                int_range(-100, -10),
                false,
                "negative range: below min",
            ),
            // ===== Range crossing zero =====
            (
                Op::BETWEEN,
                Value::Int(0),
                int_range(-10, 10),
                true,
                "zero-crossing: at zero",
            ),
            (
                Op::BETWEEN,
                Value::Int(-5),
                int_range(-10, 10),
                true,
                "zero-crossing: negative",
            ),
            (
                Op::BETWEEN,
                Value::Int(5),
                int_range(-10, 10),
                true,
                "zero-crossing: positive",
            ),
            // ===== Invalid range values =====
            (
                Op::BETWEEN,
                Value::Int(50),
                Value::Int(100),
                false,
                "invalid: scalar instead of array",
            ),
            (
                Op::BETWEEN,
                Value::Int(50),
                Value::from("10,100"),
                false,
                "invalid: string",
            ),
            (
                Op::BETWEEN,
                Value::Int(50),
                Value::from(vec![10i64]),
                false,
                "invalid: array with 1 element",
            ),
            // ===== Unsupported field types =====
            (
                Op::BETWEEN,
                Value::from("hello"),
                int_range(10, 100),
                false,
                "string field: not supported",
            ),
            (
                Op::BETWEEN,
                Value::Bool(true),
                int_range(0, 1),
                false,
                "bool field: not supported",
            ),
            // ===== Boundary values =====
            (
                Op::BETWEEN,
                Value::Int(0),
                int_range(i64::MIN, i64::MAX),
                true,
                "i64 extremes: zero",
            ),
            (
                Op::BETWEEN,
                Value::Int(i64::MIN),
                int_range(i64::MIN, i64::MAX),
                true,
                "i64 extremes: MIN",
            ),
            (
                Op::BETWEEN,
                Value::Int(i64::MAX),
                int_range(i64::MIN, i64::MAX),
                true,
                "i64 extremes: MAX",
            ),
            (
                Op::BETWEEN,
                Value::Uint(0),
                uint_range(0, u64::MAX),
                true,
                "u64 extremes: zero",
            ),
            (
                Op::BETWEEN,
                Value::Uint(u64::MAX),
                uint_range(0, u64::MAX),
                true,
                "u64 extremes: MAX",
            ),
        ];

        for (op, field, range, expected, desc) in test_cases {
            assert_eq!(field.evaluate(&op, &range), expected, "Failed: {}", desc);
        }
    }

    #[test]
    fn test_all_of_operators() {
        // Test data: reusable field arrays
        let field_int: [i64; 5] = [1, 2, 3, 4, 5];
        let field_uint: [u64; 3] = [10, 20, 30];
        let field_str: [String; 3] = [
            "apple".to_string(),
            "banana".to_string(),
            "cherry".to_string(),
        ];
        let field_bool: [bool; 2] = [true, false];
        let empty_field: [i64; 0] = [];

        // Helper to create HashSets
        let int_set = |vals: &[i64]| -> HashSet<i64> { vals.iter().copied().collect() };
        let str_set =
            |vals: &[&str]| -> HashSet<String> { vals.iter().map(|s| s.to_string()).collect() };

        // (operator, field_slice, query_value, expected, description)
        type TestCase<'a, T> = (Op, &'a [T], Value, bool, &'static str);

        // Integer tests
        let int_tests: Vec<TestCase<i64>> = vec![
            (
                Op::ALL_OF,
                &field_int,
                Value::from(vec![2, 3]),
                true,
                "subset present",
            ),
            (
                Op::ALL_OF,
                &field_int,
                Value::from(vec![2, 6]),
                false,
                "missing element",
            ),
            (
                Op::ALL_OF,
                &field_int,
                Value::from(vec![0i64; 0]),
                true,
                "empty subset (vacuously true)",
            ),
            (
                Op::ALL_OF,
                &field_int,
                Value::from(vec![2u64, 3]),
                true,
                "cross-type int/uint",
            ),
            (
                Op::ALL_OF,
                &field_int,
                Value::from(vec![2, 3, 2, 3]),
                true,
                "duplicates ignored",
            ),
            (
                Op::ALL_OF,
                &field_int,
                Value::from(int_set(&[2, 3])),
                true,
                "IntSet: contains all",
            ),
            (
                Op::ALL_OF,
                &field_int,
                Value::from(int_set(&[2, 6])),
                false,
                "IntSet: missing 6",
            ),
            (
                Op::ALL_OF,
                &field_int,
                Value::from(int_set(&[])),
                true,
                "IntSet: empty (vacuously true)",
            ),
            (
                Op::NOT_ALL_OF,
                &field_int,
                Value::from(vec![2, 3]),
                false,
                "NOT ALL OF: subset present",
            ),
            (
                Op::NOT_ALL_OF,
                &field_int,
                Value::from(vec![2, 6]),
                true,
                "NOT ALL OF: missing element",
            ),
            (
                Op::ALL_OF,
                &empty_field,
                Value::from(vec![1, 2, 3]),
                false,
                "empty field: returns false",
            ),
            (
                Op::ALL_OF,
                &empty_field,
                Value::from(int_set(&[])),
                true,
                "empty field + empty set: true",
            ),
        ];

        for (op, field, query, expected, desc) in int_tests {
            assert_eq!(op.any(field.iter(), &query), expected, "Failed: {}", desc);
        }

        // Uint tests
        let uint_tests: Vec<TestCase<u64>> = vec![(
            Op::ALL_OF,
            &field_uint,
            Value::from(vec![10u64, 20, 10]),
            true,
            "duplicates ignored",
        )];

        for (op, field, query, expected, desc) in uint_tests {
            assert_eq!(op.any(field.iter(), &query), expected, "Failed: {}", desc);
        }

        // String tests
        let str_tests: Vec<TestCase<String>> = vec![
            (
                Op::ALL_OF,
                &field_str,
                Value::from(vec!["banana", "cherry"]),
                true,
                "string subset present",
            ),
            (
                Op::ALL_OF,
                &field_str,
                Value::from(vec!["banana", "grape"]),
                false,
                "string missing element",
            ),
            (
                Op::ALL_OF,
                &field_str,
                Value::from(str_set(&["apple", "cherry"])),
                true,
                "StringSet: contains all",
            ),
            (
                Op::ALL_OF,
                &field_str,
                Value::from(str_set(&["apple", "grape"])),
                false,
                "StringSet: missing grape",
            ),
        ];

        for (op, field, query, expected, desc) in str_tests {
            assert_eq!(op.any(field.iter(), &query), expected, "Failed: {}", desc);
        }

        // Bool tests
        let bool_tests: Vec<TestCase<bool>> = vec![
            (
                Op::ALL_OF,
                &field_bool,
                Value::from(vec![true, true, true]),
                true,
                "bool: multiple true = single true",
            ),
            (
                Op::ALL_OF,
                &field_bool,
                Value::from(vec![true, false, true, false]),
                true,
                "bool: duplicates of both",
            ),
            (
                Op::ALL_OF,
                &[true, true, true],
                Value::from(vec![true, false]),
                false,
                "bool: all true missing false",
            ),
        ];

        for (op, field, query, expected, desc) in bool_tests {
            assert_eq!(op.any(field.iter(), &query), expected, "Failed: {}", desc);
        }

        // Large sets (>64 elements)
        let large_field: Vec<i64> = (1..=100).collect();
        let large_tests = vec![
            (
                Value::from((1..=70).collect::<Vec<i64>>()),
                true,
                "70 required elements",
            ),
            (
                Value::from((1..=100).collect::<Vec<i64>>()),
                true,
                "100 required elements",
            ),
            (
                Value::from((2..=101).collect::<Vec<i64>>()),
                false,
                "missing element 101",
            ),
        ];

        for (query, expected, desc) in large_tests {
            assert_eq!(
                Op::ALL_OF.any(large_field.iter(), &query),
                expected,
                "Failed: {}",
                desc
            );
        }
    }

    #[test]
    fn test_any_of_operators() {
        // Test data: reusable field arrays
        let field_int: [i64; 5] = [1, 2, 3, 4, 5];
        let field_uint: [u64; 3] = [3, 5, 7];
        let field_str: [String; 3] = ["foo".to_string(), "bar".to_string(), "baz".to_string()];
        let field_bool: [bool; 2] = [true, false];
        let empty_field: [i64; 0] = [];

        // Helper to create HashSets
        let int_set = |vals: &[i64]| -> HashSet<i64> { vals.iter().copied().collect() };
        let uint_set = |vals: &[u64]| -> HashSet<u64> { vals.iter().copied().collect() };
        let str_set =
            |vals: &[&str]| -> HashSet<String> { vals.iter().map(|s| s.to_string()).collect() };
        let bool_set = |vals: &[bool]| -> HashSet<bool> { vals.iter().copied().collect() };

        // (operator, field_slice, query_value, expected, description)
        type TestCase<'a, T> = (Op, &'a [T], Value, bool, &'static str);

        // Integer tests
        let int_tests: Vec<TestCase<i64>> = vec![
            (
                Op::ANY_OF,
                &field_int,
                Value::from(vec![3, 6, 9]),
                true,
                "array contains one",
            ),
            (
                Op::ANY_OF,
                &field_int,
                Value::from(vec![6, 7, 8]),
                false,
                "array contains none",
            ),
            (
                Op::NOT_ANY_OF,
                &field_int,
                Value::from(vec![3, 6, 9]),
                false,
                "NOT ANY OF: contains one",
            ),
            (
                Op::NOT_ANY_OF,
                &field_int,
                Value::from(vec![6, 7, 8]),
                true,
                "NOT ANY OF: contains none",
            ),
            (
                Op::ANY_OF,
                &field_int,
                Value::from(int_set(&[3, 6, 9])),
                true,
                "IntSet: contains 3",
            ),
            (
                Op::ANY_OF,
                &field_int,
                Value::from(int_set(&[6, 7, 8])),
                false,
                "IntSet: contains none",
            ),
            (
                Op::ANY_OF,
                &field_int,
                Value::from(int_set(&[])),
                false,
                "IntSet: empty set returns false",
            ),
            (
                Op::ANY_OF,
                &empty_field,
                Value::from(vec![1, 2, 3]),
                false,
                "empty field returns false",
            ),
            (
                Op::NOT_ANY_OF,
                &empty_field,
                Value::from(vec![1, 2, 3]),
                true,
                "NOT ANY OF: empty field returns true",
            ),
        ];

        for (op, field, query, expected, desc) in int_tests {
            assert_eq!(op.any(field.iter(), &query), expected, "Failed: {}", desc);
        }

        // Uint tests
        let uint_tests: Vec<TestCase<u64>> = vec![
            (
                Op::ANY_OF,
                &field_uint,
                Value::from(vec![5i64, 9]),
                true,
                "cross-type uint/int contains one",
            ),
            (
                Op::ANY_OF,
                &field_uint,
                Value::from(uint_set(&[20, 40])),
                false,
                "UintSet: contains none",
            ),
        ];

        for (op, field, query, expected, desc) in uint_tests {
            assert_eq!(op.any(field.iter(), &query), expected, "Failed: {}", desc);
        }

        // String tests
        let str_tests: Vec<TestCase<String>> = vec![
            (
                Op::ANY_OF,
                &field_str,
                Value::from(vec!["foo", "qux"]),
                true,
                "string array contains one",
            ),
            (
                Op::ANY_OF,
                &field_str,
                Value::from(vec!["qux", "xyz"]),
                false,
                "string array contains none",
            ),
            (
                Op::ANY_OF,
                &field_str,
                Value::from(str_set(&["bar", "grape"])),
                true,
                "StringSet: contains bar",
            ),
            (
                Op::ANY_OF,
                &field_str,
                Value::from(str_set(&["grape", "melon"])),
                false,
                "StringSet: contains none",
            ),
        ];

        for (op, field, query, expected, desc) in str_tests {
            assert_eq!(op.any(field.iter(), &query), expected, "Failed: {}", desc);
        }

        // Bool tests
        let bool_tests: Vec<TestCase<bool>> = vec![
            (
                Op::ANY_OF,
                &field_bool,
                Value::from(bool_set(&[true])),
                true,
                "BoolSet: contains true",
            ),
            (
                Op::ANY_OF,
                &[true, true, true],
                Value::from(bool_set(&[false])),
                false,
                "all true doesn't contain false",
            ),
            (
                Op::ANY_OF,
                &[true, false, true],
                Value::from(bool_set(&[true, false])),
                true,
                "mixed contains both",
            ),
        ];

        for (op, field, query, expected, desc) in bool_tests {
            assert_eq!(op.any(field.iter(), &query), expected, "Failed: {}", desc);
        }

        // Edge cases
        let large_field: Vec<i64> = (1..=100).collect();
        let edge_tests = vec![
            (
                Op::ANY_OF,
                Value::from(int_set(&(50..=150).collect::<Vec<_>>())),
                true,
                "large set: overlap",
            ),
            (
                Op::ANY_OF,
                Value::from(int_set(&(200..=300).collect::<Vec<_>>())),
                false,
                "large set: no overlap",
            ),
        ];

        for (op, query, expected, desc) in edge_tests {
            assert_eq!(
                op.any(large_field.iter(), &query),
                expected,
                "Failed: {}",
                desc
            );
        }

        // Boundary values
        let field_extremes: [i64; 5] = [i64::MIN, -1, 0, 1, i64::MAX];
        assert!(
            Op::ANY_OF.any(
                field_extremes.iter(),
                &Value::from(int_set(&[i64::MIN, i64::MAX]))
            ),
            "boundary: i64 extremes"
        );
    }

    #[test]
    fn test_operator_display() {
        let test_cases: Vec<(Op, &str)> = vec![
            (Op::EQ, "=="),
            (Op::NE, "!="),
            (Op::GT, ">"),
            (Op::LT, "<"),
            (Op::GTE, ">="),
            (Op::LTE, "<="),
            (Op::CONTAINS, "CONTAINS"),
            (Op::NOT_CONTAINS, "NOT CONTAINS"),
            (Op::STARTS_WITH, "STARTS WITH"),
            (Op::ENDS_WITH, "ENDS WITH"),
            (Op::NOT_STARTS_WITH, "NOT STARTS WITH"),
            (Op::NOT_ENDS_WITH, "NOT ENDS WITH"),
            (Op::ALL_OF, "ALL OF"),
            (Op::NOT_ALL_OF, "NOT ALL OF"),
            (Op::ANY_OF, "IN"),
            (Op::NOT_ANY_OF, "NOT IN"),
            (Op::BETWEEN, "BETWEEN"),
            (Op::NOT_BETWEEN, "NOT BETWEEN"),
        ];

        for (op, expected) in test_cases {
            assert_eq!(op.to_string(), expected, "Failed: {} != {}", op, expected);
        }
    }

    #[test]
    fn test_scalar_float() {
        // Test that NaN field values never match any comparison (as per SQL semantics)
        let nan = f64::NAN;
        let f = &Value::Float(5.0);
        let cases = vec![
            // Test all comparison operators with NaN field values - they should always return false
            Op::LTE,
            Op::GTE,
            Op::EQ,
            Op::GT,
            Op::LT,
        ];

        for case in cases {
            assert!(
                !case.scalar_float(nan, f),
                "Failed: {nan} {case} {f} sould be false"
            );
        }
        // Test that regular float comparisons still work

        assert!(
            Op::EQ.scalar_float(5.0, &Value::Float(5.0)),
            "5.0 == 5.0 should be true"
        );
        assert!(
            Op::LT.scalar_float(3.0, &Value::Float(5.0)),
            "3.0 < 5.0 should be true"
        );
        assert!(
            Op::GT.scalar_float(5.0, &Value::Float(3.0)),
            "5.0 > 3.0 should be true"
        );
        assert!(
            Op::LTE.scalar_float(5.0, &Value::Float(5.0)),
            "5.0 <= 5.0 should be true"
        );
        assert!(
            Op::GTE.scalar_float(5.0, &Value::Float(5.0)),
            "5.0 >= 5.0 should be true"
        );

        // Test NaN with other NaN (should also be false)
        assert!(
            !Op::EQ.scalar_float(nan, &Value::Float(f64::NAN)),
            "NaN field == NaN value should be true"
        );
        assert!(
            !Op::EQ.scalar_float(5.0, &Value::Float(f64::NAN)),
            "5.0 field == NaN value should be true"
        );
    }

    #[test]
    fn test_collection_field_scalar_eval_returns_false() {
        use crate::DnfField;

        // An array/set-typed field evaluated through the *scalar* `evaluate()`
        // path returns false — collection fields must be evaluated via `any()`.
        let test_cases: Vec<(Op, Value, Value, &str)> = vec![
            (
                Op::ALL_OF,
                Value::from(vec![1i64, 2, 3]),
                Value::from(vec![2i64, 3]),
                "ALL OF on array field",
            ),
            (
                Op::ANY_OF,
                Value::from(vec![1i64, 2, 3]),
                Value::from(vec![3i64]),
                "ANY OF on array field",
            ),
            (
                Op::BETWEEN,
                Value::from(vec![1i64, 2, 3]),
                Value::from(vec![1i64, 5]),
                "BETWEEN on array field",
            ),
        ];

        for (op, field, query, desc) in test_cases {
            assert!(
                !field.evaluate(&op, &query),
                "Failed: {} should return false",
                desc
            );
        }
    }

    #[test]
    fn test_scalar_membership_any_of_all_of() {
        use crate::DnfField;

        // `ANY OF` / `IN` on a scalar field is membership; `ALL OF` holds only
        // when every element equals the field. `NOT` variants invert the result.
        // Covers every scalar type, cross-type numerics, floats (which the
        // collection path omits), empty operands, and non-membership.
        let cases: Vec<(Value, Op, Value, bool, &str)> = vec![
            // ANY OF / IN membership.
            (
                Value::from("active"),
                Op::ANY_OF,
                Value::from(vec!["active", "pending"]),
                true,
                "string member",
            ),
            (
                Value::from("deleted"),
                Op::ANY_OF,
                Value::from(vec!["active", "pending"]),
                false,
                "string non-member",
            ),
            (
                Value::Int(5),
                Op::ANY_OF,
                Value::from(vec![3i64, 5, 7]),
                true,
                "int member",
            ),
            (
                Value::Uint(5),
                Op::ANY_OF,
                Value::from(vec![3i64, 5, 7]),
                true,
                "uint member cross-type (IntArray)",
            ),
            (
                Value::Float(2.5),
                Op::ANY_OF,
                Value::from(vec![1.5f64, 2.5]),
                true,
                "float member (collection path omits FloatArray)",
            ),
            (
                Value::Bool(true),
                Op::ANY_OF,
                Value::from(vec![true, false]),
                true,
                "bool member",
            ),
            (
                Value::Int(5),
                Op::ANY_OF,
                Value::from(Vec::<i64>::new()),
                false,
                "empty operand is never a member",
            ),
            // NOT IN inverts membership (and is true for a non-member).
            (
                Value::Int(5),
                Op::NOT_ANY_OF,
                Value::from(vec![3i64, 5, 7]),
                false,
                "NOT IN member",
            ),
            (
                Value::Int(4),
                Op::NOT_ANY_OF,
                Value::from(vec![3i64, 5, 7]),
                true,
                "NOT IN non-member",
            ),
            // ALL OF: field must equal every element.
            (
                Value::Int(5),
                Op::ALL_OF,
                Value::from(vec![5i64, 5]),
                true,
                "ALL OF all-equal",
            ),
            (
                Value::Int(5),
                Op::ALL_OF,
                Value::from(vec![5i64, 6]),
                false,
                "ALL OF mixed",
            ),
            (
                Value::Int(5),
                Op::ALL_OF,
                Value::from(Vec::<i64>::new()),
                true,
                "ALL OF empty is vacuously true",
            ),
            // String field membership against a `StringSet` operand (the
            // allocation-free scalar string path).
            (
                Value::from("active"),
                Op::ANY_OF,
                Value::string_set(["active", "pending"]),
                true,
                "string member (StringSet)",
            ),
            (
                Value::from("deleted"),
                Op::ANY_OF,
                Value::string_set(["active", "pending"]),
                false,
                "string non-member (StringSet)",
            ),
            (
                Value::from("banned"),
                Op::NOT_ANY_OF,
                Value::from(vec!["banned", "deleted"]),
                false,
                "string NOT IN a listed value",
            ),
            (
                Value::from("alice"),
                Op::NOT_ANY_OF,
                Value::from(vec!["banned", "deleted"]),
                true,
                "string NOT IN an unlisted value",
            ),
            // String `ALL OF`: a scalar holds one value, so it matches only when
            // every element is that same string.
            (
                Value::from("active"),
                Op::ALL_OF,
                Value::from(vec!["active", "active"]),
                true,
                "string ALL OF all-equal (StringArray)",
            ),
            (
                Value::from("active"),
                Op::ALL_OF,
                Value::from(vec!["active", "pending"]),
                false,
                "string ALL OF mixed",
            ),
            (
                Value::from("active"),
                Op::ALL_OF,
                Value::string_set(["active"]),
                true,
                "string ALL OF single-element (StringSet)",
            ),
            (
                Value::from("active"),
                Op::ALL_OF,
                Value::from(Vec::<&str>::new()),
                true,
                "string ALL OF empty is vacuously true",
            ),
            // Cross-type: a string field against a non-string operand still
            // resolves through the generic fallback.
            (
                Value::from("5"),
                Op::ANY_OF,
                Value::from(vec![3i64, 5, 7]),
                true,
                "string field cross-type member (IntArray)",
            ),
            (
                Value::from("9"),
                Op::ANY_OF,
                Value::from(vec![3i64, 5, 7]),
                false,
                "string field cross-type non-member (IntArray)",
            ),
        ];

        for (field, op, query, expected, desc) in cases {
            assert_eq!(field.evaluate(&op, &query), expected, "Failed: {}", desc);
        }
    }

    #[test]
    fn test_all_operators_smoke() {
        use crate::DnfField;

        // Quick smoke test for each operator
        let scalar_tests: Vec<(Value, Op, Value, bool, &str)> = vec![
            (Value::Int(5), Op::EQ, Value::Int(5), true, "EQ"),
            (Value::Int(5), Op::NE, Value::Int(3), true, "NE"),
            (Value::Int(5), Op::GT, Value::Int(3), true, "GT"),
            (Value::Int(3), Op::LT, Value::Int(5), true, "LT"),
            (Value::Int(5), Op::GTE, Value::Int(5), true, "GTE"),
            (Value::Int(5), Op::LTE, Value::Int(5), true, "LTE"),
            (
                Value::from("hello"),
                Op::CONTAINS,
                Value::from("ell"),
                true,
                "CONTAINS",
            ),
            (
                Value::from("hello"),
                Op::NOT_CONTAINS,
                Value::from("xyz"),
                true,
                "NOT_CONTAINS",
            ),
            (
                Value::from("hello"),
                Op::STARTS_WITH,
                Value::from("hel"),
                true,
                "STARTS_WITH",
            ),
            (
                Value::from("hello"),
                Op::ENDS_WITH,
                Value::from("llo"),
                true,
                "ENDS_WITH",
            ),
            (
                Value::from("hello"),
                Op::NOT_STARTS_WITH,
                Value::from("bye"),
                true,
                "NOT_STARTS_WITH",
            ),
            (
                Value::from("hello"),
                Op::NOT_ENDS_WITH,
                Value::from("xyz"),
                true,
                "NOT_ENDS_WITH",
            ),
        ];

        for (field, op, query, expected, desc) in scalar_tests {
            assert_eq!(field.evaluate(&op, &query), expected, "Failed: {}", desc);
        }

        // Collection operators with any()
        let field = [1, 2, 3];
        let collection_tests: Vec<(Op, Value, bool, &str)> = vec![
            (Op::ALL_OF, Value::from(vec![2, 3]), true, "ALL_OF"),
            (Op::NOT_ALL_OF, Value::from(vec![2, 5]), true, "NOT_ALL_OF"),
            (Op::ANY_OF, Value::from(vec![2, 7]), true, "ANY_OF"),
            (Op::NOT_ANY_OF, Value::from(vec![4, 5]), true, "NOT_ANY_OF"),
        ];

        for (op, query, expected, desc) in collection_tests {
            assert_eq!(op.any(field.iter(), &query), expected, "Failed: {}", desc);
        }
    }

    #[test]
    fn test_inverse_ordered_ops_false_when_incomparable() {
        let empty: Vec<i64> = vec![];
        let single = [5_i64];
        let cases: Vec<(bool, &str)> = vec![
            (
                Op::GTE.any(empty.iter(), &Value::Int(4)),
                "empty collection >= 4",
            ),
            (
                Op::LTE.any(empty.iter(), &Value::Int(0)),
                "empty collection <= 0",
            ),
            (
                Op::NOT_BETWEEN.any(single.iter(), &Value::from(vec![1_i64, 10])),
                "NOT BETWEEN on collection",
            ),
            (
                Op::GTE.scalar_int(5, &Value::from("abc")),
                "int >= non-numeric string",
            ),
            (Op::LTE.scalar_int(5, &Value::Bool(true)), "int <= bool"),
            (Op::GTE.scalar_uint(5, &Value::Bool(true)), "uint >= bool"),
            (
                Op::LTE.scalar_float(5.0, &Value::Bool(true)),
                "float <= bool",
            ),
            (
                Op::GTE.scalar_float(f64::NAN, &Value::Float(1.0)),
                "NaN >= 1.0",
            ),
            (Op::GTE.scalar_bool(true, &Value::Int(1)), "bool >= int"),
            (
                Op::NOT_BETWEEN.scalar_int(5, &Value::Int(1)),
                "NOT BETWEEN, non-range operand",
            ),
            (
                Op::NOT_BETWEEN.scalar_str("5", &Value::from(vec![1_i64, 10])),
                "NOT BETWEEN on str",
            ),
            (
                Op::NOT_CONTAINS.scalar_bool(true, &Value::from("t")),
                "NOT CONTAINS on bool",
            ),
        ];
        for (actual, desc) in cases {
            assert!(!actual, "Failed: {}", desc);
        }
    }

    #[test]
    fn test_between_empty_range_clamps_and_inverts() {
        // A negative bound clamps against a field's type rather than making the
        // whole range silently unsatisfiable. `0` (the clamped lower bound, and
        // the min of every unsigned type) is the key corner case.
        //
        // (result, expected, description)
        let int_range = Value::from(vec![-5_i64, 10]);
        let empty_range = Value::from(vec![-10_i64, -5]); // no unsigned value fits
        let upper_zero = Value::from(vec![-5_i64, 0]); // collapses to {0}
        let cases: Vec<(bool, bool, &str)> = vec![
            // Negative lower bound clamps to 0; 0 is included.
            (Op::BETWEEN.scalar_uint(0, &int_range), true, "0 in [-5,10]"),
            (
                Op::NOT_BETWEEN.scalar_uint(0, &int_range),
                false,
                "0 not-outside [-5,10]",
            ),
            // Upper bound of exactly 0: only 0 matches.
            (Op::BETWEEN.scalar_uint(0, &upper_zero), true, "0 in [-5,0]"),
            (
                Op::BETWEEN.scalar_uint(1, &upper_zero),
                false,
                "1 not in [-5,0]",
            ),
            // Empty range (negative upper bound) on an unsigned field: BETWEEN is
            // false, and NOT BETWEEN inverts to true — unlike an inapplicable
            // operand, which is false either way.
            (
                Op::BETWEEN.scalar_uint(0, &empty_range),
                false,
                "0 in empty range",
            ),
            (
                Op::NOT_BETWEEN.scalar_uint(0, &empty_range),
                true,
                "0 outside empty range",
            ),
            // A signed field evaluates the identical query the same way: 0 is not
            // in [-10,-5], so NOT BETWEEN is true. (Regression: these diverged.)
            (
                Op::BETWEEN.scalar_int(0, &empty_range),
                false,
                "i64: 0 in [-10,-5]",
            ),
            (
                Op::NOT_BETWEEN.scalar_int(0, &empty_range),
                true,
                "i64: 0 outside [-10,-5]",
            ),
        ];
        for (actual, expected, desc) in cases {
            assert_eq!(actual, expected, "Failed: {desc}");
        }
    }

    #[test]
    fn test_none_value_string_operators() {
        // Tests that demonstrate the fix:
        // When a field is None (Value::None), CONTAINS/STARTS_WITH/ENDS_WITH
        // should evaluate to false, not true.

        use crate::field::DnfField;

        let none_value = Value::None;
        // TODO: rework into datadriven test
        //
        // These should all be false (unresolvable reference -> false)
        assert!(!none_value.evaluate(&Op::CONTAINS, &Value::from("test")));
        assert!(!none_value.evaluate(&Op::STARTS_WITH, &Value::from("test")));
        assert!(!none_value.evaluate(&Op::ENDS_WITH, &Value::from("test")));

        // Same test with inverted operators - they should also be false
        assert!(!none_value.evaluate(&Op::NOT_CONTAINS, &Value::from("test")));
        assert!(!none_value.evaluate(&Op::NOT_STARTS_WITH, &Value::from("test")));
        assert!(!none_value.evaluate(&Op::NOT_ENDS_WITH, &Value::from("test")));
    }

    #[test]
    fn test_none_value_string_operators_comprehensive() {
        // Comprehensive tests for None values with various string operators
        use crate::field::DnfField;

        // TODO: rework into datadriven test
        let none_value = Value::None;
        let test_string = "test";

        // Test all string operators return false when field is None
        assert!(!none_value.evaluate(&Op::CONTAINS, &Value::from(test_string)));
        assert!(!none_value.evaluate(&Op::STARTS_WITH, &Value::from(test_string)));
        assert!(!none_value.evaluate(&Op::ENDS_WITH, &Value::from(test_string)));

        // Test with numeric values - should also return false
        assert!(!none_value.evaluate(&Op::CONTAINS, &Value::from(42i64)));
        assert!(!none_value.evaluate(&Op::STARTS_WITH, &Value::from(42i64)));
        assert!(!none_value.evaluate(&Op::ENDS_WITH, &Value::from(42i64)));

        // Test with boolean values - should also return false
        assert!(!none_value.evaluate(&Op::CONTAINS, &Value::from(true)));
        assert!(!none_value.evaluate(&Op::STARTS_WITH, &Value::from(true)));
        assert!(!none_value.evaluate(&Op::ENDS_WITH, &Value::from(true)));
    }
}

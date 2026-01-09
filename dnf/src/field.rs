//! Field evaluation trait for DNF queries.
//!
//! The `DnfField` trait provides a unified interface for evaluating
//! struct fields against query operators and values.

use crate::operator::Op;
use crate::value::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Trait for types that can be evaluated as a field value in DNF queries.
///
/// This trait is implemented for:
/// - All primitive types (`i8`-`i64`, `u8`-`u64`, `f32`, `f64`, `bool`)
/// - String types (`String`, `&str`, `Box<str>`, `Cow<str>`)
/// - Collections (`Vec<T>`, `HashSet<T>`) where `T: PartialEq<Value> + PartialOrd<Value>`
/// - Options (`Option<T>`) where `T: DnfField`
/// - Maps (`HashMap<K, V>`, `BTreeMap<K, V>`) where `K: AsRef<str>` and `V: DnfField`
///
/// # Example: Custom Type
///
/// ```rust
/// use dnf::{DnfField, Op, Value};
///
/// struct Score(u32);
///
/// impl DnfField for Score {
///     fn evaluate(&self, op: &Op, value: &Value) -> bool {
///         // Delegate to the i64 implementation
///         (self.0 as i64).evaluate(op, value)
///     }
/// }
/// ```
pub trait DnfField {
    /// Evaluate this field against an operator and value.
    fn evaluate(&self, op: &Op, value: &Value) -> bool;
}

// ==================== String types ====================

macro_rules! impl_dnf_field_str {
    ($($ty:ty),*) => {
        $(
            impl DnfField for $ty {
                #[inline]
                fn evaluate(&self, op: &Op, value: &Value) -> bool {
                    op.scalar_str(self, value)
                }
            }
        )*
    };
}

impl_dnf_field_str!(String, str, Box<str>);

// Cow<str> support
impl DnfField for std::borrow::Cow<'_, str> {
    #[inline]
    fn evaluate(&self, op: &Op, value: &Value) -> bool {
        op.scalar_str(self.as_ref(), value)
    }
}

// ==================== Primitives ====================

macro_rules! impl_dnf_field_numeric {
    ($method:ident as $cast:ty => $($ty:ty),*) => {
        $(
            impl DnfField for $ty {
                #[inline]
                fn evaluate(&self, op: &Op, value: &Value) -> bool {
                    op.$method(*self as $cast, value)
                }
            }
        )*
    };
}

impl_dnf_field_numeric!(scalar_int as i64 => i8, i16, i32, i64, isize);
impl_dnf_field_numeric!(scalar_uint as u64 => u8, u16, u32, u64, usize);
impl_dnf_field_numeric!(scalar_float as f64 => f32, f64);

impl DnfField for bool {
    #[inline]
    fn evaluate(&self, op: &Op, value: &Value) -> bool {
        op.scalar_bool(*self, value)
    }
}

// ==================== Value itself ====================

impl DnfField for Value {
    #[inline]
    fn evaluate(&self, op: &Op, value: &Value) -> bool {
        match self {
            Value::String(s) => op.scalar_str(s, value),
            Value::Int(n) => op.scalar_int(*n, value),
            Value::Uint(n) => op.scalar_uint(*n, value),
            Value::Float(f) => op.scalar_float(*f, value),
            Value::Bool(b) => op.scalar_bool(*b, value),
            Value::None => {
                // None == None, None != anything else
                match &op.base {
                    crate::operator::BaseOperator::Eq => {
                        let result = matches!(value, Value::None);
                        if op.inverse {
                            !result
                        } else {
                            result
                        }
                    }
                    _ => false,
                }
            }
            // Arrays/Sets are not scalar - should use any() via collection impls
            _ => false,
        }
    }
}

// ==================== Vec<T> ====================

impl<T> DnfField for Vec<T>
where
    T: PartialEq<Value> + PartialOrd<Value>,
{
    #[inline]
    fn evaluate(&self, op: &Op, value: &Value) -> bool {
        op.any(self.iter(), value)
    }
}

// ==================== HashSet<T> ====================

impl<T> DnfField for HashSet<T>
where
    T: PartialEq<Value> + PartialOrd<Value> + Eq + std::hash::Hash,
{
    #[inline]
    fn evaluate(&self, op: &Op, value: &Value) -> bool {
        op.any(self.iter(), value)
    }
}

// ==================== Option<T> ====================

// TODO: add Arc<T> support

impl<T> DnfField for Option<T>
where
    T: DnfField,
{
    #[inline]
    fn evaluate(&self, op: &Op, value: &Value) -> bool {
        match self {
            Some(v) => v.evaluate(op, value),
            None => Value::None.evaluate(op, value),
        }
    }
}

// ==================== Map types ====================

macro_rules! impl_dnf_field_map {
    ($($map_type:ident),*) => {
        $(
            impl<V> DnfField for $map_type<String, V>
            where
                V: DnfField + PartialEq<Value> + PartialOrd<Value>,
            {
                #[inline]
                fn evaluate(&self, op: &Op, value: &Value) -> bool {
                    match value {
                        Value::AtKey(key, inner) => match self.get(key.as_ref()) {
                            Some(v) => v.evaluate(op, inner),
                            None => Value::None.evaluate(op, inner),
                        },
                        Value::Keys(inner) => op.any(self.keys(), inner),
                        Value::Values(inner) => op.any(self.values(), inner),
                        _ => false,
                    }
                }
            }
        )*
    };
}

impl_dnf_field_map!(HashMap, BTreeMap);
#[cfg(test)]
mod tests {
    use super::*;

    /// Test Option<T> evaluation semantics as documented in review issue #6.
    ///
    /// For None values:
    /// - EQ with null returns true
    /// - NE with null returns false
    /// - All other operators (GT, LT, GTE, LTE, etc.) return false
    ///
    /// This is the current documented behavior, not a bug.
    #[test]
    fn test_option_evaluation_semantics() {
        struct TestCase {
            name: &'static str,
            field: Option<u32>,
            op: Op,
            value: Value,
            expected: bool,
        }

        let test_cases = vec![
            // None with EQ/NE null
            TestCase {
                name: "None == null",
                field: None,
                op: Op::EQ,
                value: Value::None,
                expected: true,
            },
            TestCase {
                name: "None != null",
                field: None,
                op: Op::NE,
                value: Value::None,
                expected: false,
            },
            // None with comparison operators - all return false
            TestCase {
                name: "None > 0",
                field: None,
                op: Op::GT,
                value: Value::Uint(0),
                expected: false,
            },
            TestCase {
                name: "None < 100",
                field: None,
                op: Op::LT,
                value: Value::Uint(100),
                expected: false,
            },
            TestCase {
                name: "None >= 0",
                field: None,
                op: Op::GTE,
                value: Value::Uint(0),
                expected: false,
            },
            TestCase {
                name: "None <= 100",
                field: None,
                op: Op::LTE,
                value: Value::Uint(100),
                expected: false,
            },
            // None with BETWEEN operator
            TestCase {
                name: "None BETWEEN [0, 100]",
                field: None,
                op: Op::BETWEEN,
                value: Value::UintArray(vec![0, 100].into()),
                expected: false,
            },
            // Some values work normally
            TestCase {
                name: "Some(5) > 0",
                field: Some(5),
                op: Op::GT,
                value: Value::Uint(0),
                expected: true,
            },
            TestCase {
                name: "Some(5) < 100",
                field: Some(5),
                op: Op::LT,
                value: Value::Uint(100),
                expected: true,
            },
            TestCase {
                name: "Some(5) == null",
                field: Some(5),
                op: Op::EQ,
                value: Value::None,
                expected: false,
            },
            TestCase {
                name: "Some(5) != null",
                field: Some(5),
                op: Op::NE,
                value: Value::None,
                expected: true,
            },
            TestCase {
                name: "Some(5) == 5",
                field: Some(5),
                op: Op::EQ,
                value: Value::Uint(5),
                expected: true,
            },
            TestCase {
                name: "Some(5) != 10",
                field: Some(5),
                op: Op::NE,
                value: Value::Uint(10),
                expected: true,
            },
            TestCase {
                name: "Some(5) BETWEEN [0, 100]",
                field: Some(5),
                op: Op::BETWEEN,
                value: Value::UintArray(vec![0, 100].into()),
                expected: true,
            },
        ];

        for case in test_cases {
            let result = case.field.evaluate(&case.op, &case.value);
            assert_eq!(
                result, case.expected,
                "Test '{}' failed: expected {}, got {}",
                case.name, case.expected, result
            );
        }
    }
}

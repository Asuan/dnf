use std::fmt;

#[cfg(feature = "parser")]
use crate::parser::ParseError;

/// Unified error type for all DNF operations
#[derive(Debug, Clone, PartialEq)]
pub enum DnfError {
    /// Field not found in the struct
    FieldNotFound(Box<str>),
    /// Type mismatch when comparing values (during evaluation or parsing)
    TypeMismatch {
        field: Box<str>,
        expected: Box<str>,
        actual: Box<str>,
        position: Option<usize>, // Some for parse-time, None for eval-time
    },
    /// Invalid op for the given type
    InvalidOp { field: Box<str>, operator: Box<str> },
    /// Unknown field name in query
    UnknownField {
        field_name: Box<str>,
        position: usize,
    },
    /// Map target value used with non-map field
    InvalidMapTarget {
        field_name: Box<str>,
        field_kind: crate::FieldKind,
    },
    /// Custom operator not registered
    UnregisteredCustomOp { operator_name: Box<str> },
    /// Parsing-related errors
    #[cfg(feature = "parser")]
    ParseError(ParseError),
}

impl fmt::Display for DnfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DnfError::FieldNotFound(field) => {
                write!(f, "Field '{}' not found", field)
            }
            DnfError::TypeMismatch {
                field,
                expected,
                actual,
                position,
            } => match position {
                Some(pos) => write!(
                    f,
                    "Type mismatch for field '{}' at position {}: expected {}, got {}",
                    field, pos, expected, actual
                ),
                None => write!(
                    f,
                    "Type mismatch for field '{}': expected {}, got {}",
                    field, expected, actual
                ),
            },
            DnfError::InvalidOp { field, operator } => {
                write!(f, "Invalid operator '{}' for field '{}'", operator, field)
            }
            DnfError::UnknownField {
                field_name,
                position,
            } => write!(f, "Unknown field '{}' at position {}", field_name, position),
            DnfError::InvalidMapTarget {
                field_name,
                field_kind,
            } => write!(
                f,
                "Map target (AtKey/Keys/Values/Entries) used with non-map field '{}' (kind: {:?})",
                field_name, field_kind
            ),
            DnfError::UnregisteredCustomOp { operator_name } => write!(
                f,
                "Custom operator '{}' is not registered in the operator registry",
                operator_name
            ),
            #[cfg(feature = "parser")]
            DnfError::ParseError(err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for DnfError {}

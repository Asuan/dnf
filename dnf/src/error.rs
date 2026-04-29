use thiserror::Error;

/// Renders an optional position as ` at position N`, or empty when `None`.
fn position_suffix(position: Option<usize>) -> String {
    position
        .map(|p| format!(" at position {p}"))
        .unwrap_or_default()
}

/// Renders ` at position N near `…`` for parser-error Display strings.
#[cfg(feature = "parser")]
fn format_location(position: usize, input: &str) -> String {
    format!(
        " at position {} near `{}`",
        position,
        get_context(input, position)
    )
}

/// Extracts a ~40-char snippet around `position` (with ellipses when
/// truncated) for human-readable parser-error context.
#[cfg(feature = "parser")]
fn get_context(input: &str, position: usize) -> String {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();

    let start = position.saturating_sub(20);
    let end = (position + 20).min(len);

    let mut snippet = String::new();
    if start > 0 {
        snippet.push_str("...");
    }
    let text: String = chars[start..end].iter().collect();
    snippet.push_str(&text);
    if end < len {
        snippet.push_str("...");
    }
    snippet
}

/// The error type returned by all fallible DNF operations.
///
/// Covers builder/evaluation errors (unknown fields, type mismatches,
/// unregistered custom operators, invalid map targets) and, when the
/// `parser` feature is enabled, parser errors (lexer and grammar failures).
///
/// # Examples
///
/// ```
/// use dnf::{DnfEvaluable, DnfError, DnfQuery, Op};
///
/// #[derive(DnfEvaluable)]
/// struct User { age: u32 }
///
/// let err = DnfQuery::builder()
///     .or(|c| c.and("unknown", Op::EQ, 1))
///     .validate::<User>()
///     .unwrap_err();
///
/// assert!(matches!(err, DnfError::UnknownField { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum DnfError {
    /// A value's type does not match what the field or operator expected.
    ///
    /// `position` is `Some` for parse-time errors (offset into the query
    /// string) and `None` for evaluation-time errors.
    #[error(
        "Type mismatch for field '{field}'{}: expected {expected}, got {actual}",
        position_suffix(*position)
    )]
    TypeMismatch {
        /// The field whose value triggered the mismatch.
        field: Box<str>,
        /// The expected type, as a human-readable string.
        expected: Box<str>,
        /// The actual type, as a human-readable string.
        actual: Box<str>,
        /// Byte offset into the source query, when known.
        position: Option<usize>,
    },
    /// The operator is not valid for the field's type.
    #[error("Invalid operator '{operator}' for field '{field}'")]
    InvalidOp {
        /// The field on which the operator was applied.
        field: Box<str>,
        /// The display form of the offending operator.
        operator: Box<str>,
    },
    /// A query references a field name that does not exist on the target type.
    #[error("Unknown field '{field_name}'{}", position_suffix(*position))]
    UnknownField {
        /// The unknown field name as it appeared in the query.
        field_name: Box<str>,
        /// Byte offset of the field name in the source query, when known.
        ///
        /// `Some` for parse-time errors; `None` for builder/evaluation-time
        /// errors that have no source position.
        position: Option<usize>,
    },
    /// A map-targeted value (such as
    /// [`Value::AtKey`](crate::Value::AtKey)) was applied to a non-map field.
    #[error("Map target (AtKey/Keys/Values) used with non-map field '{field_name}' (kind: {field_kind})")]
    InvalidMapTarget {
        /// The name of the field that received the map-targeted value.
        field_name: Box<str>,
        /// The actual kind of the field.
        field_kind: crate::FieldKind,
    },
    /// A query uses a custom operator that has not been registered on the
    /// query's [`OpRegistry`](crate::OpRegistry).
    #[error("Custom operator '{operator_name}' is not registered in the operator registry")]
    UnregisteredCustomOp {
        /// The name of the unregistered operator.
        operator_name: Box<str>,
    },

    // ---- Parser errors (feature = "parser"). Lexer and grammar failures
    // raised while tokenizing/parsing a query string. ----
    /// The parser encountered a token that did not match the grammar.
    #[cfg(feature = "parser")]
    #[error("Expected {expected}, found {found}{}", format_location(*position, input))]
    UnexpectedToken {
        /// Description of what the parser expected.
        expected: String,
        /// Description of what the parser found instead.
        found: String,
        /// Byte offset of the offending token.
        position: usize,
        /// The original query string, kept for diagnostic context.
        input: String,
    },
    /// A numeric literal could not be parsed.
    #[cfg(feature = "parser")]
    #[error("Invalid number '{value}'{}", format_location(*position, input))]
    InvalidNumber {
        /// The literal text that failed to parse.
        value: String,
        /// Byte offset of the literal.
        position: usize,
        /// The original query string.
        input: String,
    },
    /// A string literal was not closed.
    #[cfg(feature = "parser")]
    #[error("Unterminated string{}", format_location(*position, input))]
    UnterminatedString {
        /// Byte offset where the string literal began.
        position: usize,
        /// The original query string.
        input: String,
    },
    /// The query string was empty or whitespace only.
    #[cfg(feature = "parser")]
    #[error("Query string is empty")]
    EmptyQuery,
    /// The parser ran out of input while expecting more tokens.
    #[cfg(feature = "parser")]
    #[error("Unexpected end of input{}", format_location(*position, input))]
    UnexpectedEof {
        /// Byte offset at which input ended (typically `input.len()`).
        position: usize,
        /// The original query string.
        input: String,
    },
    /// An unsupported escape sequence appeared inside a string literal.
    #[cfg(feature = "parser")]
    #[error("Invalid escape sequence '{escape}'{}", format_location(*position, input))]
    InvalidEscape {
        /// The escape sequence that was rejected (e.g. `\\x`).
        escape: String,
        /// Byte offset of the escape sequence.
        position: usize,
        /// The original query string.
        input: String,
    },
}

#[cfg(test)]
#[cfg(feature = "parser")]
mod parser_error_tests {
    use super::*;

    struct ErrorFormatCase {
        desc: &'static str,
        error: DnfError,
        expected_substrings: Vec<&'static str>,
    }

    #[test]
    fn test_error_format() {
        let long_query = "a".repeat(50) + " > 18 AND invalid_token";

        let cases = vec![
            ErrorFormatCase {
                desc: "unexpected token",
                error: DnfError::UnexpectedToken {
                    expected: "operator".to_string(),
                    found: "identifier 'foo'".to_string(),
                    position: 10,
                    input: "age > 18 AND foo country == \"US\"".to_string(),
                },
                expected_substrings: vec![
                    "at position 10",
                    "near `",
                    "Expected operator",
                    "found identifier 'foo'",
                ],
            },
            ErrorFormatCase {
                desc: "invalid number",
                error: DnfError::InvalidNumber {
                    value: "123abc".to_string(),
                    position: 6,
                    input: "age > 123abc".to_string(),
                },
                expected_substrings: vec!["Invalid number '123abc'", "at position 6"],
            },
            ErrorFormatCase {
                desc: "unterminated string",
                error: DnfError::UnterminatedString {
                    position: 9,
                    input: r#"name == "unclosed string"#.to_string(),
                },
                expected_substrings: vec!["Unterminated string", "at position 9"],
            },
            ErrorFormatCase {
                desc: "invalid escape",
                error: DnfError::InvalidEscape {
                    escape: "\\x".to_string(),
                    position: 15,
                    input: r#"name == "test\xfail""#.to_string(),
                },
                expected_substrings: vec!["Invalid escape sequence '\\x'", "at position 15"],
            },
            ErrorFormatCase {
                desc: "context truncated with ellipsis",
                error: DnfError::UnexpectedToken {
                    expected: "value".to_string(),
                    found: "identifier".to_string(),
                    position: 60,
                    input: long_query,
                },
                expected_substrings: vec!["...", "at position 60"],
            },
            ErrorFormatCase {
                desc: "empty query",
                error: DnfError::EmptyQuery,
                expected_substrings: vec!["Query string is empty"],
            },
            ErrorFormatCase {
                desc: "unexpected EOF",
                error: DnfError::UnexpectedEof {
                    position: 8,
                    input: "age > 18".to_string(),
                },
                expected_substrings: vec!["Unexpected end of input", "at position 8"],
            },
        ];

        for case in cases {
            let msg = case.error.to_string();
            for substring in &case.expected_substrings {
                assert!(
                    msg.contains(substring),
                    "case '{}': expected substring '{}' in message, got: {}",
                    case.desc,
                    substring,
                    msg
                );
            }
        }
    }

    #[test]
    fn test_get_context() {
        type Validator = Box<dyn Fn(&str) -> bool>;
        let cases: Vec<(&'static str, &'static str, usize, Validator)> = vec![
            (
                "short input fits without ellipses",
                "age > 18",
                5,
                Box::new(|c: &str| c == "age > 18"),
            ),
            (
                "position at start has no leading ellipsis",
                "this is a long query string",
                0,
                Box::new(|c: &str| !c.starts_with("...")),
            ),
            (
                "position at end has no trailing ellipsis",
                "this is a long query string",
                "this is a long query string".len() - 1,
                Box::new(|c: &str| !c.ends_with("...")),
            ),
        ];

        for (desc, input, position, validator) in cases {
            let context = get_context(input, position);
            assert!(
                validator(&context),
                "case '{}': context did not satisfy validator, got: '{}'",
                desc,
                context
            );
        }
    }
}

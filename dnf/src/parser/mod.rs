//! Query string parser for DNF queries.
//!
//! This module provides internal functionality to parse query strings like
//! `(age > 18 AND country == "US") OR premium == true` into `DnfQuery` structures.
//!
//! Use [`QueryBuilder::from_query`](crate::QueryBuilder::from_query) for the public API.

use std::fmt;

use crate::{DnfError, DnfQuery, FieldInfo};

mod query_parser;
mod token;

use query_parser::Parser;
use token::tokenize;

/// Parse a query string with explicit field information.
///
/// # Arguments
///
/// * `query` - The query string to parse
/// * `fields` - Field metadata for validation
/// * `custom_op_names` - Optional iterator of custom operator names to recognize
/// * `novalue_ops` - Optional iterator of novalue operator names (operators without values)
pub(crate) fn parse_with_fields<'a, I, J>(
    query: &str,
    fields: &[FieldInfo],
    custom_op_names: Option<I>,
    novalue_ops: Option<J>,
) -> Result<DnfQuery, DnfError>
where
    I: Iterator<Item = &'a str>,
    J: Iterator<Item = &'a str>,
{
    let custom_ops: Option<Vec<String>> =
        custom_op_names.map(|iter| iter.map(|s| s.to_string()).collect());
    let novalue_ops: Option<Vec<String>> =
        novalue_ops.map(|iter| iter.map(|s| s.to_string()).collect());
    let tokens = tokenize(query, custom_ops.as_deref())?;
    let parser = Parser::new(tokens, fields, query.to_string(), novalue_ops.as_deref());
    parser.parse()
}

/// Errors that occur during query parsing
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    /// Unexpected token encountered during parsing
    UnexpectedToken {
        expected: String,
        found: String,
        position: usize,
        input: String,
    },
    /// Invalid number format
    InvalidNumber {
        value: String,
        position: usize,
        input: String,
    },
    /// Unterminated string literal
    UnterminatedString { position: usize, input: String },
    /// Empty query string
    EmptyQuery,
    /// Unexpected end of input
    UnexpectedEof,
    /// Invalid escape sequence in string
    InvalidEscape {
        escape: String,
        position: usize,
        input: String,
    },
}

impl ParseError {
    /// Get context snippet around the error position
    fn get_context(&self, input: &str, position: usize) -> String {
        let chars: Vec<char> = input.chars().collect();
        let len = chars.len();

        // Show up to 20 chars before and after the error position
        let start = position.saturating_sub(20);
        let end = (position + 20).min(len);

        let mut snippet = String::new();

        // Add ellipsis if we're not at the start
        if start > 0 {
            snippet.push_str("...");
        }

        // Add the text snippet
        let text: String = chars[start..end].iter().collect();
        snippet.push_str(&text);

        // Add ellipsis if we're not at the end
        if end < len {
            snippet.push_str("...");
        }

        snippet
    }

    /// Format position and context suffix: " at position N near `context`"
    fn format_location(&self, position: usize, input: &str) -> String {
        format!(
            " at position {} near `{}`",
            position,
            self.get_context(input, position)
        )
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnexpectedToken {
                expected,
                found,
                position,
                input,
            } => write!(
                f,
                "Expected {}, found {}{}",
                expected,
                found,
                self.format_location(*position, input)
            ),
            ParseError::InvalidNumber {
                value,
                position,
                input,
            } => write!(
                f,
                "Invalid number '{}'{}",
                value,
                self.format_location(*position, input)
            ),
            ParseError::UnterminatedString { position, input } => write!(
                f,
                "Unterminated string{}",
                self.format_location(*position, input)
            ),
            ParseError::EmptyQuery => write!(f, "Query string is empty"),
            ParseError::UnexpectedEof => write!(f, "Unexpected end of input"),
            ParseError::InvalidEscape {
                escape,
                position,
                input,
            } => write!(
                f,
                "Invalid escape sequence '{}'{}",
                escape,
                self.format_location(*position, input)
            ),
        }
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_format_unexpected_token() {
        let error = ParseError::UnexpectedToken {
            expected: "operator".to_string(),
            found: "identifier 'foo'".to_string(),
            position: 10,
            input: "age > 18 AND foo country == \"US\"".to_string(),
        };

        let msg = error.to_string();
        assert!(msg.contains("at position 10"));
        assert!(msg.contains("near `"));
        assert!(msg.contains("Expected operator"));
        assert!(msg.contains("found identifier 'foo'"));
    }

    #[test]
    fn test_error_format_invalid_number() {
        let error = ParseError::InvalidNumber {
            value: "123abc".to_string(),
            position: 6,
            input: "age > 123abc".to_string(),
        };

        let msg = error.to_string();
        assert!(msg.contains("Invalid number '123abc'"));
        assert!(msg.contains("at position 6"));
        assert!(msg.contains("near `"));
    }

    #[test]
    fn test_error_format_unterminated_string() {
        let error = ParseError::UnterminatedString {
            position: 9,
            input: r#"name == "unclosed string"#.to_string(),
        };

        let msg = error.to_string();
        assert!(msg.contains("Unterminated string"));
        assert!(msg.contains("at position 9"));
        assert!(msg.contains("near `"));
    }

    #[test]
    fn test_error_format_invalid_escape() {
        let error = ParseError::InvalidEscape {
            escape: "\\x".to_string(),
            position: 15,
            input: r#"name == "test\xfail""#.to_string(),
        };

        let msg = error.to_string();
        assert!(msg.contains("Invalid escape sequence '\\x'"));
        assert!(msg.contains("at position 15"));
        assert!(msg.contains("near `"));
    }

    #[test]
    fn test_error_format_with_ellipsis() {
        let long_query = "a".repeat(50) + " > 18 AND invalid_token";
        let error = ParseError::UnexpectedToken {
            expected: "value".to_string(),
            found: "identifier".to_string(),
            position: 60,
            input: long_query,
        };

        let msg = error.to_string();
        // Should have ellipsis due to long context
        assert!(msg.contains("..."));
        assert!(msg.contains("at position 60"));
    }

    #[test]
    fn test_error_format_empty_query() {
        let error = ParseError::EmptyQuery;
        assert_eq!(error.to_string(), "Query string is empty");
    }

    #[test]
    fn test_error_format_unexpected_eof() {
        let error = ParseError::UnexpectedEof;
        assert_eq!(error.to_string(), "Unexpected end of input");
    }

    #[test]
    fn test_get_context_short_input() {
        let error = ParseError::UnexpectedToken {
            expected: "test".to_string(),
            found: "test".to_string(),
            position: 5,
            input: "age > 18".to_string(),
        };

        let context = error.get_context("age > 18", 5);
        // Should not have ellipsis for short input
        assert!(!context.contains("..."));
        assert_eq!(context, "age > 18");
    }

    #[test]
    fn test_get_context_at_start() {
        let error = ParseError::UnexpectedToken {
            expected: "test".to_string(),
            found: "test".to_string(),
            position: 0,
            input: "this is a long query string".to_string(),
        };

        let context = error.get_context("this is a long query string", 0);
        // Should not have leading ellipsis when at start
        assert!(!context.starts_with("..."));
    }

    #[test]
    fn test_get_context_at_end() {
        let input = "this is a long query string";
        let error = ParseError::UnexpectedToken {
            expected: "test".to_string(),
            found: "test".to_string(),
            position: input.len() - 1,
            input: input.to_string(),
        };

        let context = error.get_context(input, input.len() - 1);
        // Should not have trailing ellipsis when at end
        assert!(!context.ends_with("..."));
    }
}

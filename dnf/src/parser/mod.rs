//! Query string parser for DNF queries.
//!
//! Parses query strings such as
//! `(age > 18 AND country == "US") OR premium == true` into a
//! [`DnfQuery`]. The internal tokenizer and recursive-descent
//! parser are not exposed; build queries through
//! [`DnfQuery::parse`](crate::DnfQuery::parse) or
//! [`QueryBuilder::parse`](crate::QueryBuilder::parse).
//!
//! Lexer and grammar errors surface as parser-specific variants on
//! [`DnfError`] (e.g. [`DnfError::UnexpectedToken`]).
//!
//! # Number format
//!
//! Numeric literals follow the grammar
//! `-?[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?`:
//!
//! - an optional leading `-` (a leading `+` is rejected),
//! - one or more integer digits,
//! - an optional fraction: `.` followed by one or more digits (a trailing dot
//!   such as `1.` is rejected),
//! - an optional exponent: `e`/`E`, an optional sign, then one or more digits
//!   (an exponent without digits such as `1e` or `1e+` is rejected).
//!
//! A literal that runs directly into identifier characters (`18abc`, `5_000`)
//! is rejected. Malformed literals surface as [`DnfError::InvalidNumber`] at the
//! offending byte. Examples that lex: `42`, `-5`, `1.5`, `1e5`, `1.5E-3`,
//! `2E+3`.
//!
//! # Type leniency
//!
//! A numeric field accepts any numeric literal regardless of width or sign
//! (`age > -5` on a `u32` field parses); type checks are category-level
//! (numeric / string / boolean), and cross-type numeric comparison happens at
//! evaluation time. A category mismatch such as `count == "x"` on a numeric
//! field is a parse-time [`DnfError::TypeMismatch`]. The same category check
//! applies to array operands: `age IN ["a", "b"]` on a numeric field is
//! rejected, while an empty array (`age IN []`) is accepted.

use crate::{DnfError, DnfQuery, FieldInfo};

mod query_parser;
mod token;

use query_parser::Parser;
use token::tokenize;

/// Parses a query string with explicit field metadata.
///
/// `custom_op_names` and `novalue_ops` extend the parser's vocabulary with
/// user-registered operators. Both default to empty when [`None`].
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

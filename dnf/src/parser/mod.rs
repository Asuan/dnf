//! Query string parser for DNF queries.
//!
//! Parses query strings such as
//! `(age > 18 AND country == "US") OR premium == true` into a
//! [`DnfQuery`]. The internal tokenizer and recursive-descent
//! parser are not exposed; build queries through
//! [`QueryBuilder::from_query`](crate::QueryBuilder::from_query) or
//! [`QueryBuilder::parse`](crate::QueryBuilder::parse).
//!
//! Lexer and grammar errors surface as parser-specific variants on
//! [`DnfError`] (e.g. [`DnfError::UnexpectedToken`]).

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

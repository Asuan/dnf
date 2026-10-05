use std::collections::{HashMap, HashSet};

use super::token::Token;
use crate::error::DnfError;
use crate::operator::BaseOperator;
use crate::{Condition, Conjunction, DnfQuery, FieldInfo, FieldKind, Op, Value};

/// Returns a human-readable kind name for an array element, for error messages.
fn array_element_kind(value: &Value) -> &'static str {
    match value {
        Value::String(_) => "string",
        Value::Int(_) | Value::Uint(_) => "integer",
        Value::Float(_) => "float",
        Value::Bool(_) => "boolean",
        _ => "unsupported value",
    }
}

/// Returns the number of elements in an array [`Value`], or `0` for a
/// non-array variant.
fn array_len(value: &Value) -> usize {
    match value {
        Value::StringArray(a) => a.len(),
        Value::UintArray(a) => a.len(),
        Value::IntArray(a) => a.len(),
        Value::FloatArray(a) => a.len(),
        Value::BoolArray(a) => a.len(),
        _ => 0,
    }
}

/// Returns the last `::`-separated segment of a type path, trimmed.
///
/// For example, `std::string::String` becomes `String` and a bare `str` is
/// returned unchanged.
fn last_path_segment(path: &str) -> &str {
    path.trim().rsplit("::").next().unwrap_or("").trim()
}

/// Returns the last segment of `path`, but only for bare names or paths rooted
/// at a standard-library crate (`std`, `core`, `alloc`); otherwise `None`.
fn std_path_segment(path: &str) -> Option<&str> {
    let trimmed = path.trim();
    match trimmed.split_once("::") {
        None => Some(trimmed),
        Some((root, _)) if matches!(root.trim(), "std" | "core" | "alloc") => {
            Some(last_path_segment(trimmed))
        }
        Some(_) => None,
    }
}

/// The broad value category of a field type or a parsed literal.
///
/// Numeric literals are checked against numeric fields as a single category:
/// width and sign are never gated at parse time because the evaluator compares
/// numeric [`Value`] variants cross-type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    /// Any integer or float type (`i8`..`i64`, `u8`..`u64`, `f32`, `f64`).
    Numeric,
    /// A string type (`String`, `str`, `&str`).
    Str,
    /// The `bool` type.
    Bool,
    /// Type not recognized (custom scalar, collection element, etc.).
    ///
    /// Treated leniently: accepts any literal category.
    Other,
}

/// Returns the human-readable name of a [`Category`], for error messages.
fn category_name(category: Category) -> &'static str {
    match category {
        Category::Numeric => "numeric",
        Category::Str => "string",
        Category::Bool => "boolean",
        Category::Other => "value",
    }
}

/// Shape of a condition's operand and the validation applied to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperandSpec {
    /// Scalar or array inferred from the token, no category check.
    Flexible,
    /// Scalar or array inferred from the token, scalar category checked.
    FlexibleScalar,
    /// Scalar parsed and category-checked against the field type.
    Typed,
    /// Array literal, element category checked.
    Array,
    /// Array literal of exactly two bounds, element category checked.
    Range,
}

impl OperandSpec {
    /// Returns the operand spec for a (non-novalue) operator.
    ///
    /// The match is exhaustive so a new operator must choose its operand shape.
    fn for_operator(base: &BaseOperator) -> Self {
        match base {
            BaseOperator::Eq
            | BaseOperator::Contains
            | BaseOperator::StartsWith
            | BaseOperator::EndsWith => Self::FlexibleScalar,
            BaseOperator::Comparison(_) => Self::Typed,
            BaseOperator::AllOf | BaseOperator::AnyOf => Self::Array,
            BaseOperator::Between => Self::Range,
            BaseOperator::Custom(_) => Self::Flexible,
        }
    }
}

/// Map target for map field operations.
#[derive(Debug, Clone, PartialEq, Eq)]
enum MapTarget {
    /// Access value at specific key: field["key"]
    AtKey(Box<str>),
    /// Match against keys: field.@keys
    Keys,
    /// Match against values: field.@values
    Values,
}

/// Recursive descent parser for DNF queries.
pub(crate) struct Parser<'a> {
    tokens: Vec<(Token, std::ops::Range<usize>)>, // token + byte span in the source
    current: usize,
    fields: HashMap<&'a str, (&'a str, FieldKind)>, // field_name -> (field_type, field_kind)
    input: String,                                  // Original input for error messages
    novalue_ops: HashSet<String>,                   // Operators that don't need a value
}

impl<'a> Parser<'a> {
    /// Create a new parser with spanned tokens and field information.
    pub(crate) fn new(
        tokens: Vec<(Token, std::ops::Range<usize>)>,
        fields: &'a [FieldInfo],
        input: String,
        novalue_ops: Option<&[String]>,
    ) -> Self {
        let field_map = fields
            .iter()
            .map(|f| (f.name(), (f.field_type(), f.kind())))
            .collect();

        let novalue_ops = novalue_ops
            .map(|ops| ops.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default();

        Self {
            tokens,
            current: 0,
            fields: field_map,
            input,
            novalue_ops,
        }
    }

    // Helper methods for error creation

    fn type_mismatch_error(
        &self,
        field: &str,
        expected: impl Into<Box<str>>,
        actual: impl Into<Box<str>>,
        position: usize,
    ) -> DnfError {
        DnfError::TypeMismatch {
            field: field.into(),
            expected: expected.into(),
            actual: actual.into(),
            position: Some(position),
            input: Some(self.input.as_str().into()),
        }
    }

    fn unexpected_eof(&self) -> DnfError {
        DnfError::UnexpectedEof {
            position: self.input.len(),
            input: self.input.clone(),
        }
    }

    /// Builds an `UnexpectedToken` error at the current parser position.
    fn unexpected(&self, expected: impl Into<String>) -> DnfError {
        let found = self
            .peek()
            .map(|t| t.to_string())
            .unwrap_or_else(|| "EOF".to_string());
        self.unexpected_at(expected, found, self.span().start)
    }

    /// Builds an `UnexpectedToken` error at an explicit byte offset.
    fn unexpected_at(
        &self,
        expected: impl Into<String>,
        found: impl Into<String>,
        position: usize,
    ) -> DnfError {
        DnfError::UnexpectedToken {
            expected: expected.into(),
            found: found.into(),
            position,
            input: self.input.clone(),
        }
    }

    /// Builds an `UnexpectedToken` error for a token that was just consumed via
    /// [`advance`](Self::advance), anchored at that token's start.
    fn unexpected_found(&self, expected: impl Into<String>, found: &Token) -> DnfError {
        self.unexpected_at(expected, found.to_string(), self.prev_span_start())
    }

    /// Parses a numeric literal into [`Value::Float`], [`Value::Int`], or
    /// [`Value::Uint`], inferring the variant from the literal's *shape*.
    ///
    /// The field type is never consulted: the evaluator compares numeric
    /// variants cross-type, so the literal's text alone determines the variant.
    ///
    /// | Literal shape          | Variant          |
    /// |------------------------|------------------|
    /// | contains `.`, `e`, `E` | [`Value::Float`] |
    /// | leading `-`            | [`Value::Int`]   |
    /// | otherwise              | [`Value::Uint`]  |
    ///
    /// This is the single source of truth for turning literal text into a
    /// numeric [`Value`]; other parse paths delegate here.
    ///
    /// # Errors
    ///
    /// Returns [`DnfError::InvalidNumber`] if `num` does not parse into the
    /// inferred numeric type, for example an integer literal that overflows
    /// `u64`/`i64` or a malformed float.
    fn parse_numeric_literal(&self, num: &str, position: usize) -> Result<Value, DnfError> {
        if num.contains('.') || num.contains('e') || num.contains('E') {
            num.parse::<f64>()
                .map(Value::Float)
                .map_err(|_| DnfError::InvalidNumber {
                    value: num.to_string(),
                    position,
                    input: self.input.clone(),
                })
        } else if num.starts_with('-') {
            num.parse::<i64>()
                .map(Value::Int)
                .map_err(|_| DnfError::InvalidNumber {
                    value: num.to_string(),
                    position,
                    input: self.input.clone(),
                })
        } else {
            num.parse::<u64>()
                .map(Value::Uint)
                .map_err(|_| DnfError::InvalidNumber {
                    value: num.to_string(),
                    position,
                    input: self.input.clone(),
                })
        }
    }

    /// Returns the value [`Category`] of `field_type` and whether it is `Option<T>`.
    ///
    /// The inner type of an `Option<T>` determines the category; the boolean flag
    /// reports whether a `null` literal is permitted.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(parser.field_category("u32"), (Category::Numeric, false));
    /// assert_eq!(parser.field_category("Option<String>"), (Category::Str, true));
    /// ```
    fn field_category(&self, field_type: &str) -> (Category, bool) {
        let unwrapped = self.unwrap_option(field_type);
        let optional = unwrapped.is_some();
        let base = unwrapped.unwrap_or(field_type).trim();
        let cat = if matches!(
            base,
            "i8" | "i16"
                | "i32"
                | "i64"
                | "isize"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "usize"
                | "f32"
                | "f64"
        ) {
            Category::Numeric
        } else if Self::is_str_like(base) {
            Category::Str
        } else if base == "bool" {
            Category::Bool
        } else {
            Category::Other
        };
        (cat, optional)
    }

    /// Returns the [`Category`] of a parsed literal, or `None` for `Value::None`
    /// (a `null` literal) and structural or array variants.
    fn literal_category(value: &Value) -> Option<Category> {
        match value {
            Value::Int(_) | Value::Uint(_) | Value::Float(_) => Some(Category::Numeric),
            Value::String(_) => Some(Category::Str),
            Value::Bool(_) => Some(Category::Bool),
            _ => None,
        }
    }

    /// Returns the [`Category`] of an array literal's elements, or `None` for a
    /// non-array value or an empty array (which carries no element type).
    fn array_category(value: &Value) -> Option<Category> {
        match value {
            Value::IntArray(a) if !a.is_empty() => Some(Category::Numeric),
            Value::UintArray(a) if !a.is_empty() => Some(Category::Numeric),
            Value::FloatArray(a) if !a.is_empty() => Some(Category::Numeric),
            Value::StringArray(a) if !a.is_empty() => Some(Category::Str),
            Value::BoolArray(a) if !a.is_empty() => Some(Category::Bool),
            _ => None,
        }
    }

    /// Checks a scalar literal's category against `field_type`, returning an error
    /// on a category mismatch.
    ///
    /// Non-scalar literals (`Value::None` and array variants) are skipped, and an
    /// [`Category::Other`] field accepts any literal.
    ///
    /// # Errors
    ///
    /// Returns [`DnfError::TypeMismatch`] if the literal's category differs from
    /// the field's, for example a string literal on a numeric field.
    fn check_scalar_category(
        &self,
        field_name: &str,
        field_type: &str,
        value: &Value,
        position: usize,
    ) -> Result<(), DnfError> {
        match Self::literal_category(value) {
            Some(lit_cat) => self.check_category(field_name, field_type, lit_cat, position),
            None => Ok(()),
        }
    }

    /// Checks an array operand's element category against `field_type`, returning
    /// an error on a category mismatch.
    ///
    /// Array operators (`IN`/`ANY OF`/`ALL OF`) compare each element against the
    /// scalar field, so the elements must share the field's category. Non-array
    /// and empty-array operands are skipped, and an [`Category::Other`] field
    /// accepts any elements.
    ///
    /// # Errors
    ///
    /// Returns [`DnfError::TypeMismatch`] if the element category differs from the
    /// field's, for example a string array on a numeric field.
    fn check_array_category(
        &self,
        field_name: &str,
        field_type: &str,
        value: &Value,
        position: usize,
    ) -> Result<(), DnfError> {
        match Self::array_category(value) {
            Some(lit_cat) => self.check_category(field_name, field_type, lit_cat, position),
            None => Ok(()),
        }
    }

    /// Compares a literal [`Category`] against the field's, erroring on mismatch.
    ///
    /// An [`Category::Other`] field accepts any category.
    ///
    /// # Errors
    ///
    /// Returns [`DnfError::TypeMismatch`] if `lit_cat` differs from the field's
    /// category.
    fn check_category(
        &self,
        field_name: &str,
        field_type: &str,
        lit_cat: Category,
        position: usize,
    ) -> Result<(), DnfError> {
        let (cat, _optional) = self.field_category(field_type);
        if cat == Category::Other || cat == lit_cat {
            Ok(())
        } else {
            Err(self.type_mismatch_error(
                field_name,
                category_name(cat),
                category_name(lit_cat),
                position,
            ))
        }
    }

    /// Parse the tokens into a DnfQuery.
    pub(crate) fn parse(mut self) -> Result<DnfQuery, DnfError> {
        if self.tokens.is_empty() {
            return Err(DnfError::EmptyQuery);
        }

        let conjunctions = self.parse_or_expr()?;

        // Ensure we consumed all tokens
        if !self.is_at_end() {
            return Err(self.unexpected("end of input"));
        }

        Ok(DnfQuery::from_conjunctions(conjunctions))
    }

    /// Parse OR expression: conjunction (OR conjunction)*
    fn parse_or_expr(&mut self) -> Result<Vec<Conjunction>, DnfError> {
        let mut conjunctions = vec![self.parse_conjunction()?];

        while self.match_token(&Token::Or) {
            conjunctions.push(self.parse_conjunction()?);
        }

        Ok(conjunctions)
    }

    /// Parse a conjunction: '(' condition (AND condition)* ')' | condition (AND condition)*
    fn parse_conjunction(&mut self) -> Result<Conjunction, DnfError> {
        // Check for opening parenthesis
        let has_parens = self.match_token(&Token::LeftParen);

        // Parse first condition
        let mut conditions = vec![self.parse_condition()?];

        // Parse additional AND conditions
        while self.match_token(&Token::And) {
            conditions.push(self.parse_condition()?);
        }

        // If we had an opening paren, expect a closing one
        if has_parens && !self.match_token(&Token::RightParen) {
            return Err(self.unexpected(")"));
        }

        Ok(Conjunction::from_conditions(conditions))
    }

    /// Parse a single condition: identifier operator value
    /// Supports map field syntax: field["key"], field.@keys, field.@values
    fn parse_condition(&mut self) -> Result<Condition, DnfError> {
        let field_name_str: Box<str> = match self.advance() {
            Some(Token::Identifier(name)) => name,
            Some(token) => return Err(self.unexpected_found("field identifier", &token)),
            None => return Err(self.unexpected_eof()),
        };

        let (field_type, field_kind) =
            *self
                .fields
                .get(field_name_str.as_ref())
                .ok_or_else(|| DnfError::UnknownField {
                    field_name: field_name_str.clone(),
                    position: Some(self.prev_span_start()),
                })?;

        // Check for map target syntax: .@keys, .@values, or ["key"]
        let map_target = self.parse_map_target(field_kind)?;

        let operator = self.parse_operator()?;

        // Check if this is a novalue operator
        let is_novalue = if let BaseOperator::Custom(name) = &operator.base {
            self.novalue_ops.contains(name.as_ref())
        } else {
            false
        };

        // Parse the comparison value (skip for novalue operators). Map field
        // types have no scalar category, so category checks are inert there.
        let raw_value = if is_novalue {
            Value::None
        } else {
            let spec = OperandSpec::for_operator(&operator.base);
            self.parse_operand(spec, &field_name_str, (field_type, field_kind))?
        };

        // Wrap value in map target if present
        let value = match map_target {
            Some(MapTarget::AtKey(key)) => Value::AtKey(key, Box::new(raw_value)),
            Some(MapTarget::Keys) => Value::Keys(Box::new(raw_value)),
            Some(MapTarget::Values) => Value::Values(Box::new(raw_value)),
            None => raw_value,
        };

        Ok(Condition::new(field_name_str, operator, value).with_novalue(is_novalue))
    }

    /// Returns `true` for the array [`Value`] variants.
    fn is_array(value: &Value) -> bool {
        matches!(
            value,
            Value::StringArray(_)
                | Value::UintArray(_)
                | Value::IntArray(_)
                | Value::FloatArray(_)
                | Value::BoolArray(_)
        )
    }

    /// Parses the operand of a condition according to `spec`.
    ///
    /// # Errors
    ///
    /// Returns the errors of the underlying scalar or array parse, a
    /// [`DnfError::TypeMismatch`] if the operand's category differs from the
    /// field's, or [`DnfError::UnexpectedToken`] if a range does not have
    /// exactly two bounds.
    fn parse_operand(
        &mut self,
        spec: OperandSpec,
        field_name: &str,
        (field_type, field_kind): (&str, FieldKind),
    ) -> Result<Value, DnfError> {
        let position = self.span().start;
        match spec {
            // Operand semantics are user-defined (e.g. `name FUZZY 5`).
            OperandSpec::Flexible => self.parse_value_flexible(field_name),
            OperandSpec::FlexibleScalar => {
                let value = self.parse_value_flexible(field_name)?;
                // An array operand can never equal or match a scalar field.
                if field_kind == FieldKind::Scalar && Self::is_array(&value) {
                    return Err(self.unexpected_at(
                        "scalar value",
                        "array (use IN, ANY OF, or ALL OF)",
                        position,
                    ));
                }
                self.check_scalar_category(field_name, field_type, &value, position)?;
                Ok(value)
            }
            OperandSpec::Typed => self.parse_value(field_name, field_type),
            // Rejects a bare scalar operand (e.g. `status IN "active"`):
            // `parse_array` errors unless the next token opens an array.
            OperandSpec::Array => {
                let value = self.parse_array(field_name)?;
                self.check_array_category(field_name, field_type, &value, position)?;
                Ok(value)
            }
            // Bounds are checked by element category (a numeric field rejects
            // string/bool bounds), while width/sign/float-to-int leniency stays
            // deferred to eval.
            OperandSpec::Range => {
                let value = self.parse_array(field_name)?;
                self.check_array_category(field_name, field_type, &value, position)?;
                let len = array_len(&value);
                if len != 2 {
                    return Err(self.unexpected_at(
                        "exactly 2 values",
                        format!("{len} values (BETWEEN takes exactly 2 values)"),
                        position,
                    ));
                }
                Ok(value)
            }
        }
    }

    /// Parses map target syntax: `.@keys`, `.@values`, or `["key"]` bracket access.
    ///
    /// For bracket access the key is consumed too and stored in the target.
    fn parse_map_target(&mut self, field_kind: FieldKind) -> Result<Option<MapTarget>, DnfError> {
        // The peeked token both selects the target and names the syntax for the
        // shared non-map-field guard below.
        let syntax = match self.peek() {
            Some(Token::MapKeys) => ".@keys",
            Some(Token::MapValues) => ".@values",
            Some(Token::LeftBracket) => "bracket access",
            _ => return Ok(None),
        };

        if field_kind != FieldKind::Map {
            return Err(self.unexpected_at(
                format!("map field for {syntax}"),
                format!("{field_kind:?} field"),
                self.span().start,
            ));
        }

        let target = match self.advance() {
            Some(Token::MapKeys) => MapTarget::Keys,
            Some(Token::MapValues) => MapTarget::Values,
            // Bracket access: the key follows the consumed `[`.
            _ => MapTarget::AtKey(self.parse_bracket_key()?),
        };
        Ok(Some(target))
    }

    /// Parse the key from bracket notation: ["key"]
    fn parse_bracket_key(&mut self) -> Result<Box<str>, DnfError> {
        let key: Box<str> = match self.advance() {
            Some(Token::String(s)) => s,
            Some(token) => return Err(self.unexpected_found("string key", &token)),
            None => return Err(self.unexpected_eof()),
        };

        // Expect closing bracket
        if !self.match_token(&Token::RightBracket) {
            return Err(self.unexpected("]"));
        }

        Ok(key)
    }

    /// Parse an operator token.
    fn parse_operator(&mut self) -> Result<Op, DnfError> {
        match self.advance() {
            Some(Token::Eq) => Ok(Op::EQ),
            Some(Token::Ne) => Ok(Op::NE),
            Some(Token::Gt) => Ok(Op::GT),
            Some(Token::Lt) => Ok(Op::LT),
            Some(Token::Gte) => Ok(Op::GTE),
            Some(Token::Lte) => Ok(Op::LTE),
            Some(Token::Contains) => Ok(Op::CONTAINS),
            Some(Token::NotContains) => Ok(Op::NOT_CONTAINS),
            Some(Token::StartsWith) => Ok(Op::STARTS_WITH),
            Some(Token::EndsWith) => Ok(Op::ENDS_WITH),
            Some(Token::NotStartsWith) => Ok(Op::NOT_STARTS_WITH),
            Some(Token::NotEndsWith) => Ok(Op::NOT_ENDS_WITH),
            Some(Token::AllOf) => Ok(Op::ALL_OF),
            Some(Token::AnyOf) => Ok(Op::ANY_OF),
            Some(Token::NotAllOf) => Ok(Op::NOT_ALL_OF),
            Some(Token::NotAnyOf) => Ok(Op::NOT_ANY_OF),
            Some(Token::Between) => Ok(Op::BETWEEN),
            Some(Token::NotBetween) => Ok(Op::NOT_BETWEEN),
            Some(Token::CustomOp(name)) => Ok(Op::custom(name)),
            Some(Token::NotCustomOp(name)) => Ok(Op::not_custom(name)),
            Some(token) => Err(self.unexpected_found("operator", &token)),
            None => Err(self.unexpected_eof()),
        }
    }

    /// Parses a scalar value and checks its category against `field_type`.
    ///
    /// The literal's variant is inferred from its *shape* (see
    /// [`parse_numeric_literal`](Self::parse_numeric_literal)); the field type is
    /// consulted only for a coarse category check, never to gate a numeric
    /// literal's width or sign. A `null` literal is accepted only for `Option<T>`.
    ///
    /// # Errors
    ///
    /// Returns [`DnfError::TypeMismatch`] if the literal's category differs from
    /// the field's (e.g. a string literal on a numeric field, or `null` on a
    /// non-`Option` field), [`DnfError::InvalidNumber`] for a malformed numeric
    /// literal, [`DnfError::UnexpectedToken`] for a non-value token, and
    /// [`DnfError::UnexpectedEof`] at end of input.
    fn parse_value(&mut self, field_name: &str, field_type: &str) -> Result<Value, DnfError> {
        let position = self.span().start;
        let token = self.advance().ok_or_else(|| self.unexpected_eof())?;
        let value = self.scalar_from_token(token, position, "value")?;

        // `scalar_from_token` never yields an array, so the only uncategorized
        // scalar is `Value::None` (a `null` literal), allowed only for `Option<T>`.
        if matches!(value, Value::None) {
            let (_, optional) = self.field_category(field_type);
            if !optional {
                return Err(self.type_mismatch_error(field_name, field_type, "null", position));
            }
            return Ok(value);
        }

        self.check_scalar_category(field_name, field_type, &value, position)?;
        Ok(value)
    }

    /// Parse a value without field type constraints (for string operators).
    /// Accepts strings, numbers, booleans, and arrays, inferring types from the token.
    fn parse_value_flexible(&mut self, field_name: &str) -> Result<Value, DnfError> {
        let position = self.span().start;

        // Check for array literal
        if self.peek() == Some(&Token::LeftBracket) {
            return self.parse_array(field_name);
        }

        let token = self.advance().ok_or_else(|| self.unexpected_eof())?;
        self.scalar_from_token(token, position, "value or array")
    }

    /// Maps `values` through `extract` and collects into a boxed slice.
    ///
    /// Each item carries its source byte offset so `extract` can anchor a
    /// [`DnfError::TypeMismatch`] at the offending element.
    fn collect_array<T, F>(values: Vec<(Value, usize)>, extract: F) -> Result<Box<[T]>, DnfError>
    where
        F: FnMut((Value, usize)) -> Result<T, DnfError>,
    {
        values
            .into_iter()
            .map(extract)
            .collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice)
    }

    fn parse_array(&mut self, field_name: &str) -> Result<Value, DnfError> {
        // Consume '['
        if !self.match_token(&Token::LeftBracket) {
            return Err(self.unexpected("["));
        }

        // Handle empty array
        if self.match_token(&Token::RightBracket) {
            // Default to empty string array
            return Ok(Value::StringArray(
                Vec::<Box<str>>::new().into_boxed_slice(),
            ));
        }

        // Parse first element to determine array type
        let mut elements = vec![self.parse_array_element()?];

        // Parse remaining elements
        while self.match_token(&Token::Comma) {
            elements.push(self.parse_array_element()?);
        }

        // Consume ']'
        if !self.match_token(&Token::RightBracket) {
            return Err(self.unexpected("] or ,"));
        }

        self.build_array(field_name, elements)
    }

    /// Builds a homogeneous array [`Value`] from parsed elements.
    ///
    /// Numeric elements are promoted to a common type so mixed-sign and
    /// int/float literals parse: `Uint` widens to `Int` when any element is
    /// signed, and everything widens to `Float` when any element is a float.
    /// Thus `[-5, 0, 5]` yields an [`Value::IntArray`] and `[1, 2.5]` a
    /// [`Value::FloatArray`]. Strings and booleans are not promoted and must be
    /// uniform.
    ///
    /// # Errors
    ///
    /// Returns [`DnfError::TypeMismatch`] if the elements mix incompatible
    /// kinds (e.g. strings with numbers) or if a positive value in an otherwise
    /// signed array exceeds [`i64::MAX`].
    fn build_array(
        &self,
        field_name: &str,
        elements: Vec<(Value, usize)>,
    ) -> Result<Value, DnfError> {
        let all_numeric = elements
            .iter()
            .all(|(v, _)| matches!(v, Value::Int(_) | Value::Uint(_) | Value::Float(_)));

        if all_numeric {
            // `Int` only arises from a leading `-`, so "any signed" == "any negative".
            let any_float = elements.iter().any(|(v, _)| matches!(v, Value::Float(_)));
            let any_signed = elements.iter().any(|(v, _)| matches!(v, Value::Int(_)));

            if any_float {
                let floats: Vec<f64> = elements
                    .into_iter()
                    .map(|(v, _)| match v {
                        Value::Float(f) => f,
                        Value::Int(i) => i as f64,
                        Value::Uint(u) => u as f64,
                        _ => unreachable!("all elements are numeric"),
                    })
                    .collect();
                return Ok(Value::FloatArray(floats.into_boxed_slice()));
            }
            if any_signed {
                return Self::collect_array(elements, |(v, pos)| match v {
                    Value::Int(i) => Ok(i),
                    Value::Uint(u) if u <= i64::MAX as u64 => Ok(u as i64),
                    Value::Uint(u) => Err(self.type_mismatch_error(
                        field_name.as_ref(),
                        format!("signed integer (max {})", i64::MAX),
                        format!("unsigned integer {u}"),
                        pos,
                    )),
                    _ => unreachable!("all elements are numeric"),
                })
                .map(Value::IntArray);
            }
            let uints: Vec<u64> = elements
                .into_iter()
                .map(|(v, _)| match v {
                    Value::Uint(u) => u,
                    _ => unreachable!("no float and no signed element means all uint"),
                })
                .collect();
            return Ok(Value::UintArray(uints.into_boxed_slice()));
        }

        // Non-numeric arrays must be uniformly strings or uniformly booleans.
        let first_position = elements[0].1;
        match &elements[0].0 {
            Value::String(_) => Self::collect_array(elements, |(v, pos)| match v {
                Value::String(s) => Ok(s),
                other => Err(self.type_mismatch_error(
                    field_name.as_ref(),
                    "string elements",
                    array_element_kind(&other),
                    pos,
                )),
            })
            .map(Value::StringArray),
            Value::Bool(_) => Self::collect_array(elements, |(v, pos)| match v {
                Value::Bool(b) => Ok(b),
                other => Err(self.type_mismatch_error(
                    field_name.as_ref(),
                    "boolean elements",
                    array_element_kind(&other),
                    pos,
                )),
            })
            .map(Value::BoolArray),
            other => Err(self.type_mismatch_error(
                field_name.as_ref(),
                "string, number, or boolean elements",
                array_element_kind(other),
                first_position,
            )),
        }
    }

    /// Parse a single array element (string, number, or boolean), returning it
    /// paired with its source byte offset for later diagnostics.
    fn parse_array_element(&mut self) -> Result<(Value, usize), DnfError> {
        let position = self.span().start;
        let token = self.advance().ok_or_else(|| self.unexpected_eof())?;

        let value = self.scalar_from_token(
            token,
            position,
            "array element (string, number, or boolean)",
        )?;
        Ok((value, position))
    }

    /// Helper function to parse scalar values from tokens.
    fn scalar_from_token(
        &self,
        token: Token,
        position: usize,
        expected: &str,
    ) -> Result<Value, DnfError> {
        match token {
            Token::String(s) => Ok(Value::String(s)),
            Token::Number(num) => self.parse_numeric_literal(&num, position),
            Token::Boolean(b) => Ok(Value::Bool(b)),
            Token::Null => Ok(Value::None),
            token => Err(self.unexpected_at(expected, token.to_string(), position)),
        }
    }

    /// Returns `true` for `str`/`String` spellings, following references and the
    /// `Box`/`Cow`/`Rc`/`Arc` wrappers down to their innermost type.
    fn is_str_like(type_str: &str) -> bool {
        let trimmed = type_str.trim();

        // Wrapper types like `Box<str>` or `Cow<'_, str>`: recurse on the last
        // generic argument (the type, after any lifetime argument).
        if let (Some(open), Some(end)) = (trimmed.find('<'), trimmed.rfind('>')) {
            if open < end {
                if let Some(wrapper) = std_path_segment(&trimmed[..open]) {
                    if matches!(wrapper, "Box" | "Cow" | "Rc" | "Arc") {
                        let last_arg = trimmed[open + 1..end].rsplit(',').next().unwrap_or("");
                        return Self::is_str_like(last_arg);
                    }
                }
            }
            return false;
        }

        // Strip a leading reference and any lifetime / `mut`: `&'a str` -> `str`.
        let inner = trimmed
            .strip_prefix('&')
            .map_or(trimmed, |r| r.split_whitespace().last().unwrap_or(""));

        matches!(std_path_segment(inner), Some("String") | Some("str"))
    }

    /// Unwraps `Option<T>`, returning the inner type `T`.
    ///
    /// Accepts spacing variants (`Option < T >`) and fully-qualified paths
    /// (`std::option::Option<T>`, `core::option::Option<T>`).
    fn unwrap_option<'b>(&self, type_str: &'b str) -> Option<&'b str> {
        let trimmed = type_str.trim();

        let open = trimmed.find('<')?;
        if std_path_segment(&trimmed[..open]) != Some("Option") {
            return None;
        }
        let end = trimmed.rfind('>')?;
        if open < end {
            Some(trimmed[open + 1..end].trim())
        } else {
            None
        }
    }

    /// Check if current token matches the expected token and advance if so.
    fn match_token(&mut self, expected: &Token) -> bool {
        if let Some(token) = self.peek() {
            if std::mem::discriminant(token) == std::mem::discriminant(expected) {
                self.current += 1;
                return true;
            }
        }
        false
    }

    /// Get the current token without consuming it.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.current).map(|(token, _)| token)
    }

    /// Returns the byte span of the current token.
    ///
    /// At end of input the span collapses to `input.len()..input.len()`, so
    /// callers can use `span().start` as a valid trailing byte offset.
    fn span(&self) -> std::ops::Range<usize> {
        self.tokens
            .get(self.current)
            .map(|(_, span)| span.clone())
            .unwrap_or(self.input.len()..self.input.len())
    }

    /// Returns the byte-offset start of the previously consumed token.
    ///
    /// Falls back to `input.len()` before any token has been consumed.
    fn prev_span_start(&self) -> usize {
        self.current
            .checked_sub(1)
            .and_then(|i| self.tokens.get(i))
            .map(|(_, span)| span.start)
            .unwrap_or(self.input.len())
    }

    /// Consume and return ownership of the current token.
    fn advance(&mut self) -> Option<Token> {
        if !self.is_at_end() {
            let i = self.current;
            self.current += 1;
            Some(std::mem::replace(&mut self.tokens[i].0, Token::Consumed))
        } else {
            None
        }
    }

    /// Check if we've consumed all tokens.
    fn is_at_end(&self) -> bool {
        self.current >= self.tokens.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::token::tokenize;

    // Type aliases for complex test case types (to satisfy clippy::type_complexity)
    type ValueValidator = Box<dyn Fn(&Value) -> bool>;
    type TypeInferenceCase = (&'static str, &'static str, ValueValidator);
    type ArrayTestCase = (&'static str, &'static str, Vec<FieldInfo>, ValueValidator);

    // ==================== Basic Parsing Tests ====================

    struct ParseTestCase {
        name: &'static str,
        query: &'static str,
        fields: Vec<FieldInfo>,
        expected_conjunctions: usize,
        expected_conditions: Vec<usize>, // conditions per conjunction
    }

    #[test]
    fn test_parse_basic_queries() {
        let cases = vec![
            ParseTestCase {
                name: "simple condition",
                query: "age > 18",
                fields: vec![FieldInfo::new("age", "u32")],
                expected_conjunctions: 1,
                expected_conditions: vec![1],
            },
            ParseTestCase {
                name: "AND conjunction",
                query: "age > 18 AND country == \"US\"",
                fields: vec![
                    FieldInfo::new("age", "u32"),
                    FieldInfo::new("country", "String"),
                ],
                expected_conjunctions: 1,
                expected_conditions: vec![2],
            },
            ParseTestCase {
                name: "OR disjunction",
                query: "age > 18 OR premium == true",
                fields: vec![
                    FieldInfo::new("age", "u32"),
                    FieldInfo::new("premium", "bool"),
                ],
                expected_conjunctions: 2,
                expected_conditions: vec![1, 1],
            },
            ParseTestCase {
                name: "complex query with parentheses",
                query: "(age > 18 AND country == \"US\") OR (premium == true AND verified == true)",
                fields: vec![
                    FieldInfo::new("age", "u32"),
                    FieldInfo::new("country", "String"),
                    FieldInfo::new("premium", "bool"),
                    FieldInfo::new("verified", "bool"),
                ],
                expected_conjunctions: 2,
                expected_conditions: vec![2, 2],
            },
        ];

        for case in cases {
            let tokens = tokenize(case.query, None).unwrap();
            let parser = Parser::new(tokens, &case.fields, case.query.to_string(), None);
            let query = parser
                .parse()
                .unwrap_or_else(|e| panic!("Failed to parse '{}': {:?}", case.name, e));

            assert_eq!(
                query.conjunctions().len(),
                case.expected_conjunctions,
                "Conjunction count mismatch for '{}'",
                case.name
            );

            for (i, &expected_count) in case.expected_conditions.iter().enumerate() {
                assert_eq!(
                    query.conjunctions()[i].conditions().len(),
                    expected_count,
                    "Condition count mismatch for '{}' conjunction {}",
                    case.name,
                    i
                );
            }
        }
    }

    #[test]
    fn test_parse_operators() {
        // Every surface operator spelling maps to the expected `BaseOperator`.
        // Field types are chosen to satisfy each operator's category check.
        let cases = vec![
            // (query, field_name, field_type, expected base operator)
            ("age == 18", "age", "u32", Op::EQ.base),
            ("age != 18", "age", "u32", Op::NE.base),
            ("age > 18", "age", "u32", Op::GT.base),
            ("age < 18", "age", "u32", Op::LT.base),
            ("age >= 18", "age", "u32", Op::GTE.base),
            ("age <= 18", "age", "u32", Op::LTE.base),
            ("name CONTAINS \"J\"", "name", "String", Op::CONTAINS.base),
            (
                "name NOT CONTAINS \"J\"",
                "name",
                "String",
                Op::NOT_CONTAINS.base,
            ),
            (
                "name STARTS WITH \"J\"",
                "name",
                "String",
                Op::STARTS_WITH.base,
            ),
            ("name ENDS WITH \"n\"", "name", "String", Op::ENDS_WITH.base),
            (
                "name NOT STARTS WITH \"J\"",
                "name",
                "String",
                Op::NOT_STARTS_WITH.base,
            ),
            (
                "name NOT ENDS WITH \"n\"",
                "name",
                "String",
                Op::NOT_ENDS_WITH.base,
            ),
            // `IN` is the only spelling for ANY OF; `ANY OF` is not a keyword.
            ("tags IN [1, 2]", "tags", "u32", Op::ANY_OF.base),
            ("tags ALL OF [1, 2]", "tags", "u32", Op::ALL_OF.base),
            ("tags NOT IN [1, 2]", "tags", "u32", Op::NOT_ANY_OF.base),
            ("tags NOT ALL OF [1, 2]", "tags", "u32", Op::NOT_ALL_OF.base),
            ("age BETWEEN [1, 2]", "age", "u32", Op::BETWEEN.base),
            ("age NOT BETWEEN [1, 2]", "age", "u32", Op::NOT_BETWEEN.base),
        ];

        for (query, field_name, field_type, expected_op) in cases {
            let tokens = tokenize(query, None).unwrap();
            let fields = vec![FieldInfo::new(field_name, field_type)];
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser
                .parse()
                .unwrap_or_else(|e| panic!("Failed to parse '{}': {:?}", query, e));

            assert_eq!(
                result.conjunctions()[0].conditions()[0].operator().base,
                expected_op,
                "Operator mismatch for: {}",
                query
            );
        }
    }

    // ==================== Type Inference Tests ====================

    #[test]
    fn test_parse_type_inference() {
        let cases: Vec<TypeInferenceCase> = vec![
            (
                "age > 18",
                "u32",
                Box::new(|v| matches!(v, Value::Uint(18))),
            ),
            (
                "count > -5",
                "i32",
                Box::new(|v| matches!(v, Value::Int(-5))),
            ),
            (
                "premium == true",
                "bool",
                Box::new(|v| matches!(v, Value::Bool(true))),
            ),
            (
                r#"name == """#,
                "String",
                Box::new(|v| matches!(v, Value::String(s) if s.as_ref() == "")),
            ),
            (
                "value == null",
                "Option < String >",
                Box::new(|v| matches!(v, Value::None)),
            ),
        ];

        for (query, field_type, validator) in cases {
            let tokens = tokenize(query, None).unwrap();
            let fields = vec![
                FieldInfo::new("age", field_type),
                FieldInfo::new("count", field_type),
                FieldInfo::new("premium", field_type),
                FieldInfo::new("name", field_type),
                FieldInfo::new("value", field_type),
            ];
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser.parse().unwrap();

            let value = result.conjunctions()[0].conditions()[0].value();
            assert!(
                validator(value),
                "Type inference failed for '{}', got: {:?}",
                query,
                value
            );
        }
    }

    // ==================== Category Compatibility Tests ====================

    #[test]
    fn test_parse_cross_numeric_accepted() {
        // A numeric field accepts any numeric literal, regardless of the literal's
        // sign or the field's width/signedness — the evaluator compares cross-type.
        let cases = vec![
            ("age > -5", "u32", "negative literal on unsigned field"),
            ("n == 5", "i32", "unsigned literal on signed field"),
            ("n == 5", "u64", "unsigned literal on unsigned field"),
            ("n < 10", "f64", "integer literal on float field"),
            ("n >= 5", "usize", "unsigned literal on usize field"),
        ];

        for (query, field_type, desc) in cases {
            let field_name = query.split_whitespace().next().unwrap();
            let fields = vec![FieldInfo::new(field_name, field_type)];
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            assert!(
                parser.parse().is_ok(),
                "Failed: {} ('{}' on {})",
                desc,
                query,
                field_type
            );
        }
    }

    #[test]
    fn test_parse_category_mismatch() {
        // A literal whose category differs from the field's is rejected, on both
        // the typed path (`>`) and the flexible path (`==`).
        let cases = vec![
            ("name == 5", "String", "numeric literal on string field"),
            ("flag > 1", "bool", "numeric literal on bool field"),
            (
                "count == \"x\"",
                "u32",
                "string literal on numeric field (flexible path)",
            ),
        ];

        for (query, field_type, desc) in cases {
            let field_name = query.split_whitespace().next().unwrap();
            let fields = vec![FieldInfo::new(field_name, field_type)];
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser.parse();
            assert!(
                matches!(result, Err(DnfError::TypeMismatch { .. })),
                "Failed: {} ('{}' on {}), got: {:?}",
                desc,
                query,
                field_type,
                result
            );
        }
    }

    #[test]
    fn test_parse_null_optionality() {
        // On the typed path (comparison operators), `null` is accepted only for
        // `Option<T>`; a non-`Option` field rejects it with a `TypeMismatch` whose
        // `actual` is "null". (Flexible operators like `==` skip this check.)
        let ok_cases = vec![
            ("nick > null", "Option<String>"),
            ("age > null", "Option<u32>"),
        ];
        for (query, field_type) in ok_cases {
            let field_name = query.split_whitespace().next().unwrap();
            let fields = vec![FieldInfo::new(field_name, field_type)];
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            assert!(
                parser.parse().is_ok(),
                "null should parse on {}: '{}'",
                field_type,
                query
            );
        }

        let err_cases = vec![("name > null", "String"), ("age > null", "u32")];
        for (query, field_type) in err_cases {
            let field_name = query.split_whitespace().next().unwrap();
            let fields = vec![FieldInfo::new(field_name, field_type)];
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser.parse();
            assert!(
                matches!(
                    &result,
                    Err(DnfError::TypeMismatch { actual, .. }) if actual.as_ref() == "null"
                ),
                "null should be rejected on non-Option {}: got {:?}",
                field_type,
                result
            );
        }
    }

    #[test]
    fn test_parse_null_accepted_on_flexible_ops() {
        // On the flexible path a `null` operand parses leniently to
        // `Value::None` for every scalar operator and string spelling — it is
        // never a parse error. Equality (`==`/`!=`) is a genuine null/None
        // check, and the string operators resolve a null operand to `false` at
        // eval time (see `tests/parser_type_leniency.rs`). Array operators
        // (`IN`/`ANY OF`/`ALL OF`) are excluded: they require an array literal.
        let cases = vec![
            ("name == null", "String"),
            ("name != null", "String"),
            ("name == null", "&str"),
            ("name != null", "&str"),
            ("name == null", "Option<String>"),
            ("name == null", "Option<&str>"),
            ("name CONTAINS null", "String"),
            ("name NOT CONTAINS null", "String"),
            ("name STARTS WITH null", "String"),
            ("name ENDS WITH null", "String"),
            ("name CONTAINS null", "&str"),
            ("name STARTS WITH null", "&str"),
        ];
        for (query, field_type) in cases {
            let fields = vec![FieldInfo::new("name", field_type)];
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let parsed = parser
                .parse()
                .unwrap_or_else(|e| panic!("'{}' on {} should parse: {:?}", query, field_type, e));
            assert!(
                matches!(
                    parsed.conjunctions()[0].conditions()[0].value(),
                    Value::None
                ),
                "'{}' should yield Value::None",
                query
            );
        }
    }

    #[test]
    fn test_parse_other_type_is_lenient() {
        // A field whose type is not recognized as numeric/string/bool falls into
        // `Category::Other` and accepts any literal category, on both the typed
        // (`>`) and flexible (`==`) paths.
        let cases = vec![
            ("tag == 5", "MyId"),
            ("tag == \"x\"", "MyId"),
            ("tag == true", "MyId"),
            ("tag > 5", "MyId"),
        ];
        for (query, field_type) in cases {
            let fields = vec![FieldInfo::new("tag", field_type)];
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            assert!(
                parser.parse().is_ok(),
                "Other-category field should accept any literal: '{}'",
                query
            );
        }
    }

    #[test]
    fn test_parse_scalar_happy_paths() {
        // A literal whose category matches the field parses on the typed path.
        let cases = vec![
            ("name == \"John\"", "String"),
            ("flag == true", "bool"),
            ("age > 18", "u32"),
        ];
        for (query, field_type) in cases {
            let field_name = query.split_whitespace().next().unwrap();
            let fields = vec![FieldInfo::new(field_name, field_type)];
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            assert!(
                parser.parse().is_ok(),
                "matching literal should parse: '{}'",
                query
            );
        }
    }

    #[test]
    fn test_parse_string_option_type_detection() {
        // Field-type spellings recognized as string / optional-string, plus
        // rejections that confirm the detection is not overly broad.
        let cases = vec![
            // Each spelling is recognized as a string field: a string literal is
            // accepted and a numeric literal is rejected (proving `Str`, not the
            // lenient `Other`, category).
            (
                "&str",
                "name == \"x\"",
                true,
                "reference str accepts string",
            ),
            (
                "&'a str",
                "name == \"x\"",
                true,
                "reference str with lifetime accepts string",
            ),
            (
                "std::string::String",
                "name == \"x\"",
                true,
                "fully-qualified String accepts string",
            ),
            (
                "core::primitive::str",
                "name == 5",
                false,
                "fully-qualified str rejects numeric",
            ),
            ("Box<str>", "name == 5", false, "boxed str rejects numeric"),
            (
                "Cow<'_, str>",
                "name == \"x\"",
                true,
                "cow with lifetime accepts string",
            ),
            (
                "Cow<str>",
                "name == 5",
                false,
                "cow without lifetime rejects numeric",
            ),
            // A non-string wrapper is still lenient (`Other`), so numeric passes.
            (
                "Vec<u8>",
                "name == 5",
                true,
                "non-string wrapper stays lenient",
            ),
            // Qualified `Option<..>` spellings unwrap, so the typed `>` path sees
            // an optional field and permits `null`; the non-optional field does not.
            (
                "std::option::Option<String>",
                "name > null",
                true,
                "qualified Option permits null on typed path",
            ),
            (
                "std::string::String",
                "name > null",
                false,
                "non-optional String rejects null on typed path",
            ),
            // `alloc`-rooted standard spellings are still recognized.
            (
                "alloc::string::String",
                "name == 5",
                false,
                "alloc String is str-like and rejects numeric",
            ),
            // A user type in another module that merely ends in `String` must not
            // be mistaken for the standard type: it stays lenient (`Other`), so a
            // numeric literal is accepted.
            (
                "foo::String",
                "name == 5",
                true,
                "non-std String stays lenient",
            ),
            // Likewise a user `Option` in another module does not unwrap, so the
            // typed path sees a non-optional field and rejects `null`.
            (
                "foo::Option<String>",
                "name > null",
                false,
                "non-std Option does not unwrap",
            ),
        ];

        for (field_type, query, expect_ok, desc) in cases {
            let fields = vec![FieldInfo::new("name", field_type)];
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            assert_eq!(
                parser.parse().is_ok(),
                expect_ok,
                "Failed: {} ('{}' on '{}')",
                desc,
                query,
                field_type
            );
        }
    }

    #[test]
    fn test_parse_bool_string_cross_category_rejected() {
        // Cross-category literals are rejected on the typed path in both
        // directions for bool and string fields.
        let cases = vec![
            ("name == true", "String", "bool literal on string field"),
            ("flag == \"yes\"", "bool", "string literal on bool field"),
            ("age == \"x\"", "u32", "string literal on numeric field"),
        ];
        for (query, field_type, desc) in cases {
            let field_name = query.split_whitespace().next().unwrap();
            let fields = vec![FieldInfo::new(field_name, field_type)];
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser.parse();
            assert!(
                matches!(result, Err(DnfError::TypeMismatch { .. })),
                "Failed: {} ('{}'), got: {:?}",
                desc,
                query,
                result
            );
        }
    }

    #[test]
    fn test_parse_exponent_literals() {
        // Exponent literals parse as floats on a float field.
        let cases = vec![
            ("x == 1e5", 100_000.0, "integer mantissa"),
            ("x == 1.5e3", 1_500.0, "fractional mantissa"),
            ("x == 2e-3", 0.002, "negative exponent"),
        ];

        for (query, expected, desc) in cases {
            let fields = vec![FieldInfo::new("x", "f64")];
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let parsed = parser
                .parse()
                .unwrap_or_else(|e| panic!("Failed to parse '{}': {:?}", query, e));
            let value = parsed.conjunctions()[0].conditions()[0].value();
            assert!(
                matches!(value, Value::Float(f) if (*f - expected).abs() < f64::EPSILON),
                "Failed: {} ('{}'), got: {:?}",
                desc,
                query,
                value
            );
        }
    }

    // ==================== Error Tests ====================

    #[test]
    fn test_parse_errors() {
        // Each case asserts both the error variant and that the rendered message
        // carries the expected substrings — including the field *name* (not its
        // type) for a type mismatch.
        let cases = vec![
            // (query, fields, must-contain substrings)
            (
                "unknown > 18",
                vec![FieldInfo::new("age", "u32")],
                vec!["UnknownField"],
            ),
            (
                "age > \"not a number\"",
                vec![FieldInfo::new("age", "u32")],
                vec!["TypeMismatch"],
            ),
            // A type mismatch names the field (`name`), not its type (`String`).
            (
                "name == 5",
                vec![FieldInfo::new("name", "String")],
                vec!["TypeMismatch", "name"],
            ),
        ];

        for (query, fields, expected_substrings) in cases {
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser.parse();

            assert!(result.is_err(), "Expected error for: {}", query);
            let err_str = format!("{:?}", result.unwrap_err());
            for expected in expected_substrings {
                assert!(
                    err_str.contains(expected),
                    "Expected '{}' in error for '{}', got: {}",
                    expected,
                    query,
                    err_str
                );
            }
        }
    }

    #[test]
    fn test_parse_error_positions() {
        // Parser errors still carry the byte offset of the offending token in
        // their `position` field. A `TypeMismatch` reports it SQL-style as a
        // `near` snippet (no bare "at position"); `UnexpectedToken` keeps both.
        // The `café` case confirms multi-byte fields keep offsets on char
        // boundaries.
        struct PosCase {
            query: &'static str,
            field: &'static str,
            field_type: &'static str,
            expected_pos: usize,
            /// Whether the message quotes the literal `at position N` (only the
            /// `UnexpectedToken` family does; `TypeMismatch` shows `near` only).
            expect_position_text: bool,
            desc: &'static str,
        }

        let cases = vec![
            PosCase {
                query: r#"age > "x""#,
                field: "age",
                field_type: "u32",
                expected_pos: 6,
                expect_position_text: false, // TypeMismatch renders `near` only
                desc: "string literal on numeric field",
            },
            PosCase {
                query: "age > abc",
                field: "age",
                field_type: "u32",
                expected_pos: 6,
                expect_position_text: true, // UnexpectedToken keeps position + near
                desc: "identifier where a value is expected",
            },
            PosCase {
                query: "name == 5",
                field: "name",
                field_type: "String",
                expected_pos: 8,
                expect_position_text: false,
                desc: "flexible-path category mismatch",
            },
            PosCase {
                query: r#"café > "x""#,
                field: "café",
                field_type: "u32",
                expected_pos: 8,
                expect_position_text: false,
                desc: "multi-byte field name keeps byte offset",
            },
        ];

        for case in cases {
            let fields = vec![FieldInfo::new(case.field, case.field_type)];
            let tokens = tokenize(case.query, None).unwrap();
            let parser = Parser::new(tokens, &fields, case.query.to_string(), None);
            let err = parser.parse().unwrap_err();

            let position = match &err {
                DnfError::TypeMismatch {
                    position: Some(p), ..
                } => *p,
                DnfError::UnexpectedToken { position, .. } => *position,
                other => panic!("Unexpected error for '{}': {:?}", case.desc, other),
            };
            assert_eq!(
                position, case.expected_pos,
                "Wrong position for '{}': {}",
                case.desc, case.query
            );

            // Every parse-time error quotes the offending text as a `near` snippet.
            let msg = err.to_string();
            assert!(
                msg.contains("near"),
                "message for '{}' should show a context snippet: {}",
                case.desc,
                msg
            );
            assert_eq!(
                msg.contains(&format!("at position {}", case.expected_pos)),
                case.expect_position_text,
                "message for '{}' position-text expectation not met: {}",
                case.desc,
                msg
            );
        }
    }

    #[test]
    fn test_parse_empty() {
        let tokens = vec![];
        let fields = vec![FieldInfo::new("age", "u32")];
        let parser = Parser::new(tokens, &fields, String::new(), None);
        let result = parser.parse();

        assert!(matches!(result, Err(DnfError::EmptyQuery)));
    }

    // ==================== Array Tests ====================

    #[test]
    fn test_parse_arrays() {
        let cases: Vec<ArrayTestCase> = vec![
            (
                "string array",
                r#"status IN ["active", "pending"]"#,
                vec![FieldInfo::new("status", "String")],
                Box::new(|v: &Value| matches!(v, Value::StringArray(arr) if arr.len() == 2)),
            ),
            (
                "uint array",
                "age IN [18, 21, 25]",
                vec![FieldInfo::new("age", "u32")],
                Box::new(
                    |v: &Value| matches!(v, Value::UintArray(arr) if arr.len() == 3 && arr[0] == 18),
                ),
            ),
            (
                "int array (negatives)",
                "value IN [-10, -5, -1]",
                vec![FieldInfo::new("value", "i32")],
                Box::new(
                    |v: &Value| matches!(v, Value::IntArray(arr) if arr.len() == 3 && arr[0] == -10),
                ),
            ),
            (
                "float array",
                "price IN [9.99, 19.99, 29.99]",
                vec![FieldInfo::new("price", "f64")],
                Box::new(|v: &Value| matches!(v, Value::FloatArray(arr) if arr.len() == 3)),
            ),
            (
                "mixed-sign array promotes to int",
                "value IN [-5, 0, 5]",
                vec![FieldInfo::new("value", "i32")],
                Box::new(
                    |v: &Value| matches!(v, Value::IntArray(arr) if arr.as_ref() == [-5, 0, 5]),
                ),
            ),
            (
                "int/float mix promotes to float",
                "price IN [1, 2, 3.5]",
                vec![FieldInfo::new("price", "f64")],
                Box::new(
                    |v: &Value| matches!(v, Value::FloatArray(arr) if arr.as_ref() == [1.0, 2.0, 3.5]),
                ),
            ),
            (
                "boolean array",
                "flags IN [true, false]",
                vec![FieldInfo::new("flags", "bool")],
                Box::new(|v: &Value| matches!(v, Value::BoolArray(arr) if arr.len() == 2)),
            ),
            (
                "empty array",
                "status IN []",
                vec![FieldInfo::new("status", "String")],
                Box::new(|v: &Value| matches!(v, Value::StringArray(arr) if arr.is_empty())),
            ),
            (
                "single element array",
                r#"status IN ["active"]"#,
                vec![FieldInfo::new("status", "String")],
                Box::new(|v: &Value| matches!(v, Value::StringArray(arr) if arr.len() == 1)),
            ),
        ];

        for (name, query, fields_vec, validator) in cases {
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields_vec, query.to_string(), None);
            let result = parser.parse().unwrap();

            let value = result.conjunctions()[0].conditions()[0].value();
            assert!(
                validator(value),
                "Array validation failed for '{}': {:?}",
                name,
                value
            );
        }
    }

    #[test]
    fn test_parse_array_operator_requires_array_literal() {
        // Array operators (ANY OF / ALL OF and their NOT / IN aliases) must be
        // given an array literal; a bare scalar operand is a syntax error.
        let cases = vec![
            ("IN scalar string", r#"status IN "active""#, "String"),
            (
                "ANY OF scalar string",
                r#"status ANY OF "active""#,
                "String",
            ),
            (
                "ALL OF scalar string",
                r#"status ALL OF "active""#,
                "String",
            ),
            ("IN scalar number", "age IN 18", "u32"),
            ("NOT IN scalar string", r#"status NOT IN "x""#, "String"),
        ];

        for (name, query, field_type) in cases {
            let tokens = tokenize(query, None).unwrap();
            let fields = vec![
                FieldInfo::new("status", field_type),
                FieldInfo::new("age", field_type),
            ];
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser.parse();

            assert!(
                matches!(result, Err(DnfError::UnexpectedToken { .. })),
                "Expected UnexpectedToken for '{}', got: {:?}",
                name,
                result
            );
        }
    }

    #[test]
    fn test_parse_array_operator_element_category() {
        // Array operators compare each element against the scalar field, so the
        // elements must share the field's category. Mismatches are rejected at
        // parse time; matching and `Other`/empty operands are accepted.
        let cases = vec![
            // (name, query, field_type, expect_ok)
            (
                "string array on numeric field",
                r#"age IN ["a", "b"]"#,
                "u32",
                false,
            ),
            (
                "numeric array on string field",
                "name IN [1, 2]",
                "String",
                false,
            ),
            (
                "bool array on numeric field",
                "age IN [true, false]",
                "u32",
                false,
            ),
            (
                "string array on string field",
                r#"name IN ["a", "b"]"#,
                "String",
                true,
            ),
            (
                "numeric array on numeric field",
                "age IN [1, 2]",
                "u32",
                true,
            ),
            (
                "string array on Option<String>",
                r#"name IN ["a"]"#,
                "Option<String>",
                true,
            ),
            (
                "string array on Other field",
                r#"tag IN ["a", "b"]"#,
                "MyId",
                true,
            ),
            (
                "numeric array on Other field",
                "tag IN [1, 2]",
                "MyId",
                true,
            ),
            ("empty array on numeric field", "age IN []", "u32", true),
            (
                "ALL OF string array on numeric",
                r#"age ALL OF ["a"]"#,
                "u32",
                false,
            ),
        ];

        for (name, query, field_type, expect_ok) in cases {
            let tokens = tokenize(query, None).unwrap();
            let fields = vec![
                FieldInfo::new("age", field_type),
                FieldInfo::new("name", field_type),
                FieldInfo::new("tag", field_type),
            ];
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser.parse();

            assert_eq!(
                result.is_ok(),
                expect_ok,
                "Failed: {} (query '{}' on {}), got: {:?}",
                name,
                query,
                field_type,
                result
            );
            if !expect_ok {
                assert!(
                    matches!(result, Err(DnfError::TypeMismatch { .. })),
                    "Expected TypeMismatch for '{}', got: {:?}",
                    name,
                    result
                );
            }
        }
    }

    // ==================== Nested Field Tests ====================

    #[test]
    fn test_parse_nested_fields() {
        let cases = vec![
            ParseTestCase {
                name: "simple nested field",
                query: "user.name.first == \"John\"",
                fields: vec![FieldInfo::new("user.name.first", "String")],
                expected_conjunctions: 1,
                expected_conditions: vec![1],
            },
            ParseTestCase {
                name: "nested fields with AND",
                query: "person.age > 18 AND person.contact.email CONTAINS \"@\"",
                fields: vec![
                    FieldInfo::new("person.age", "u32"),
                    FieldInfo::new("person.contact.email", "String"),
                ],
                expected_conjunctions: 1,
                expected_conditions: vec![2],
            },
            ParseTestCase {
                name: "nested field with array",
                query: r#"user.status IN ["active", "pending"]"#,
                fields: vec![FieldInfo::new("user.status", "String")],
                expected_conjunctions: 1,
                expected_conditions: vec![1],
            },
            ParseTestCase {
                name: "deeply nested field",
                query: "a.b.c.d.e == 42",
                fields: vec![FieldInfo::new("a.b.c.d.e", "u32")],
                expected_conjunctions: 1,
                expected_conditions: vec![1],
            },
            ParseTestCase {
                name: "complex nested query",
                query: r#"(user.age > 18 AND user.status IN ["active"]) OR user.role == "admin""#,
                fields: vec![
                    FieldInfo::new("user.age", "u32"),
                    FieldInfo::new("user.status", "String"),
                    FieldInfo::new("user.role", "String"),
                ],
                expected_conjunctions: 2,
                expected_conditions: vec![2, 1],
            },
        ];

        for case in cases {
            let tokens = tokenize(case.query, None).unwrap();
            let parser = Parser::new(tokens, &case.fields, case.query.to_string(), None);
            let query = parser
                .parse()
                .unwrap_or_else(|e| panic!("Failed to parse '{}': {:?}", case.name, e));

            assert_eq!(
                query.conjunctions().len(),
                case.expected_conjunctions,
                "Conjunction count mismatch for '{}'",
                case.name
            );

            for (i, &expected_count) in case.expected_conditions.iter().enumerate() {
                assert_eq!(
                    query.conjunctions()[i].conditions().len(),
                    expected_count,
                    "Condition count mismatch for '{}' conjunction {}",
                    case.name,
                    i
                );
            }
        }
    }

    // ==================== String Escape Round-Trip Tests ====================

    #[test]
    #[cfg(feature = "parser")]
    fn test_parse_query_with_escaped_strings() {
        let cases = vec![
            (r#"name == "He said \"Hello\"""#, "He said \"Hello\""),
            (r#"path == "C:\\Users\\Test""#, "C:\\Users\\Test"),
            (r#"url == "https:\/\/example.com""#, "https://example.com"),
            (r#"text == "Line1\nLine2\tTab""#, "Line1\nLine2\tTab"),
        ];

        for (query, expected_str) in cases {
            let tokens = tokenize(query, None).unwrap();
            let fields = vec![
                FieldInfo::new("name", "String"),
                FieldInfo::new("path", "String"),
                FieldInfo::new("url", "String"),
                FieldInfo::new("text", "String"),
            ];
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser.parse().unwrap();

            let value = result.conjunctions()[0].conditions()[0].value();
            if let Value::String(s) = value {
                assert_eq!(
                    s.as_ref(),
                    expected_str,
                    "Failed to parse escaped string in: {}",
                    query
                );
            } else {
                panic!("Expected String value for: {}", query);
            }
        }
    }

    #[test]
    #[cfg(feature = "parser")]
    fn test_query_display_roundtrip() {
        let test_cases = vec![
            r#"name == "simple""#,
            r#"path == "C:\\Users\\Test""#,
            r#"quote == "He said \"Hello\"""#,
            r#"url == "https:\/\/example.com""#,
            r#"text == "Line1\nLine2""#,
            r#"status IN ["active", "pending"]"#,
            r#"(age > 18 AND country == "US") OR premium == true"#,
        ];

        for query_str in test_cases {
            let fields = vec![
                FieldInfo::new("name", "String"),
                FieldInfo::new("path", "String"),
                FieldInfo::new("quote", "String"),
                FieldInfo::new("url", "String"),
                FieldInfo::new("text", "String"),
                FieldInfo::new("status", "String"),
                FieldInfo::new("age", "u32"),
                FieldInfo::new("country", "String"),
                FieldInfo::new("premium", "bool"),
            ];

            // Parse the query
            let tokens = tokenize(query_str, None)
                .unwrap_or_else(|e| panic!("Failed to tokenize '{}': {:?}", query_str, e));
            let parser = Parser::new(tokens, &fields, query_str.to_string(), None);
            let query1 = parser
                .parse()
                .unwrap_or_else(|e| panic!("Failed to parse '{}': {:?}", query_str, e));

            // Convert to string
            let query_str2 = query1.to_string();

            // Parse again
            let tokens2 = tokenize(&query_str2, None).unwrap_or_else(|e| {
                panic!(
                    "Failed to tokenize round-trip '{}' -> '{}': {:?}",
                    query_str, query_str2, e
                )
            });
            let parser2 = Parser::new(tokens2, &fields, query_str2.clone(), None);
            let query2 = parser2.parse().unwrap_or_else(|e| {
                panic!(
                    "Failed to parse round-trip '{}' -> '{}': {:?}",
                    query_str, query_str2, e
                )
            });

            // Compare the two queries by comparing their string representations
            assert_eq!(
                query1.to_string(),
                query2.to_string(),
                "Round-trip failed for: {}",
                query_str
            );
        }
    }

    // ==================== BETWEEN Operator Tests ====================

    #[test]
    fn test_parse_between_operator() {
        let fields = vec![
            FieldInfo::new("age", "u32"),
            FieldInfo::new("score", "f64"),
            FieldInfo::new("value", "i32"),
        ];

        let cases = vec![
            ("age BETWEEN [18, 65]", Op::BETWEEN.base),
            ("age NOT BETWEEN [0, 17]", Op::NOT_BETWEEN.base),
            ("score BETWEEN [60.0, 100.0]", Op::BETWEEN.base),
            ("value BETWEEN [-100, 100]", Op::BETWEEN.base),
        ];

        for (query, expected_op) in cases {
            let tokens = tokenize(query, None)
                .unwrap_or_else(|e| panic!("Failed to tokenize '{}': {:?}", query, e));
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser
                .parse()
                .unwrap_or_else(|e| panic!("Failed to parse '{}': {:?}", query, e));

            assert_eq!(
                result.conjunctions()[0].conditions()[0].operator().base,
                expected_op,
                "Operator mismatch for: {}",
                query
            );

            // Verify the value is an array
            let value = result.conjunctions()[0].conditions()[0].value();
            assert!(
                matches!(
                    value,
                    Value::IntArray(_) | Value::UintArray(_) | Value::FloatArray(_)
                ),
                "Expected array value for BETWEEN, got: {:?}",
                value
            );
        }
    }

    #[test]
    fn test_parse_between_in_complex_query() {
        let query = r#"(age BETWEEN [18, 65] AND country == "US") OR premium == true"#;
        let fields = vec![
            FieldInfo::new("age", "u32"),
            FieldInfo::new("country", "String"),
            FieldInfo::new("premium", "bool"),
        ];

        let tokens = tokenize(query, None).unwrap();
        let parser = Parser::new(tokens, &fields, query.to_string(), None);
        let result = parser.parse().unwrap();

        // Should have 2 conjunctions
        assert_eq!(result.conjunctions().len(), 2);

        // First conjunction should have 2 conditions (age BETWEEN and country ==)
        assert_eq!(result.conjunctions()[0].conditions().len(), 2);
        assert_eq!(
            result.conjunctions()[0].conditions()[0].operator().base,
            Op::BETWEEN.base
        );
    }

    // ==================== Negative BETWEEN Operator Tests ====================

    #[test]
    fn test_parse_between_errors() {
        // Only structural errors are caught at parse time (width/sign/kind
        // leniency is deferred to eval, see `test_parse_between_lenient`). Each
        // malformed shape is checked for both `BETWEEN` and `NOT BETWEEN`, which
        // share the same array path.
        let fields = vec![
            FieldInfo::new("age", "u32"),
            FieldInfo::new("score", "f64"),
            FieldInfo::new("value", "i32"),
        ];

        // (query, substring of the Display error message)
        let error_cases = vec![
            // Missing brackets around the bounds.
            ("age BETWEEN 18, 65", "Expected ["),
            ("age NOT BETWEEN 18, 65", "Expected ["),
            // Missing comma between the two bounds.
            ("age BETWEEN [18 65]", "Expected ] or ,"),
            ("age NOT BETWEEN [18 65]", "Expected ] or ,"),
            // Too few bounds (arity < 2).
            ("age BETWEEN [18]", "BETWEEN takes exactly 2 values"),
            ("age NOT BETWEEN [18]", "BETWEEN takes exactly 2 values"),
            // Too many bounds (arity > 2).
            (
                "age BETWEEN [18, 65, 100]",
                "BETWEEN takes exactly 2 values",
            ),
            (
                "age NOT BETWEEN [18, 65, 100]",
                "BETWEEN takes exactly 2 values",
            ),
            // Empty array (arity 0).
            ("age BETWEEN []", "BETWEEN takes exactly 2 values"),
            ("age NOT BETWEEN []", "BETWEEN takes exactly 2 values"),
        ];

        for (query, expected_error) in error_cases {
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser.parse();

            assert!(result.is_err(), "Expected error for: {}", query);
            let err_str = result.unwrap_err().to_string();
            assert!(
                err_str.contains(expected_error),
                "Expected '{}' error for '{}', got: {}",
                expected_error,
                query,
                err_str
            );
        }
    }

    #[test]
    fn test_parse_between_valid() {
        let fields = vec![
            FieldInfo::new("age", "u32"),
            FieldInfo::new("score", "f64"),
            FieldInfo::new("value", "i32"),
        ];

        // Exactly two numeric bounds parse; integer bounds widen to a float
        // field, and negative bounds are accepted on a signed field.
        let ok_cases = vec![
            "age BETWEEN [18, 65]",
            "age NOT BETWEEN [18, 65]",
            "score BETWEEN [18, 65]", // integer bounds widen to f64
            "score BETWEEN [1.5, 9.5]",
            "value BETWEEN [-5, 5]",
        ];

        for query in ok_cases {
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            assert!(
                parser.parse().is_ok(),
                "Expected '{}' to parse successfully",
                query
            );
        }
    }

    #[test]
    fn test_parse_between_lenient() {
        // BETWEEN shares the general array path, so its bounds follow the same
        // within-category leniency as any other array: width/sign/float-to-int
        // mismatches against a numeric field are deferred to eval rather than
        // rejected at parse time. These all parse (width/kind checks happen
        // later). Cross-category bounds (e.g. strings on a numeric field) are
        // rejected at parse time — see `test_parse_between_category_mismatch`.
        let fields = vec![FieldInfo::new("age", "u32"), FieldInfo::new("value", "i32")];

        let cases: Vec<(&str, ValueValidator)> = vec![
            // Float bounds on an integer field → FloatArray (deferred to eval).
            (
                "age BETWEEN [18.5, 65.3]",
                Box::new(|v| matches!(v, Value::FloatArray(a) if a.len() == 2)),
            ),
            (
                "value BETWEEN [18.5, 65.3]",
                Box::new(|v| matches!(v, Value::FloatArray(a) if a.len() == 2)),
            ),
            // Negative bound on an unsigned field → IntArray (deferred to eval).
            (
                "age BETWEEN [-5, 10]",
                Box::new(|v| matches!(v, Value::IntArray(a) if a.as_ref() == [-5, 10])),
            ),
        ];

        for (query, validator) in cases {
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let parsed = parser
                .parse()
                .unwrap_or_else(|e| panic!("Expected '{}' to parse: {:?}", query, e));
            let value = parsed.conjunctions()[0].conditions()[0].value();
            assert!(
                validator(value),
                "Unexpected BETWEEN bounds value for '{}': {:?}",
                query,
                value
            );
        }
    }

    #[test]
    fn test_parse_between_category_mismatch() {
        // Bounds whose category differs from a numeric field are rejected at
        // parse time, instead of parsing into a value that can never match.
        // Width/sign/float-to-int leniency within the numeric category stays
        // deferred to eval (covered by `test_parse_between_lenient`).
        let fields = vec![
            FieldInfo::new("age", "u32"),
            FieldInfo::new("value", "i32"),
            FieldInfo::new("score", "f64"),
        ];

        // (query, expected error substring); checked for BETWEEN and NOT BETWEEN.
        let cases = vec![
            (r#"age BETWEEN ["lo", "hi"]"#, "expected numeric"),
            (r#"age NOT BETWEEN ["lo", "hi"]"#, "expected numeric"),
            (r#"score BETWEEN ["lo", "hi"]"#, "expected numeric"),
            ("value BETWEEN [true, false]", "expected numeric"),
        ];

        for (query, expected_error) in cases {
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser.parse();

            assert!(result.is_err(), "Expected error for: {}", query);
            let err_str = result.unwrap_err().to_string();
            assert!(
                err_str.contains(expected_error),
                "Expected '{}' error for '{}', got: {}",
                expected_error,
                query,
                err_str
            );
        }
    }

    // ==================== Special Value Tests ====================

    #[test]
    fn test_parse_special_values() {
        let cases: Vec<ArrayTestCase> = vec![
            (
                "null value",
                "name == null",
                vec![FieldInfo::new("name", "Option < String >")],
                Box::new(|v: &Value| matches!(v, Value::None)),
            ),
            (
                "null with not equals",
                "name != null",
                vec![FieldInfo::new("name", "Option < String >")],
                Box::new(|v: &Value| matches!(v, Value::None)),
            ),
            (
                "empty string",
                r#"name == """#,
                vec![FieldInfo::new("name", "String")],
                Box::new(|v: &Value| matches!(v, Value::String(s) if s.as_ref() == "")),
            ),
        ];

        for (name, query, fields_vec, validator) in cases {
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields_vec, query.to_string(), None);
            let result = parser.parse().unwrap();

            let value = result.conjunctions()[0].conditions()[0].value();
            assert!(
                validator(value),
                "Special value validation failed for '{}': {:?}",
                name,
                value
            );
        }
    }

    // ==================== Map Target Tests ====================

    #[test]
    fn test_parse_map_targets() {
        // The three map-access spellings each parse to the matching map-wrapped
        // `Value`. The field must be declared as a map (`FieldKind::Map`).
        let cases: Vec<(&str, ValueValidator)> = vec![
            (
                r#"meta.@keys CONTAINS "author""#,
                Box::new(
                    |v| matches!(v, Value::Keys(inner) if matches!(inner.as_ref(), Value::String(s) if s.as_ref() == "author")),
                ),
            ),
            (
                r#"meta.@values IN ["a", "b"]"#,
                Box::new(
                    |v| matches!(v, Value::Values(inner) if matches!(inner.as_ref(), Value::StringArray(a) if a.len() == 2)),
                ),
            ),
            (
                r#"meta["author"] == "Alice""#,
                Box::new(
                    |v| matches!(v, Value::AtKey(k, inner) if k.as_ref() == "author" && matches!(inner.as_ref(), Value::String(s) if s.as_ref() == "Alice")),
                ),
            ),
        ];

        for (query, validator) in cases {
            let fields = vec![FieldInfo::with_kind("meta", "HashMap", FieldKind::Map)];
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let parsed = parser
                .parse()
                .unwrap_or_else(|e| panic!("'{}' should parse: {:?}", query, e));
            let value = parsed.conjunctions()[0].conditions()[0].value();
            assert!(
                validator(value),
                "Unexpected map-target value for '{}': {:?}",
                query,
                value
            );
        }
    }

    #[test]
    fn test_parse_map_target_errors() {
        // Map syntax on a scalar field, and malformed bracket keys, are rejected
        // at parse time. (query, is_map_field, expected error variant substring)
        let cases = vec![
            // Map access on a non-map (scalar) field.
            (r#"meta.@keys CONTAINS "x""#, false, "UnexpectedToken"),
            (r#"meta.@values CONTAINS "x""#, false, "UnexpectedToken"),
            (r#"meta["k"] == "x""#, false, "UnexpectedToken"),
            // Bracket key must be a string literal, not a number.
            (r#"meta[5] == "x""#, true, "UnexpectedToken"),
            // Missing closing bracket.
            (r#"meta["k" == "x""#, true, "UnexpectedToken"),
            // End of input right after the opening bracket.
            (r#"meta["#, true, "UnexpectedEof"),
        ];

        for (query, is_map, expected) in cases {
            let kind = if is_map {
                FieldKind::Map
            } else {
                FieldKind::Scalar
            };
            let fields = vec![FieldInfo::with_kind("meta", "HashMap", kind)];
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser.parse();

            assert!(result.is_err(), "Expected error for: {}", query);
            let err_str = format!("{:?}", result.unwrap_err());
            assert!(
                err_str.contains(expected),
                "Expected '{}' for '{}', got: {}",
                expected,
                query,
                err_str
            );
        }
    }

    // ==================== Custom Operator Tests ====================

    #[test]
    fn test_parse_custom_operators() {
        // A registered custom operator tokenizes to `CustomOp` and parses to an
        // `Op::custom`; one additionally listed as novalue takes no operand.
        let custom_ops = vec!["IS_SIMILAR".to_string(), "IS_VALID".to_string()];
        let novalue_ops = vec!["IS_VALID".to_string()];
        let fields = vec![FieldInfo::new("name", "String")];

        // (query, expected value variant check)
        let cases: Vec<(&str, ValueValidator)> = vec![
            // Custom op with a value operand.
            (
                r#"name IS_SIMILAR "bob""#,
                Box::new(|v| matches!(v, Value::String(s) if s.as_ref() == "bob")),
            ),
            // Novalue custom op: no operand, value defaults to `None`.
            ("name IS_VALID", Box::new(|v| matches!(v, Value::None))),
        ];

        for (query, validator) in cases {
            let tokens = tokenize(query, Some(&custom_ops)).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), Some(&novalue_ops));
            let parsed = parser
                .parse()
                .unwrap_or_else(|e| panic!("'{}' should parse: {:?}", query, e));
            let condition = &parsed.conjunctions()[0].conditions()[0];
            assert!(
                matches!(
                    condition.operator().base,
                    crate::operator::BaseOperator::Custom(_)
                ),
                "'{}' should yield a custom operator",
                query
            );
            assert!(
                validator(condition.value()),
                "Unexpected value for '{}': {:?}",
                query,
                condition.value()
            );
        }
    }

    #[test]
    fn test_parse_custom_operator_operand_cross_category() {
        // Custom operand semantics are user-defined, so the operand's category
        // is not checked against the field's type.
        let custom_ops = vec!["FUZZY".to_string()];
        let fields = vec![
            FieldInfo::new("name", "String"),
            FieldInfo::new("active", "bool"),
        ];

        for query in [
            "name FUZZY 5",
            "name NOT FUZZY 5",
            "active FUZZY 5",
            "active NOT FUZZY 5",
        ] {
            let tokens = tokenize(query, Some(&custom_ops)).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            parser
                .parse()
                .unwrap_or_else(|e| panic!("'{}' should parse: {:?}", query, e));
        }
    }

    #[test]
    fn test_parse_custom_operator_edge_cases() {
        // Covers the two text-syntax fixes for custom operators:
        //   (a) a custom op whose name collides with a reserved keyword, and
        //   (b) a `NOT`-prefixed custom op (negated form).
        let custom_ops = vec!["BETWEEN".to_string(), "IS_ADULT".to_string()];
        let novalue_ops = vec!["IS_ADULT".to_string()];
        let fields = vec![FieldInfo::new("age", "u32")];

        // (query, expected name, expected inverse flag, description)
        let cases = vec![
            (
                "age BETWEEN 5",
                "BETWEEN",
                false,
                "reserved-name custom op is honored over the built-in",
            ),
            (
                "age NOT IS_ADULT",
                "IS_ADULT",
                true,
                "NOT custom op parses to a negated custom op",
            ),
            (
                "age IS_ADULT",
                "IS_ADULT",
                false,
                "bare novalue custom op parses without an operand",
            ),
        ];

        for (query, name, inverse, desc) in cases {
            let tokens = tokenize(query, Some(&custom_ops)).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), Some(&novalue_ops));
            let parsed = parser
                .parse()
                .unwrap_or_else(|e| panic!("'{}' should parse ({}): {:?}", query, desc, e));
            let op = parsed.conjunctions()[0].conditions()[0].operator();
            assert_eq!(op.custom_name(), Some(name), "Failed: {}", desc);
            assert_eq!(op.inverse, inverse, "Failed inverse flag: {}", desc);
        }
    }

    // ==================== Structural Error Tests ====================

    #[test]
    fn test_parse_structural_errors() {
        // Grammar-shape errors across the parser surface. The field set stays
        // fixed; each query is malformed in exactly one way.
        // (query, expected error variant substring, description)
        let fields = vec![
            FieldInfo::new("age", "u32"),
            FieldInfo::new("country", "String"),
        ];
        let cases = vec![
            ("(age > 18", "UnexpectedToken", "unclosed parenthesis"),
            ("age > 18 country", "UnexpectedToken", "trailing tokens"),
            ("age > 18 AND", "UnexpectedEof", "dangling AND"),
            ("age > 18 OR", "UnexpectedEof", "dangling OR"),
            ("age >", "UnexpectedEof", "missing value"),
            ("age 18", "UnexpectedToken", "missing operator"),
            ("> 18", "UnexpectedToken", "no field identifier"),
            ("AND age > 18", "UnexpectedToken", "leading connective"),
        ];

        for (query, expected, desc) in cases {
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser.parse();

            assert!(result.is_err(), "Expected error for '{}' ({})", desc, query);
            let err_str = format!("{:?}", result.unwrap_err());
            assert!(
                err_str.contains(expected),
                "Expected '{}' for '{}' ({}), got: {}",
                expected,
                desc,
                query,
                err_str
            );
        }
    }

    #[test]
    fn test_parse_empty_input() {
        // Whitespace-only and empty strings lex to zero tokens and parse to
        // `EmptyQuery`, matching the hand-built empty-token case.
        let fields = vec![FieldInfo::new("age", "u32")];
        for query in ["", "   ", "\t\n"] {
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            assert!(
                matches!(parser.parse(), Err(DnfError::EmptyQuery)),
                "Expected EmptyQuery for {:?}",
                query
            );
        }
    }

    #[test]
    fn test_parse_array_structural_errors() {
        // Malformed array literals and incompatible element kinds are rejected.
        // (query, field_type, expected error variant substring, description)
        let cases = vec![
            ("age IN [18, 21", "u32", "UnexpectedToken", "unclosed array"),
            (
                r#"age IN [18, "a"]"#,
                "u32",
                "TypeMismatch",
                "mixed number and string elements",
            ),
            ("age IN [18,]", "u32", "UnexpectedToken", "trailing comma"),
        ];

        for (query, field_type, expected, desc) in cases {
            let fields = vec![FieldInfo::new("age", field_type)];
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser.parse();

            assert!(result.is_err(), "Expected error for '{}' ({})", desc, query);
            let err_str = format!("{:?}", result.unwrap_err());
            assert!(
                err_str.contains(expected),
                "Expected '{}' for '{}' ({}), got: {}",
                expected,
                desc,
                query,
                err_str
            );
        }
    }

    #[test]
    fn test_parse_array_type_mismatch_anchors_at_offending_element() {
        // A type mismatch inside an array literal is anchored at the offending
        // element and quoted SQL-style (`near `…``), not left pointing past the
        // closing `]` at end of input. (query, field_type, byte offset of the
        // bad element, the text the `near` snippet must quote, description)
        let cases = vec![
            (
                r#"tags IN ["a", 5]"#,
                "String",
                14, // the `5`
                "5",
                "number among string elements",
            ),
            (
                r#"flags IN [true, "x"]"#,
                "bool",
                16, // the `"x"`
                "x",
                "string among boolean elements",
            ),
        ];

        for (query, field_type, expected_pos, near_text, desc) in cases {
            let field = query.split_whitespace().next().unwrap();
            let fields = vec![FieldInfo::new(field, field_type)];
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let err = parser.parse().unwrap_err();

            let position = match &err {
                DnfError::TypeMismatch {
                    position: Some(p), ..
                } => *p,
                other => panic!("Expected TypeMismatch for '{}', got: {:?}", desc, other),
            };
            assert_eq!(
                position, expected_pos,
                "'{}' should anchor at the offending element, not EOF ({})",
                desc, query
            );
            assert!(
                position < query.len(),
                "'{}' position must point inside the query, not past `]`",
                desc
            );

            let msg = err.to_string();
            assert!(
                msg.contains("near `") && msg.contains(near_text),
                "'{}' should quote the offending element `{}`: {}",
                desc,
                near_text,
                msg
            );
        }
    }
}

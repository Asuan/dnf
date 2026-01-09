use std::collections::{HashMap, HashSet};

use super::token::Token;
use super::ParseError;
use crate::error::DnfError;
use crate::{Condition, Conjunction, DnfQuery, FieldInfo, FieldKind, Op, Value};

/// Map target for map field operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MapTarget {
    /// Access value at specific key: field["key"]
    AtKey,
    /// Match against keys: field.@keys
    Keys,
    /// Match against values: field.@values
    Values,
}

/// Recursive descent parser for DNF queries.
pub(crate) struct Parser<'a> {
    tokens: Vec<Token>,
    current: usize,
    fields: HashMap<&'a str, (&'a str, FieldKind)>, // field_name -> (field_type, field_kind)
    input: String,                                  // Original input for error messages
    novalue_ops: HashSet<String>,                   // Operators that don't need a value
}

impl<'a> Parser<'a> {
    /// Create a new parser with tokens and field information.
    pub(crate) fn new(
        tokens: Vec<Token>,
        fields: &'a [FieldInfo],
        input: String,
        novalue_ops: Option<&[String]>,
    ) -> Self {
        let field_map = fields
            .iter()
            .map(|f| (f.name, (f.field_type, f.kind)))
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

    fn invalid_number_error(&self, value: &Value) -> DnfError {
        DnfError::ParseError(ParseError::InvalidNumber {
            value: format!("{:?}", value),
            position: self.current,
            input: self.input.clone(),
        })
    }

    fn type_mismatch_error(
        &self,
        field: &str,
        expected: impl Into<Box<str>>,
        actual: impl Into<Box<str>>,
    ) -> DnfError {
        DnfError::TypeMismatch {
            field: field.into(),
            expected: expected.into(),
            actual: actual.into(),
            position: Some(self.current),
        }
    }

    /// Parse the tokens into a DnfQuery.
    pub(crate) fn parse(mut self) -> Result<DnfQuery, DnfError> {
        if self.tokens.is_empty() {
            return Err(DnfError::ParseError(ParseError::EmptyQuery));
        }

        let conjunctions = self.parse_or_expr()?;

        // Ensure we consumed all tokens
        if !self.is_at_end() {
            return Err(DnfError::ParseError(ParseError::UnexpectedToken {
                expected: "end of input".to_string(),
                found: self
                    .peek()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "EOF".to_string()),
                position: self.current,
                input: self.input.clone(),
            }));
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
            return Err(DnfError::ParseError(ParseError::UnexpectedToken {
                expected: ")".to_string(),
                found: self
                    .peek()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "EOF".to_string()),
                position: self.current,
                input: self.input.clone(),
            }));
        }

        Ok(Conjunction::from_conditions(conditions))
    }

    /// Parse a single condition: identifier operator value
    /// Supports map field syntax: field["key"], field.@keys, field.@values
    fn parse_condition(&mut self) -> Result<Condition, DnfError> {
        let field_name_str = match self.advance() {
            Some(Token::Identifier(name)) => name.clone(),
            Some(token) => {
                return Err(DnfError::ParseError(ParseError::UnexpectedToken {
                    expected: "field identifier".to_string(),
                    found: token.to_string(),
                    position: self.current - 1,
                    input: self.input.clone(),
                }));
            }
            None => return Err(DnfError::ParseError(ParseError::UnexpectedEof)),
        };

        let (field_type, field_kind) =
            *self
                .fields
                .get(field_name_str.as_str())
                .ok_or_else(|| DnfError::UnknownField {
                    field_name: field_name_str.clone().into_boxed_str(),
                    position: self.current - 1,
                })?;

        // Check for map target syntax: .@keys, .@values, or ["key"]
        let map_target = self.parse_map_target(field_kind)?;

        // For AtKey with bracket notation, we already parsed the key
        let at_key_value = if map_target == Some(MapTarget::AtKey) {
            Some(self.parse_bracket_key()?)
        } else {
            None
        };

        let operator = self.parse_operator()?;

        // Check if this is a novalue operator
        let is_novalue = if let crate::operator::BaseOperator::Custom(name) = &operator.base {
            self.novalue_ops.contains(name.as_ref())
        } else {
            false
        };

        // Parse the comparison value (skip for novalue operators)
        let raw_value = if is_novalue {
            Value::None
        } else {
            // For string operators (CONTAINS, STARTS_WITH, ENDS_WITH),
            // equality operators (Eq), and array operators (AnyOf, AllOf),
            // we allow flexible parsing since values can be compared as strings
            // or arrays can be parsed from literals
            let is_flexible_operator = matches!(
                &operator.base,
                crate::operator::BaseOperator::Contains
                    | crate::operator::BaseOperator::StartsWith
                    | crate::operator::BaseOperator::EndsWith
                    | crate::operator::BaseOperator::Eq
                    | crate::operator::BaseOperator::AnyOf
                    | crate::operator::BaseOperator::AllOf
            );

            // BETWEEN needs typed parsing for proper array type inference
            let is_between_operator =
                matches!(&operator.base, crate::operator::BaseOperator::Between);

            if map_target.is_some() {
                // Map targets always use flexible parsing
                self.parse_map_value(map_target)?
            } else if is_flexible_operator {
                self.parse_value_flexible()?
            } else if is_between_operator {
                // BETWEEN expects an array [min, max] - parse as typed array
                self.parse_value_array(field_type)?
            } else {
                self.parse_value(field_type)?
            }
        };

        // Wrap value in map target if present
        let value = match map_target {
            Some(MapTarget::AtKey) => {
                let key = at_key_value.expect("AtKey should have key");
                Value::AtKey(Box::from(key.as_str()), Box::new(raw_value))
            }
            Some(MapTarget::Keys) => Value::Keys(Box::new(raw_value)),
            Some(MapTarget::Values) => Value::Values(Box::new(raw_value)),
            None => raw_value,
        };

        Ok(Condition::new(field_name_str, operator, value))
    }

    /// Parse map target syntax: .@keys, .@values, or detect ["key"] bracket access
    fn parse_map_target(&mut self, field_kind: FieldKind) -> Result<Option<MapTarget>, DnfError> {
        match self.peek() {
            Some(Token::MapKeys) => {
                if field_kind != FieldKind::Map {
                    return Err(DnfError::ParseError(ParseError::UnexpectedToken {
                        expected: "map field for .@keys".to_string(),
                        found: format!("{:?} field", field_kind),
                        position: self.current,
                        input: self.input.clone(),
                    }));
                }
                self.advance();
                Ok(Some(MapTarget::Keys))
            }
            Some(Token::MapValues) => {
                if field_kind != FieldKind::Map {
                    return Err(DnfError::ParseError(ParseError::UnexpectedToken {
                        expected: "map field for .@values".to_string(),
                        found: format!("{:?} field", field_kind),
                        position: self.current,
                        input: self.input.clone(),
                    }));
                }
                self.advance();
                Ok(Some(MapTarget::Values))
            }
            Some(Token::LeftBracket) => {
                if field_kind != FieldKind::Map {
                    return Err(DnfError::ParseError(ParseError::UnexpectedToken {
                        expected: "map field for bracket access".to_string(),
                        found: format!("{:?} field", field_kind),
                        position: self.current,
                        input: self.input.clone(),
                    }));
                }
                self.advance(); // consume '['
                Ok(Some(MapTarget::AtKey))
            }
            _ => Ok(None),
        }
    }

    /// Parse the key from bracket notation: ["key"]
    fn parse_bracket_key(&mut self) -> Result<String, DnfError> {
        let key = match self.advance() {
            Some(Token::String(s)) => s.clone(),
            Some(token) => {
                return Err(DnfError::ParseError(ParseError::UnexpectedToken {
                    expected: "string key".to_string(),
                    found: token.to_string(),
                    position: self.current - 1,
                    input: self.input.clone(),
                }));
            }
            None => return Err(DnfError::ParseError(ParseError::UnexpectedEof)),
        };

        // Expect closing bracket
        if !self.match_token(&Token::RightBracket) {
            return Err(DnfError::ParseError(ParseError::UnexpectedToken {
                expected: "]".to_string(),
                found: self
                    .peek()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "EOF".to_string()),
                position: self.current,
                input: self.input.clone(),
            }));
        }

        Ok(key)
    }

    /// Parse a value for map operations.
    fn parse_map_value(&mut self, _map_target: Option<MapTarget>) -> Result<Value, DnfError> {
        // Map targets use flexible parsing
        self.parse_value_flexible()
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
            Some(Token::CustomOp(name)) => Ok(Op::custom(name.clone())),
            Some(token) => Err(DnfError::ParseError(ParseError::UnexpectedToken {
                expected: "operator".to_string(),
                found: token.to_string(),
                position: self.current - 1,
                input: self.input.clone(),
            })),
            None => Err(DnfError::ParseError(ParseError::UnexpectedEof)),
        }
    }

    /// Parse a value based on the expected field type.
    fn parse_value(&mut self, field_type: &str) -> Result<Value, DnfError> {
        let position = self.current;
        let token = self
            .advance()
            .ok_or(DnfError::ParseError(ParseError::UnexpectedEof))?
            .clone();

        match token {
            Token::String(s) => {
                // String values
                if self.is_string_type(field_type) {
                    Ok(Value::String(Box::from(s.as_str())))
                } else {
                    Err(DnfError::TypeMismatch {
                        field: field_type.into(),
                        expected: field_type.into(),
                        actual: "String".into(),
                        position: Some(position),
                    })
                }
            }
            Token::Number(num) => self.parse_number(&num, field_type, position),
            Token::Boolean(b) => {
                if self.is_bool_type(field_type) {
                    Ok(Value::Bool(b))
                } else {
                    Err(DnfError::TypeMismatch {
                        field: field_type.into(),
                        expected: field_type.into(),
                        actual: "Boolean".into(),
                        position: Some(position),
                    })
                }
            }
            Token::Null => {
                // Null is allowed for Option types
                if self.unwrap_option(field_type).is_some() {
                    Ok(Value::None)
                } else {
                    Err(DnfError::TypeMismatch {
                        field: field_type.into(),
                        expected: field_type.into(),
                        actual: "null".into(),
                        position: Some(position),
                    })
                }
            }
            token => Err(DnfError::ParseError(ParseError::UnexpectedToken {
                expected: "value".to_string(),
                found: token.to_string(),
                position,
                input: self.input.clone(),
            })),
        }
    }

    /// Parse a value without field type constraints (for string operators).
    /// Accepts strings, numbers, booleans, and arrays, inferring types from the token.
    fn parse_value_flexible(&mut self) -> Result<Value, DnfError> {
        let position = self.current;

        // Check for array literal
        if self.peek() == Some(&Token::LeftBracket) {
            return self.parse_array();
        }

        let token = self
            .advance()
            .ok_or(DnfError::ParseError(ParseError::UnexpectedEof))?
            .clone();

        match token {
            Token::String(s) => Ok(Value::String(Box::from(s.as_str()))),
            Token::Number(num) => {
                // Try to parse as different numeric types
                if num.contains('.') || num.contains('e') || num.contains('E') {
                    // Float
                    num.parse::<f64>()
                        .map(Value::Float)
                        .map_err(|_| DnfError::TypeMismatch {
                            field: "number".into(),
                            expected: "valid number".into(),
                            actual: num.clone().into_boxed_str(),
                            position: Some(position),
                        })
                } else if num.starts_with('-') {
                    // Signed integer
                    num.parse::<i64>()
                        .map(Value::Int)
                        .map_err(|_| DnfError::TypeMismatch {
                            field: "number".into(),
                            expected: "valid integer".into(),
                            actual: num.clone().into_boxed_str(),
                            position: Some(position),
                        })
                } else {
                    // Unsigned integer
                    num.parse::<u64>()
                        .map(Value::Uint)
                        .map_err(|_| DnfError::TypeMismatch {
                            field: "number".into(),
                            expected: "valid integer".into(),
                            actual: num.clone().into_boxed_str(),
                            position: Some(position),
                        })
                }
            }
            Token::Boolean(b) => Ok(Value::Bool(b)),
            Token::Null => Ok(Value::None),
            token => Err(DnfError::ParseError(ParseError::UnexpectedToken {
                expected: "value or array".to_string(),
                found: token.to_string(),
                position,
                input: self.input.clone(),
            })),
        }
    }

    /// Parse an array literal: [value, value, ...]
    /// All elements must be of the same type.
    /// Parse an array with type information from the field.
    /// Used for BETWEEN operator to ensure correct array type.
    fn parse_value_array(&mut self, field_type: &str) -> Result<Value, DnfError> {
        let base_type = self.unwrap_option(field_type).unwrap_or(field_type);

        // Consume '['
        if !self.match_token(&Token::LeftBracket) {
            return Err(DnfError::ParseError(ParseError::UnexpectedToken {
                expected: "[".to_string(),
                found: self
                    .peek()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "EOF".to_string()),
                position: self.current,
                input: self.input.clone(),
            }));
        }

        // Determine target array type from field type
        let is_signed = matches!(base_type, "i8" | "i16" | "i32" | "i64" | "isize");
        let is_unsigned = matches!(base_type, "u8" | "u16" | "u32" | "u64" | "usize");
        let is_float = matches!(base_type, "f32" | "f64");

        let mut values = Vec::new();

        // Parse first element
        let token = self
            .advance()
            .ok_or(DnfError::ParseError(ParseError::UnexpectedEof))?
            .clone();
        values.push(self.parse_number_token(&token, is_float, is_signed, is_unsigned)?);

        // Parse remaining elements
        while self.match_token(&Token::Comma) {
            let token = self
                .advance()
                .ok_or(DnfError::ParseError(ParseError::UnexpectedEof))?
                .clone();
            values.push(self.parse_number_token(&token, is_float, is_signed, is_unsigned)?);
        }

        // Consume ']'
        if !self.match_token(&Token::RightBracket) {
            return Err(DnfError::ParseError(ParseError::UnexpectedToken {
                expected: "]".to_string(),
                found: self
                    .peek()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "EOF".to_string()),
                position: self.current,
                input: self.input.clone(),
            }));
        }

        // Convert to appropriate Value type
        if is_float {
            let float_values: Result<Vec<f64>, DnfError> = values
                .into_iter()
                .map(|v| match v {
                    Value::Float(f) => Ok(f),
                    Value::Int(i) => Ok(i as f64),
                    Value::Uint(u) => Ok(u as f64),
                    _ => Err(self.invalid_number_error(&v)),
                })
                .collect();
            Ok(Value::FloatArray(float_values?.into_boxed_slice()))
        } else if is_signed {
            // Check if all values are already Int - avoid conversion
            let all_int = values.iter().all(|v| matches!(v, Value::Int(_)));
            if all_int {
                let int_values: Vec<i64> = values
                    .into_iter()
                    .map(|v| match v {
                        Value::Int(i) => i,
                        _ => unreachable!("already checked all are Int"),
                    })
                    .collect();
                Ok(Value::IntArray(int_values.into_boxed_slice()))
            } else {
                // Need to convert Uint to Int
                let int_values: Result<Vec<i64>, DnfError> = values
                    .into_iter()
                    .map(|v| match v {
                        Value::Int(i) => Ok(i),
                        Value::Uint(u) if u <= i64::MAX as u64 => Ok(u as i64),
                        Value::Uint(u) => Err(self.type_mismatch_error(
                            base_type,
                            format!("signed integer (max {})", i64::MAX),
                            format!("unsigned integer {}", u),
                        )),
                        _ => Err(self.invalid_number_error(&v)),
                    })
                    .collect();
                Ok(Value::IntArray(int_values?.into_boxed_slice()))
            }
        } else if is_unsigned {
            // Check if all values are already Uint - avoid conversion
            let all_uint = values.iter().all(|v| matches!(v, Value::Uint(_)));
            if all_uint {
                let uint_values: Vec<u64> = values
                    .into_iter()
                    .map(|v| match v {
                        Value::Uint(u) => u,
                        _ => unreachable!("already checked all are Uint"),
                    })
                    .collect();
                Ok(Value::UintArray(uint_values.into_boxed_slice()))
            } else {
                // Need to convert Int to Uint (must be non-negative)
                let uint_values: Result<Vec<u64>, DnfError> = values
                    .into_iter()
                    .map(|v| match v {
                        Value::Uint(u) => Ok(u),
                        Value::Int(i) if i >= 0 => Ok(i as u64),
                        Value::Int(i) => Err(self.type_mismatch_error(
                            base_type,
                            "unsigned integer",
                            format!("negative integer {}", i),
                        )),
                        _ => Err(self.invalid_number_error(&v)),
                    })
                    .collect();
                Ok(Value::UintArray(uint_values?.into_boxed_slice()))
            }
        } else {
            // Default to Int if type unknown
            // Check if all values are already Int - avoid conversion
            let all_int = values.iter().all(|v| matches!(v, Value::Int(_)));
            if all_int {
                let int_values: Vec<i64> = values
                    .into_iter()
                    .map(|v| match v {
                        Value::Int(i) => i,
                        _ => unreachable!("already checked all are Int"),
                    })
                    .collect();
                Ok(Value::IntArray(int_values.into_boxed_slice()))
            } else {
                let int_values: Result<Vec<i64>, DnfError> = values
                    .into_iter()
                    .map(|v| match v {
                        Value::Int(i) => Ok(i),
                        Value::Uint(u) if u <= i64::MAX as u64 => Ok(u as i64),
                        Value::Uint(u) => Err(self.type_mismatch_error(
                            base_type,
                            format!("signed integer (max {})", i64::MAX),
                            format!("unsigned integer {}", u),
                        )),
                        _ => Err(self.invalid_number_error(&v)),
                    })
                    .collect();
                Ok(Value::IntArray(int_values?.into_boxed_slice()))
            }
        }
    }

    fn parse_number_token(
        &self,
        token: &Token,
        is_float: bool,
        is_signed: bool,
        _is_unsigned: bool,
    ) -> Result<Value, DnfError> {
        match token {
            Token::Number(num) => {
                if is_float || num.contains('.') || num.contains('e') || num.contains('E') {
                    num.parse::<f64>().map(Value::Float).map_err(|_| {
                        DnfError::ParseError(ParseError::InvalidNumber {
                            value: num.clone(),
                            position: self.current - 1,
                            input: self.input.clone(),
                        })
                    })
                } else if is_signed || num.starts_with('-') {
                    num.parse::<i64>().map(Value::Int).map_err(|_| {
                        DnfError::ParseError(ParseError::InvalidNumber {
                            value: num.clone(),
                            position: self.current - 1,
                            input: self.input.clone(),
                        })
                    })
                } else {
                    num.parse::<u64>().map(Value::Uint).map_err(|_| {
                        DnfError::ParseError(ParseError::InvalidNumber {
                            value: num.clone(),
                            position: self.current - 1,
                            input: self.input.clone(),
                        })
                    })
                }
            }
            _ => Err(DnfError::ParseError(ParseError::UnexpectedToken {
                expected: "number".to_string(),
                found: token.to_string(),
                position: self.current - 1,
                input: self.input.clone(),
            })),
        }
    }

    fn parse_array(&mut self) -> Result<Value, DnfError> {
        let start_position = self.current;

        // Consume '['
        if !self.match_token(&Token::LeftBracket) {
            return Err(DnfError::ParseError(ParseError::UnexpectedToken {
                expected: "[".to_string(),
                found: self
                    .peek()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "EOF".to_string()),
                position: self.current,
                input: self.input.clone(),
            }));
        }

        // Handle empty array
        if self.match_token(&Token::RightBracket) {
            // Default to empty string array
            return Ok(Value::StringArray(
                Vec::<Box<str>>::new().into_boxed_slice(),
            ));
        }

        // Parse first element to determine array type
        let first_element = self.parse_array_element()?;
        let mut elements = vec![first_element.clone()];

        // Parse remaining elements
        while self.match_token(&Token::Comma) {
            let element = self.parse_array_element()?;

            // Verify type consistency
            if std::mem::discriminant(&element) != std::mem::discriminant(&first_element) {
                return Err(DnfError::TypeMismatch {
                    field: "array".into(),
                    expected: format!("{:?}", std::mem::discriminant(&first_element))
                        .into_boxed_str(),
                    actual: format!("{:?}", std::mem::discriminant(&element)).into_boxed_str(),
                    position: Some(self.current - 1),
                });
            }

            elements.push(element);
        }

        // Consume ']'
        if !self.match_token(&Token::RightBracket) {
            return Err(DnfError::ParseError(ParseError::UnexpectedToken {
                expected: "] or ,".to_string(),
                found: self
                    .peek()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "EOF".to_string()),
                position: self.current,
                input: self.input.clone(),
            }));
        }

        // Convert to appropriate array type
        match &first_element {
            Value::String(_) => {
                let strings: Vec<Box<str>> = elements
                    .into_iter()
                    .filter_map(|v| {
                        if let Value::String(s) = v {
                            Some(s)
                        } else {
                            None
                        }
                    })
                    .collect();
                Ok(Value::StringArray(strings.into_boxed_slice()))
            }
            Value::Int(_) => {
                let ints: Vec<i64> = elements
                    .into_iter()
                    .filter_map(|v| if let Value::Int(i) = v { Some(i) } else { None })
                    .collect();
                Ok(Value::IntArray(ints.into_boxed_slice()))
            }
            Value::Uint(_) => {
                let uints: Vec<u64> = elements
                    .into_iter()
                    .filter_map(|v| {
                        if let Value::Uint(u) = v {
                            Some(u)
                        } else {
                            None
                        }
                    })
                    .collect();
                Ok(Value::UintArray(uints.into_boxed_slice()))
            }
            Value::Float(_) => {
                let floats: Vec<f64> = elements
                    .into_iter()
                    .filter_map(|v| {
                        if let Value::Float(f) = v {
                            Some(f)
                        } else {
                            None
                        }
                    })
                    .collect();
                Ok(Value::FloatArray(floats.into_boxed_slice()))
            }
            Value::Bool(_) => {
                let bools: Vec<bool> = elements
                    .into_iter()
                    .filter_map(|v| {
                        if let Value::Bool(b) = v {
                            Some(b)
                        } else {
                            None
                        }
                    })
                    .collect();
                Ok(Value::BoolArray(bools.into_boxed_slice()))
            }
            _ => Err(DnfError::TypeMismatch {
                field: "array".into(),
                expected: "string, number, or boolean elements".into(),
                actual: format!("{:?}", first_element).into_boxed_str(),
                position: Some(start_position),
            }),
        }
    }

    /// Parse a single array element (string, number, or boolean).
    fn parse_array_element(&mut self) -> Result<Value, DnfError> {
        let position = self.current;
        let token = self
            .advance()
            .ok_or(DnfError::ParseError(ParseError::UnexpectedEof))?
            .clone();

        match token {
            Token::String(s) => Ok(Value::String(Box::from(s.as_str()))),
            Token::Number(num) => {
                if num.contains('.') || num.contains('e') || num.contains('E') {
                    num.parse::<f64>().map(Value::Float).map_err(|_| {
                        DnfError::ParseError(ParseError::InvalidNumber {
                            value: num.clone(),
                            position,
                            input: self.input.clone(),
                        })
                    })
                } else if num.starts_with('-') {
                    num.parse::<i64>().map(Value::Int).map_err(|_| {
                        DnfError::ParseError(ParseError::InvalidNumber {
                            value: num.clone(),
                            position,
                            input: self.input.clone(),
                        })
                    })
                } else {
                    num.parse::<u64>().map(Value::Uint).map_err(|_| {
                        DnfError::ParseError(ParseError::InvalidNumber {
                            value: num.clone(),
                            position,
                            input: self.input.clone(),
                        })
                    })
                }
            }
            Token::Boolean(b) => Ok(Value::Bool(b)),
            token => Err(DnfError::ParseError(ParseError::UnexpectedToken {
                expected: "array element (string, number, or boolean)".to_string(),
                found: token.to_string(),
                position,
                input: self.input.clone(),
            })),
        }
    }

    /// Parse a number string into the appropriate Value type.
    /// Uses field metadata when available, falls back to value-based inference.
    fn parse_number(
        &self,
        num: &str,
        field_type: &str,
        position: usize,
    ) -> Result<Value, DnfError> {
        // Unwrap Option<T> to get T
        let base_type = self.unwrap_option(field_type).unwrap_or(field_type);

        // Check if field type indicates signed integer
        let is_signed = matches!(base_type, "i8" | "i16" | "i32" | "i64" | "isize");

        // Check if field type indicates unsigned integer
        let is_unsigned = matches!(base_type, "u8" | "u16" | "u32" | "u64" | "usize");

        // Check if field type indicates float
        let is_float = matches!(base_type, "f32" | "f64");

        // Parse based on field type if known, otherwise infer from value
        if is_float || num.contains('.') || num.contains('e') || num.contains('E') {
            num.parse::<f64>().map(Value::Float).map_err(|_| {
                DnfError::ParseError(ParseError::InvalidNumber {
                    value: num.to_string(),
                    position,
                    input: self.input.clone(),
                })
            })
        } else if is_unsigned {
            num.parse::<u64>().map(Value::Uint).map_err(|_| {
                DnfError::ParseError(ParseError::InvalidNumber {
                    value: num.to_string(),
                    position,
                    input: self.input.clone(),
                })
            })
        } else if is_signed || num.starts_with('-') {
            num.parse::<i64>().map(Value::Int).map_err(|_| {
                DnfError::ParseError(ParseError::InvalidNumber {
                    value: num.to_string(),
                    position,
                    input: self.input.clone(),
                })
            })
        } else {
            // No field type info - infer from value
            if let Ok(i) = num.parse::<i64>() {
                Ok(Value::Int(i))
            } else if let Ok(u) = num.parse::<u64>() {
                Ok(Value::Uint(u))
            } else {
                num.parse::<f64>().map(Value::Float).map_err(|_| {
                    DnfError::ParseError(ParseError::InvalidNumber {
                        value: num.to_string(),
                        position,
                        input: self.input.clone(),
                    })
                })
            }
        }
    }

    /// Check if a type is a string type.
    /// Simplified: checks for common string patterns without complex normalization.
    fn is_string_type(&self, field_type: &str) -> bool {
        let base_type = self.unwrap_option(field_type).unwrap_or(field_type);
        let trimmed = base_type.trim();

        // Check for common string type patterns
        trimmed == "String"
            || trimmed == "str"
            || trimmed == "&str"
            || trimmed.starts_with("&") && trimmed.contains("str") && !trimmed.contains("struct")
    }

    /// Check if a type is a boolean type.
    fn is_bool_type(&self, field_type: &str) -> bool {
        let base_type = self.unwrap_option(field_type).unwrap_or(field_type);
        base_type == "bool"
    }

    /// Unwrap `Option<T>` to get T.
    /// Simplified: handles common Option patterns.
    fn unwrap_option<'b>(&self, type_str: &'b str) -> Option<&'b str> {
        let trimmed = type_str.trim();

        // Handle "Option<T>" - try without spaces first
        if let Some(inner) = trimmed.strip_prefix("Option<") {
            if let Some(inner) = inner.strip_suffix('>') {
                return Some(inner.trim());
            }
        }

        // Handle "Option < T >" with spaces
        if trimmed.starts_with("Option") {
            // Find the angle brackets
            if let Some(start) = trimmed.find('<') {
                if let Some(end) = trimmed.rfind('>') {
                    if start < end {
                        return Some(trimmed[start + 1..end].trim());
                    }
                }
            }
        }

        None
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
        self.tokens.get(self.current)
    }

    /// Consume and return the current token.
    fn advance(&mut self) -> Option<&Token> {
        if !self.is_at_end() {
            self.current += 1;
            self.tokens.get(self.current - 1)
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
        let cases = vec![
            ("age > 18", "u32", Op::GT.base),
            ("name CONTAINS \"John\"", "String", Op::CONTAINS.base),
            ("premium == true", "bool", Op::EQ.base),
        ];

        for (query, field_type, expected_op) in cases {
            let tokens = tokenize(query, None).unwrap();
            let fields = vec![
                FieldInfo::new("age", field_type),
                FieldInfo::new("name", field_type),
                FieldInfo::new("premium", field_type),
            ];
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser.parse().unwrap();

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

    // ==================== Error Tests ====================

    #[test]
    fn test_parse_errors() {
        let cases = vec![
            (
                "unknown > 18",
                vec![FieldInfo::new("age", "u32")],
                "UnknownField",
            ),
            (
                "age > \"not a number\"",
                vec![FieldInfo::new("age", "u32")],
                "TypeMismatch",
            ),
        ];

        for (query, fields, expected_error) in cases {
            let tokens = tokenize(query, None).unwrap();
            let parser = Parser::new(tokens, &fields, query.to_string(), None);
            let result = parser.parse();

            assert!(result.is_err(), "Expected error for: {}", query);
            let err_str = format!("{:?}", result.unwrap_err());
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
    fn test_parse_empty() {
        let tokens = vec![];
        let fields = vec![FieldInfo::new("age", "u32")];
        let parser = Parser::new(tokens, &fields, String::new(), None);
        let result = parser.parse();

        assert!(matches!(
            result,
            Err(DnfError::ParseError(ParseError::EmptyQuery))
        ));
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
    fn test_parse_not_in_operator() {
        let query_str = r#"status NOT IN ["deleted", "blocked"]"#;
        let tokens = tokenize(query_str, None).unwrap();
        let fields = vec![FieldInfo::new("status", "String")];
        let parser = Parser::new(tokens, &fields, query_str.to_string(), None);
        let query = parser.parse().unwrap();

        assert_eq!(
            query.conjunctions()[0].conditions()[0].operator().base,
            Op::NOT_ANY_OF.base
        );
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
}

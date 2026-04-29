use crate::error::DnfError;

/// Tokens for the query language.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    // Identifiers and keywords
    Identifier(Box<str>),
    And,
    Or,

    // Operators
    Eq,                 // == or =
    Ne,                 // !=
    Gt,                 // >
    Lt,                 // <
    Gte,                // >=
    Lte,                // <=
    Contains,           // CONTAINS
    NotContains,        // NOT CONTAINS
    StartsWith,         // STARTS WITH
    EndsWith,           // ENDS WITH
    NotStartsWith,      // NOT STARTS WITH
    NotEndsWith,        // NOT ENDS WITH
    AllOf,              // ALL OF
    AnyOf,              // IN (value in array)
    NotAllOf,           // NOT ALL OF
    NotAnyOf,           // NOT IN (value not in array)
    Between,            // BETWEEN [min, max]
    NotBetween,         // NOT BETWEEN [min, max]
    CustomOp(Box<str>), // Custom operator (e.g., IS_ADULT)

    // Values
    String(Box<str>),
    Number(Box<str>),
    Boolean(bool),
    Null,

    // Delimiters
    LeftParen,
    RightParen,
    LeftBracket,  // [
    RightBracket, // ]
    Comma,        // ,

    // Map target tokens
    MapKeys,   // .@keys
    MapValues, // .@values

    // Internal sentinel; not produced by the tokenizer.
    Consumed,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Identifier(s) => write!(f, "identifier '{}'", s),
            Token::And => write!(f, "AND"),
            Token::Or => write!(f, "OR"),
            Token::Eq => write!(f, "=="),
            Token::Ne => write!(f, "!="),
            Token::Gt => write!(f, ">"),
            Token::Lt => write!(f, "<"),
            Token::Gte => write!(f, ">="),
            Token::Lte => write!(f, "<="),
            Token::Contains => write!(f, "CONTAINS"),
            Token::NotContains => write!(f, "NOT CONTAINS"),
            Token::StartsWith => write!(f, "STARTS WITH"),
            Token::EndsWith => write!(f, "ENDS WITH"),
            Token::NotStartsWith => write!(f, "NOT STARTS WITH"),
            Token::NotEndsWith => write!(f, "NOT ENDS WITH"),
            Token::AllOf => write!(f, "ALL OF"),
            Token::AnyOf => write!(f, "IN"),
            Token::NotAllOf => write!(f, "NOT ALL OF"),
            Token::NotAnyOf => write!(f, "NOT IN"),
            Token::Between => write!(f, "BETWEEN"),
            Token::NotBetween => write!(f, "NOT BETWEEN"),
            Token::CustomOp(name) => write!(f, "{}", name),
            Token::String(s) => write!(f, "string '{}'", s),
            Token::Number(n) => write!(f, "number '{}'", n),
            Token::Boolean(b) => write!(f, "boolean {}", b),
            Token::Null => write!(f, "null"),
            Token::LeftParen => write!(f, "("),
            Token::RightParen => write!(f, ")"),
            Token::LeftBracket => write!(f, "["),
            Token::RightBracket => write!(f, "]"),
            Token::Comma => write!(f, ","),
            Token::MapKeys => write!(f, ".@keys"),
            Token::MapValues => write!(f, ".@values"),
            Token::Consumed => write!(f, "<consumed>"),
        }
    }
}

/// Tokenize a query string into a vector of tokens.
///
/// # Arguments
///
/// * `input` - The query string to tokenize
/// * `custom_op_names` - Optional slice of custom operator names to recognize
pub(crate) fn tokenize(
    input: &str,
    custom_op_names: Option<&[String]>,
) -> Result<Vec<Token>, DnfError> {
    let input_string = input.to_string();
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some((pos, ch)) = chars.next() {
        match ch {
            // Skip whitespace
            ' ' | '\t' | '\n' | '\r' => continue,

            // Delimiters
            '(' => tokens.push(Token::LeftParen),
            ')' => tokens.push(Token::RightParen),
            '[' => tokens.push(Token::LeftBracket),
            ']' => tokens.push(Token::RightBracket),
            ',' => tokens.push(Token::Comma),

            // Operators
            '=' => {
                if chars.peek().map(|(_, c)| *c) == Some('=') {
                    chars.next();
                    tokens.push(Token::Eq);
                } else {
                    tokens.push(Token::Eq);
                }
            }
            '!' => {
                if chars.peek().map(|(_, c)| *c) == Some('=') {
                    chars.next();
                    tokens.push(Token::Ne);
                } else {
                    return Err(DnfError::UnexpectedToken {
                        expected: "!=".to_string(),
                        found: "!".to_string(),
                        position: pos,
                        input: input_string.clone(),
                    });
                }
            }
            '>' => {
                if chars.peek().map(|(_, c)| *c) == Some('=') {
                    chars.next();
                    tokens.push(Token::Gte);
                } else {
                    tokens.push(Token::Gt);
                }
            }
            '<' => {
                if chars.peek().map(|(_, c)| *c) == Some('=') {
                    chars.next();
                    tokens.push(Token::Lte);
                } else {
                    tokens.push(Token::Lt);
                }
            }

            // Map target syntax: .@keys, .@values
            '.' => {
                if chars.peek().map(|(_, c)| *c) == Some('@') {
                    chars.next(); // consume '@'

                    // Read the target name
                    let mut target = String::new();
                    while let Some(&(_, ch)) = chars.peek() {
                        if ch.is_alphanumeric() || ch == '_' {
                            target.push(ch);
                            chars.next();
                        } else {
                            break;
                        }
                    }

                    match target.as_str() {
                        "keys" => tokens.push(Token::MapKeys),
                        "values" => tokens.push(Token::MapValues),
                        _ => {
                            return Err(DnfError::UnexpectedToken {
                                expected: "@keys or @values".to_string(),
                                found: format!("@{}", target),
                                position: pos,
                                input: input_string.clone(),
                            });
                        }
                    }
                } else {
                    return Err(DnfError::UnexpectedToken {
                        expected: "identifier or @".to_string(),
                        found: ".".to_string(),
                        position: pos,
                        input: input_string.clone(),
                    });
                }
            }

            // String literals
            '"' | '\'' => {
                let quote = ch;
                let mut string = String::new();
                let mut escaped = false;
                let mut found_closing_quote = false;

                for (escape_pos, ch) in chars.by_ref() {
                    if escaped {
                        match ch {
                            'n' => string.push('\n'),
                            't' => string.push('\t'),
                            'r' => string.push('\r'),
                            '\\' => string.push('\\'),
                            '"' => string.push('"'),
                            '\'' => string.push('\''),
                            '/' => string.push('/'),
                            _ => {
                                return Err(DnfError::InvalidEscape {
                                    escape: format!("\\{}", ch),
                                    position: escape_pos,
                                    input: input_string.clone(),
                                });
                            }
                        }
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == quote {
                        tokens.push(Token::String(string.into_boxed_str()));
                        found_closing_quote = true;
                        break;
                    } else {
                        string.push(ch);
                    }
                }

                // Check if we found the closing quote
                if !found_closing_quote {
                    return Err(DnfError::UnterminatedString {
                        position: pos,
                        input: input_string.clone(),
                    });
                }
            }

            // Numbers
            '0'..='9' | '+' | '-' => {
                let mut number = String::new();
                number.push(ch);

                // Only allow + or - at the start
                let is_sign = ch == '+' || ch == '-';
                if is_sign {
                    // Must be followed by a digit
                    if !chars
                        .peek()
                        .map(|(_, c)| c.is_ascii_digit())
                        .unwrap_or(false)
                    {
                        return Err(DnfError::InvalidNumber {
                            value: number,
                            position: pos,
                            input: input_string.clone(),
                        });
                    }
                }

                while let Some(&(_, ch)) = chars.peek() {
                    if ch.is_ascii_digit() || ch == '.' {
                        number.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }

                tokens.push(Token::Number(number.into_boxed_str()));
            }

            // Identifiers and keywords (supports nested fields like user.name.first)
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut ident = String::new();
                ident.push(ch);

                while let Some(&(_, ch)) = chars.peek() {
                    // Check for map target syntax: .@keys, .@values
                    if ch == '.' {
                        // Peek ahead to check for @
                        let mut peek_iter = chars.clone();
                        peek_iter.next(); // consume the '.'
                        if let Some(&(_, '@')) = peek_iter.peek() {
                            // This is a map target, stop identifier here
                            break;
                        }
                        // Regular nested field, continue
                        ident.push(ch);
                        chars.next();
                    } else if ch.is_alphanumeric() || ch == '_' {
                        ident.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }

                // Check for keywords (case-sensitive)
                // Operators/keywords: UPPERCASE (AND, OR, CONTAINS, etc.)
                // Constants: lowercase (true, false, null)
                match ident.as_str() {
                    "AND" => tokens.push(Token::And),
                    "OR" => tokens.push(Token::Or),
                    "true" => tokens.push(Token::Boolean(true)),
                    "false" => tokens.push(Token::Boolean(false)),
                    "null" => tokens.push(Token::Null),
                    "CONTAINS" => tokens.push(Token::Contains),
                    "IN" => tokens.push(Token::AnyOf), // IN is alias for ANY OF
                    "BETWEEN" => tokens.push(Token::Between),
                    "NOT" => {
                        // Check for "NOT CONTAINS", "NOT STARTS WITH", "NOT ENDS WITH", "NOT ALL OF", "NOT ANY OF"
                        // Skip whitespace
                        while let Some(&(_, ch)) = chars.peek() {
                            if ch.is_whitespace() {
                                chars.next();
                            } else {
                                break;
                            }
                        }

                        // Track position where next word should start
                        let next_word_pos = chars.peek().map(|(p, _)| *p).unwrap_or(pos);

                        // Read next word
                        let mut next_word = String::new();
                        while let Some(&(_, ch)) = chars.peek() {
                            if ch.is_alphanumeric() || ch == '_' {
                                next_word.push(ch);
                                chars.next();
                            } else {
                                break;
                            }
                        }

                        if next_word.is_empty() {
                            return Err(DnfError::UnexpectedToken {
                                expected:
                                    "CONTAINS, IN, BETWEEN, STARTS, ENDS, ALL, or ANY (after NOT)"
                                        .to_string(),
                                found: "end of expression".to_string(),
                                position: next_word_pos,
                                input: input_string.clone(),
                            });
                        }

                        match next_word.as_str() {
                            "CONTAINS" => tokens.push(Token::NotContains),
                            "IN" => tokens.push(Token::NotAnyOf), // NOT IN is alias for NOT ANY OF
                            "BETWEEN" => tokens.push(Token::NotBetween),
                            "STARTS" => {
                                // Read "WITH"
                                while let Some(&(_, ch)) = chars.peek() {
                                    if ch.is_whitespace() {
                                        chars.next();
                                    } else {
                                        break;
                                    }
                                }
                                let with_pos = chars.peek().map(|(p, _)| *p).unwrap_or(pos);
                                let mut with_word = String::new();
                                while let Some(&(_, ch)) = chars.peek() {
                                    if ch.is_alphanumeric() || ch == '_' {
                                        with_word.push(ch);
                                        chars.next();
                                    } else {
                                        break;
                                    }
                                }
                                if with_word == "WITH" {
                                    tokens.push(Token::NotStartsWith);
                                } else {
                                    return Err(DnfError::UnexpectedToken {
                                        expected: "WITH (after NOT STARTS)".to_string(),
                                        found: with_word,
                                        position: with_pos,
                                        input: input_string.clone(),
                                    });
                                }
                            }
                            "ENDS" => {
                                // Read "WITH"
                                while let Some(&(_, ch)) = chars.peek() {
                                    if ch.is_whitespace() {
                                        chars.next();
                                    } else {
                                        break;
                                    }
                                }
                                let with_pos = chars.peek().map(|(p, _)| *p).unwrap_or(pos);
                                let mut with_word = String::new();
                                while let Some(&(_, ch)) = chars.peek() {
                                    if ch.is_alphanumeric() || ch == '_' {
                                        with_word.push(ch);
                                        chars.next();
                                    } else {
                                        break;
                                    }
                                }
                                if with_word == "WITH" {
                                    tokens.push(Token::NotEndsWith);
                                } else {
                                    return Err(DnfError::UnexpectedToken {
                                        expected: "WITH (after NOT ENDS)".to_string(),
                                        found: with_word,
                                        position: with_pos,
                                        input: input_string.clone(),
                                    });
                                }
                            }
                            "ALL" => {
                                // Read "OF"
                                while let Some(&(_, ch)) = chars.peek() {
                                    if ch.is_whitespace() {
                                        chars.next();
                                    } else {
                                        break;
                                    }
                                }
                                let of_pos = chars.peek().map(|(p, _)| *p).unwrap_or(pos);
                                let mut of_word = String::new();
                                while let Some(&(_, ch)) = chars.peek() {
                                    if ch.is_alphanumeric() || ch == '_' {
                                        of_word.push(ch);
                                        chars.next();
                                    } else {
                                        break;
                                    }
                                }
                                if of_word == "OF" {
                                    tokens.push(Token::NotAllOf);
                                } else {
                                    return Err(DnfError::UnexpectedToken {
                                        expected: "OF (after NOT ALL)".to_string(),
                                        found: of_word,
                                        position: of_pos,
                                        input: input_string.clone(),
                                    });
                                }
                            }
                            // "NOT ANY" is not supported - use "NOT IN" instead
                            _ => {
                                return Err(DnfError::UnexpectedToken {
                                    expected:
                                        "CONTAINS, IN, BETWEEN, STARTS, ENDS, or ALL (after NOT)"
                                            .to_string(),
                                    found: next_word,
                                    position: next_word_pos,
                                    input: input_string.clone(),
                                });
                            }
                        }
                    }
                    "STARTS" => {
                        // Check for "STARTS WITH"
                        // Skip whitespace
                        while let Some(&(_, ch)) = chars.peek() {
                            if ch.is_whitespace() {
                                chars.next();
                            } else {
                                break;
                            }
                        }

                        // Track position where next word should start
                        let next_word_pos = chars.peek().map(|(p, _)| *p).unwrap_or(pos);

                        // Read next word
                        let mut next_word = String::new();
                        while let Some(&(_, ch)) = chars.peek() {
                            if ch.is_alphanumeric() || ch == '_' {
                                next_word.push(ch);
                                chars.next();
                            } else {
                                break;
                            }
                        }

                        if next_word.is_empty() {
                            return Err(DnfError::UnexpectedToken {
                                expected: "WITH (after STARTS)".to_string(),
                                found: "end of expression".to_string(),
                                position: next_word_pos,
                                input: input_string.clone(),
                            });
                        }

                        if next_word == "WITH" {
                            tokens.push(Token::StartsWith);
                        } else {
                            return Err(DnfError::UnexpectedToken {
                                expected: "WITH (after STARTS)".to_string(),
                                found: next_word,
                                position: next_word_pos,
                                input: input_string.clone(),
                            });
                        }
                    }
                    "ENDS" => {
                        // Check for "ENDS WITH"
                        // Skip whitespace
                        while let Some(&(_, ch)) = chars.peek() {
                            if ch.is_whitespace() {
                                chars.next();
                            } else {
                                break;
                            }
                        }

                        // Track position where next word should start
                        let next_word_pos = chars.peek().map(|(p, _)| *p).unwrap_or(pos);

                        // Read next word
                        let mut next_word = String::new();
                        while let Some(&(_, ch)) = chars.peek() {
                            if ch.is_alphanumeric() || ch == '_' {
                                next_word.push(ch);
                                chars.next();
                            } else {
                                break;
                            }
                        }

                        if next_word.is_empty() {
                            return Err(DnfError::UnexpectedToken {
                                expected: "WITH (after ENDS)".to_string(),
                                found: "end of expression".to_string(),
                                position: next_word_pos,
                                input: input_string.clone(),
                            });
                        }

                        if next_word == "WITH" {
                            tokens.push(Token::EndsWith);
                        } else {
                            return Err(DnfError::UnexpectedToken {
                                expected: "WITH (after ENDS)".to_string(),
                                found: next_word,
                                position: next_word_pos,
                                input: input_string.clone(),
                            });
                        }
                    }
                    "ALL" => {
                        // Check for "ALL OF"
                        // Skip whitespace
                        while let Some(&(_, ch)) = chars.peek() {
                            if ch.is_whitespace() {
                                chars.next();
                            } else {
                                break;
                            }
                        }

                        // Track position where next word should start
                        let next_word_pos = chars.peek().map(|(p, _)| *p).unwrap_or(pos);

                        // Read next word
                        let mut next_word = String::new();
                        while let Some(&(_, ch)) = chars.peek() {
                            if ch.is_alphanumeric() || ch == '_' {
                                next_word.push(ch);
                                chars.next();
                            } else {
                                break;
                            }
                        }

                        if next_word.is_empty() {
                            return Err(DnfError::UnexpectedToken {
                                expected: "OF (after ALL)".to_string(),
                                found: "end of expression".to_string(),
                                position: next_word_pos,
                                input: input_string.clone(),
                            });
                        }

                        if next_word == "OF" {
                            tokens.push(Token::AllOf);
                        } else {
                            return Err(DnfError::UnexpectedToken {
                                expected: "OF (after ALL)".to_string(),
                                found: next_word,
                                position: next_word_pos,
                                input: input_string.clone(),
                            });
                        }
                    }
                    // "ANY" is not supported - use "IN" instead
                    // ANY OF has been replaced by IN operator
                    _ => {
                        // Check if this is a custom operator
                        if let Some(custom_ops) = custom_op_names {
                            // Case-sensitive match for custom operators
                            if custom_ops.iter().any(|op| op == &ident) {
                                tokens.push(Token::CustomOp(ident.into_boxed_str()));
                            } else {
                                tokens.push(Token::Identifier(ident.into_boxed_str()));
                            }
                        } else {
                            tokens.push(Token::Identifier(ident.into_boxed_str()));
                        }
                    }
                }
            }

            _ => {
                return Err(DnfError::UnexpectedToken {
                    expected: "valid token".to_string(),
                    found: ch.to_string(),
                    position: pos,
                    input: input_string.clone(),
                });
            }
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Success Cases ====================

    struct TokenizeTestCase {
        name: &'static str,
        input: &'static str,
        expected: Vec<Token>,
    }

    #[test]
    fn test_tokenize_basic_expressions() {
        let cases = vec![
            TokenizeTestCase {
                name: "simple comparison",
                input: "age > 18",
                expected: vec![
                    Token::Identifier("age".into()),
                    Token::Gt,
                    Token::Number("18".into()),
                ],
            },
            TokenizeTestCase {
                name: "AND conjunction",
                input: "age > 18 AND country == \"US\"",
                expected: vec![
                    Token::Identifier("age".into()),
                    Token::Gt,
                    Token::Number("18".into()),
                    Token::And,
                    Token::Identifier("country".into()),
                    Token::Eq,
                    Token::String("US".into()),
                ],
            },
            TokenizeTestCase {
                name: "string with spaces",
                input: r#"name == "John Doe""#,
                expected: vec![
                    Token::Identifier("name".into()),
                    Token::Eq,
                    Token::String("John Doe".into()),
                ],
            },
            TokenizeTestCase {
                name: "escaped quotes",
                input: r#"name == "John \"The Boss\" Doe""#,
                expected: vec![
                    Token::Identifier("name".into()),
                    Token::Eq,
                    Token::String("John \"The Boss\" Doe".into()),
                ],
            },
            TokenizeTestCase {
                name: "boolean value",
                input: "premium == true",
                expected: vec![
                    Token::Identifier("premium".into()),
                    Token::Eq,
                    Token::Boolean(true),
                ],
            },
            TokenizeTestCase {
                name: "parentheses",
                input: "(age > 18)",
                expected: vec![
                    Token::LeftParen,
                    Token::Identifier("age".into()),
                    Token::Gt,
                    Token::Number("18".into()),
                    Token::RightParen,
                ],
            },
            TokenizeTestCase {
                name: "negative number",
                input: "age > -5",
                expected: vec![
                    Token::Identifier("age".into()),
                    Token::Gt,
                    Token::Number("-5".into()),
                ],
            },
            TokenizeTestCase {
                name: "float number",
                input: "price > 19.99",
                expected: vec![
                    Token::Identifier("price".into()),
                    Token::Gt,
                    Token::Number("19.99".into()),
                ],
            },
            TokenizeTestCase {
                name: "multiword string value",
                input: r#"description == "This is a multi word value""#,
                expected: vec![
                    Token::Identifier("description".into()),
                    Token::Eq,
                    Token::String("This is a multi word value".into()),
                ],
            },
        ];

        for case in cases {
            let tokens = tokenize(case.input, None).unwrap_or_else(|e| {
                panic!(
                    "Failed to tokenize '{}' ({}): {:?}",
                    case.name, case.input, e
                )
            });
            assert_eq!(
                tokens, case.expected,
                "Mismatch for '{}': {}",
                case.name, case.input
            );
        }
    }

    #[test]
    fn test_tokenize_operators() {
        let cases = vec![
            ("a == b", Token::Eq),
            ("a = b", Token::Eq),
            ("a != b", Token::Ne),
            ("a > b", Token::Gt),
            ("a < b", Token::Lt),
            ("a >= b", Token::Gte),
            ("a <= b", Token::Lte),
        ];

        for (input, expected_op) in cases {
            let tokens = tokenize(input, None).unwrap();
            assert_eq!(tokens[1], expected_op, "Failed for: {}", input);
        }
    }

    #[test]
    fn test_tokenize_string_operators() {
        let cases = vec![
            ("name CONTAINS \"John\"", Token::Contains),
            ("name NOT CONTAINS \"John\"", Token::NotContains),
            ("name STARTS WITH \"John\"", Token::StartsWith),
            ("name ENDS WITH \"Doe\"", Token::EndsWith),
            ("name NOT STARTS WITH \"X\"", Token::NotStartsWith),
            ("name NOT ENDS WITH \"Y\"", Token::NotEndsWith),
        ];

        for (input, expected_op) in cases {
            let tokens = tokenize(input, None).unwrap();
            assert_eq!(tokens[1], expected_op, "Failed for: {}", input);
        }
    }

    #[test]
    fn test_tokenize_between_operators() {
        let cases = vec![
            ("age BETWEEN [18, 65]", Token::Between),
            ("age NOT BETWEEN [0, 17]", Token::NotBetween),
            ("score BETWEEN [60.0, 100.0]", Token::Between),
        ];

        for (input, expected_op) in cases {
            let tokens = tokenize(input, None).unwrap();
            assert_eq!(tokens[1], expected_op, "Failed for: {}", input);
        }
    }

    #[test]
    fn test_tokenize_case_sensitive() {
        // Operators are UPPERCASE, constants are lowercase
        let tokens = tokenize("age > 18 AND premium == true", None).unwrap();
        assert_eq!(tokens[3], Token::And, "AND should be recognized");
        assert_eq!(tokens[6], Token::Boolean(true), "true should be recognized");

        // Lowercase 'and' should be treated as identifier
        let tokens = tokenize("age > 18 and premium == true", None).unwrap();
        assert_eq!(
            tokens[3],
            Token::Identifier("and".into()),
            "lowercase 'and' should be identifier"
        );
        assert_eq!(tokens[6], Token::Boolean(true), "true should still work");

        // Uppercase 'TRUE' should be treated as identifier (constants are lowercase)
        let tokens = tokenize("age > 18 AND premium == TRUE", None).unwrap();
        assert_eq!(tokens[3], Token::And, "AND should be recognized");
        assert_eq!(
            tokens[6],
            Token::Identifier("TRUE".into()),
            "uppercase 'TRUE' should be identifier"
        );

        // Mixed case operators should be treated as identifiers
        let tokens = tokenize("age > 18 AnD premium == true", None).unwrap();
        assert_eq!(
            tokens[3],
            Token::Identifier("AnD".into()),
            "mixed case 'AnD' should be identifier"
        );
    }

    #[test]
    fn test_tokenize_arrays() {
        let cases = vec![
            TokenizeTestCase {
                name: "string array",
                input: r#"status IN ["active", "pending"]"#,
                expected: vec![
                    Token::Identifier("status".into()),
                    Token::AnyOf,
                    Token::LeftBracket,
                    Token::String("active".into()),
                    Token::Comma,
                    Token::String("pending".into()),
                    Token::RightBracket,
                ],
            },
            TokenizeTestCase {
                name: "numeric array",
                input: "age IN [18, 21, 25]",
                expected: vec![
                    Token::Identifier("age".into()),
                    Token::AnyOf,
                    Token::LeftBracket,
                    Token::Number("18".into()),
                    Token::Comma,
                    Token::Number("21".into()),
                    Token::Comma,
                    Token::Number("25".into()),
                    Token::RightBracket,
                ],
            },
            TokenizeTestCase {
                name: "NOT IN operator",
                input: r#"status NOT IN ["deleted"]"#,
                expected: vec![
                    Token::Identifier("status".into()),
                    Token::NotAnyOf,
                    Token::LeftBracket,
                    Token::String("deleted".into()),
                    Token::RightBracket,
                ],
            },
            TokenizeTestCase {
                name: "IN without array",
                input: "status IN values",
                expected: vec![
                    Token::Identifier("status".into()),
                    Token::AnyOf,
                    Token::Identifier("values".into()),
                ],
            },
        ];

        for case in cases {
            let tokens = tokenize(case.input, None).unwrap();
            assert_eq!(
                tokens, case.expected,
                "Mismatch for '{}': {}",
                case.name, case.input
            );
        }
    }

    #[test]
    fn test_tokenize_nested_fields() {
        let cases = vec![
            TokenizeTestCase {
                name: "simple nested field",
                input: r#"user.name.first == "John""#,
                expected: vec![
                    Token::Identifier("user.name.first".into()),
                    Token::Eq,
                    Token::String("John".into()),
                ],
            },
            TokenizeTestCase {
                name: "nested fields with operators",
                input: "person.age > 18 AND person.contact.email CONTAINS \"@\"",
                expected: vec![
                    Token::Identifier("person.age".into()),
                    Token::Gt,
                    Token::Number("18".into()),
                    Token::And,
                    Token::Identifier("person.contact.email".into()),
                    Token::Contains,
                    Token::String("@".into()),
                ],
            },
        ];

        for case in cases {
            let tokens = tokenize(case.input, None).unwrap();
            assert_eq!(
                tokens, case.expected,
                "Mismatch for '{}': {}",
                case.name, case.input
            );
        }
    }

    #[test]
    fn test_tokenize_multiword_operators_with_whitespace() {
        let cases = vec![
            ("name NOT    CONTAINS \"value\"", Token::NotContains),
            ("name STARTS    WITH \"value\"", Token::StartsWith),
            ("name ENDS    WITH \"value\"", Token::EndsWith),
        ];

        for (input, expected_op) in cases {
            let tokens = tokenize(input, None).unwrap();
            assert_eq!(tokens[1], expected_op, "Failed for: {}", input);
        }
    }

    // ==================== Error Cases ====================

    struct TokenizeErrorCase {
        name: &'static str,
        input: &'static str,
        expected_contains: &'static str,
    }

    #[test]
    fn test_tokenize_escape_sequences() {
        let cases = vec![
            TokenizeTestCase {
                name: "escaped backslash",
                input: r#"path == "C:\\Users\\John""#,
                expected: vec![
                    Token::Identifier("path".into()),
                    Token::Eq,
                    Token::String("C:\\Users\\John".into()),
                ],
            },
            TokenizeTestCase {
                name: "escaped forward slash",
                input: r#"url == "https:\/\/example.com""#,
                expected: vec![
                    Token::Identifier("url".into()),
                    Token::Eq,
                    Token::String("https://example.com".into()),
                ],
            },
            TokenizeTestCase {
                name: "escaped quotes in string",
                input: r#"quote == "He said \"Hello\"""#,
                expected: vec![
                    Token::Identifier("quote".into()),
                    Token::Eq,
                    Token::String(r#"He said "Hello""#.into()),
                ],
            },
            TokenizeTestCase {
                name: "newline and tab",
                input: "text == \"Line1\\nLine2\\tTabbed\"",
                expected: vec![
                    Token::Identifier("text".into()),
                    Token::Eq,
                    Token::String("Line1\nLine2\tTabbed".into()),
                ],
            },
            TokenizeTestCase {
                name: "mixed escapes",
                input: r#"data == "Path: C:\\test\nURL: https:\/\/site.com""#,
                expected: vec![
                    Token::Identifier("data".into()),
                    Token::Eq,
                    Token::String("Path: C:\\test\nURL: https://site.com".into()),
                ],
            },
        ];

        for case in cases {
            let tokens = tokenize(case.input, None).unwrap_or_else(|e| {
                panic!(
                    "Failed to tokenize '{}' ({}): {:?}",
                    case.name, case.input, e
                )
            });
            assert_eq!(
                tokens, case.expected,
                "Mismatch for '{}': {}",
                case.name, case.input
            );
        }
    }

    #[test]
    fn test_tokenize_unterminated_strings() {
        let cases = vec![
            ("double quote", r#"name == "unclosed"#),
            ("single quote", "name == 'unclosed string"),
            ("with escape", r#"name == "has escape \n but no end"#),
        ];

        for (name, input) in cases {
            let result = tokenize(input, None);
            assert!(
                matches!(result, Err(DnfError::UnterminatedString { .. })),
                "Expected UnterminatedString for '{}': {}",
                name,
                input
            );
        }
    }

    #[test]
    fn test_tokenize_incomplete_multiword_operators() {
        let cases = vec![
            TokenizeErrorCase {
                name: "NOT at EOF",
                input: "name NOT",
                expected_contains: "CONTAINS",
            },
            TokenizeErrorCase {
                name: "NOT STARTS without WITH",
                input: "name NOT STARTS",
                expected_contains: "WITH",
            },
            TokenizeErrorCase {
                name: "NOT before value",
                input: r#"name NOT "value""#,
                expected_contains: "CONTAINS",
            },
            TokenizeErrorCase {
                name: "STARTS at EOF",
                input: "name STARTS",
                expected_contains: "WITH",
            },
            TokenizeErrorCase {
                name: "STARTS with wrong word",
                input: "name STARTS BY",
                expected_contains: "WITH",
            },
            TokenizeErrorCase {
                name: "ENDS at EOF",
                input: "name ENDS",
                expected_contains: "WITH",
            },
            TokenizeErrorCase {
                name: "ENDS with wrong word",
                input: "name ENDS IN",
                expected_contains: "WITH",
            },
        ];

        for case in cases {
            let result = tokenize(case.input, None);
            assert!(
                matches!(result, Err(DnfError::UnexpectedToken { .. })),
                "Expected UnexpectedToken for '{}': {}",
                case.name,
                case.input
            );

            if let Err(DnfError::UnexpectedToken { expected, .. }) = result {
                assert!(
                    expected.contains(case.expected_contains),
                    "Error for '{}' should contain '{}', got: {}",
                    case.name,
                    case.expected_contains,
                    expected
                );
            }
        }
    }
}

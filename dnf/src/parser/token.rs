use crate::error::DnfError;

/// Tokens for the query language.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    // Identifiers and keywords
    Identifier(Box<str>),
    And,
    Or,

    // Operators
    Eq,                    // == or =
    Ne,                    // !=
    Gt,                    // >
    Lt,                    // <
    Gte,                   // >=
    Lte,                   // <=
    Contains,              // CONTAINS
    NotContains,           // NOT CONTAINS
    StartsWith,            // STARTS WITH
    EndsWith,              // ENDS WITH
    NotStartsWith,         // NOT STARTS WITH
    NotEndsWith,           // NOT ENDS WITH
    AllOf,                 // ALL OF
    AnyOf,                 // IN (value in array)
    NotAllOf,              // NOT ALL OF
    NotAnyOf,              // NOT IN (value not in array)
    Between,               // BETWEEN [min, max]
    NotBetween,            // NOT BETWEEN [min, max]
    CustomOp(Box<str>),    // Custom operator (e.g., IS_ADULT)
    NotCustomOp(Box<str>), // NOT <custom operator> (e.g., NOT IS_ADULT)

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
    MapKeys,
    MapValues,

    /// Internal sentinel produced after `mem::replace` consumes a token slot.
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
            Token::NotCustomOp(name) => write!(f, "NOT {}", name),
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

type CharStream<'a> = std::iter::Peekable<std::str::CharIndices<'a>>;

#[inline]
fn skip_whitespace(chars: &mut CharStream<'_>) {
    while let Some(&(_, ch)) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
        } else {
            break;
        }
    }
}

/// Sigil that introduces a map target: `.@keys`, `.@values`.
const MAP_SIGIL: char = '@';

/// Consumes a run of word characters (alphanumerics and `_`).
fn read_word(chars: &mut CharStream<'_>) -> String {
    let mut word = String::new();
    while let Some(&(_, ch)) = chars.peek() {
        if ch.is_alphanumeric() || ch == '_' {
            word.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    word
}

/// Returns `true` if the stream is positioned at a `.` that starts a map
/// target (`.@keys`, `.@values`) rather than a nested field separator.
fn at_map_target(chars: &CharStream<'_>) -> bool {
    let mut ahead = chars.clone();
    matches!(
        (ahead.next(), ahead.next()),
        (Some((_, '.')), Some((_, MAP_SIGIL)))
    )
}

/// Skips whitespace and consumes an identifier.
///
/// Returns the start position (or `fallback_pos` at end of input) and the identifier.
fn read_keyword(chars: &mut CharStream<'_>, fallback_pos: usize) -> (usize, String) {
    skip_whitespace(chars);
    let start = chars.peek().map(|(p, _)| *p).unwrap_or(fallback_pos);
    (start, read_word(chars))
}

/// Skips whitespace and verifies the next identifier matches `expected`.
fn expect_keyword(
    chars: &mut CharStream<'_>,
    expected: &str,
    fallback_pos: usize,
    input: &str,
    expected_label: &str,
) -> Result<(), DnfError> {
    let (pos, word) = read_keyword(chars, fallback_pos);
    if word == expected {
        Ok(())
    } else {
        let found = if word.is_empty() {
            "end of expression".to_string()
        } else {
            word
        };
        Err(DnfError::UnexpectedToken {
            expected: expected_label.to_string(),
            found,
            position: pos,
            input: input.to_string(),
        })
    }
}

/// Returns `true` if `name` is one of the registered custom operator names.
fn is_custom_op_name(custom_op_names: Option<&[String]>, name: &str) -> bool {
    custom_op_names.is_some_and(|ops| ops.iter().any(|op| op == name))
}

/// Tokenize a query string into a vector of tokens paired with byte spans.
///
/// Each token carries the `start..end` byte range it occupies in `input`, so
/// the parser can report accurate positions in its errors.
///
/// # Arguments
///
/// * `input` - The query string to tokenize
/// * `custom_op_names` - Optional slice of custom operator names to recognize
pub(crate) fn tokenize(
    input: &str,
    custom_op_names: Option<&[String]>,
) -> Result<Vec<(Token, std::ops::Range<usize>)>, DnfError> {
    // `input` is only materialized into an owned `String` on the cold error
    // paths below, so a successful tokenize never clones the whole input.
    let mut tokens: Vec<Token> = Vec::new();
    let mut spans: Vec<std::ops::Range<usize>> = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some((pos, ch)) = chars.next() {
        // Each iteration below pushes at most one token; whitespace `continue`s
        // and error arms `return`. The span is `start..(next unconsumed byte)`.
        let start = pos;
        let len_before = tokens.len();
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
                        input: input.to_string(),
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
                if chars.peek().map(|(_, c)| *c) == Some(MAP_SIGIL) {
                    chars.next(); // consume the sigil

                    // Read the target name
                    let target = read_word(&mut chars);

                    match target.as_str() {
                        "keys" => tokens.push(Token::MapKeys),
                        "values" => tokens.push(Token::MapValues),
                        _ => {
                            return Err(DnfError::UnexpectedToken {
                                expected: "@keys or @values".to_string(),
                                found: format!("@{}", target),
                                position: pos,
                                input: input.to_string(),
                            });
                        }
                    }
                } else {
                    return Err(DnfError::UnexpectedToken {
                        expected: "identifier or @".to_string(),
                        found: ".".to_string(),
                        position: pos,
                        input: input.to_string(),
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
                                    input: input.to_string(),
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
                        input: input.to_string(),
                    });
                }
            }

            // Numbers: `-?[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?`. A leading `+` is
            // rejected; `-` must precede a digit.
            '0'..='9' | '+' | '-' => {
                // A leading `+` is not part of the number grammar.
                if ch == '+' {
                    return Err(DnfError::InvalidNumber {
                        value: ch.to_string(),
                        position: pos,
                        input: input.to_string(),
                    });
                }

                let mut number = String::new();
                number.push(ch);

                // A leading `-` must be followed by a digit.
                if ch == '-'
                    && !chars
                        .peek()
                        .map(|(_, c)| c.is_ascii_digit())
                        .unwrap_or(false)
                {
                    return Err(DnfError::InvalidNumber {
                        value: number,
                        position: pos,
                        input: input.to_string(),
                    });
                }

                // Integer and optional fraction digits.
                let mut seen_dot = false;
                while let Some(&(dot_pos, c)) = chars.peek() {
                    if c.is_ascii_digit() {
                        number.push(c);
                        chars.next();
                    } else if c == '.' {
                        if seen_dot {
                            return Err(DnfError::InvalidNumber {
                                value: number,
                                position: dot_pos,
                                input: input.to_string(),
                            });
                        }
                        seen_dot = true;
                        number.push(c);
                        chars.next();
                        // A `.` must be followed by at least one digit (`1.`).
                        if !chars
                            .peek()
                            .map(|(_, d)| d.is_ascii_digit())
                            .unwrap_or(false)
                        {
                            return Err(DnfError::InvalidNumber {
                                value: number,
                                position: dot_pos,
                                input: input.to_string(),
                            });
                        }
                    } else {
                        break;
                    }
                }

                // Optional exponent: `[eE][+-]?[0-9]+`.
                if let Some(&(e_pos, e)) = chars.peek() {
                    if e == 'e' || e == 'E' {
                        number.push(e);
                        chars.next();
                        // Optional exponent sign.
                        if let Some(&(_, sign)) = chars.peek() {
                            if sign == '+' || sign == '-' {
                                number.push(sign);
                                chars.next();
                            }
                        }
                        // At least one exponent digit is required.
                        if !chars
                            .peek()
                            .map(|(_, d)| d.is_ascii_digit())
                            .unwrap_or(false)
                        {
                            return Err(DnfError::InvalidNumber {
                                value: number,
                                position: e_pos,
                                input: input.to_string(),
                            });
                        }
                        while let Some(&(_, d)) = chars.peek() {
                            if d.is_ascii_digit() {
                                number.push(d);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                    }
                }

                // A number cannot run directly into identifier characters
                // (`18abc`), which would otherwise lex as a separate identifier.
                if let Some(&(id_pos, c)) = chars.peek() {
                    if c.is_ascii_alphabetic() || c == '_' {
                        return Err(DnfError::InvalidNumber {
                            value: number,
                            position: id_pos,
                            input: input.to_string(),
                        });
                    }
                }

                tokens.push(Token::Number(number.into_boxed_str()));
            }

            // Identifiers and keywords (supports nested fields like user.name.first)
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut ident = String::new();
                ident.push(ch);

                loop {
                    ident.push_str(&read_word(&mut chars));
                    // A `.` continues a nested field unless it starts a map target.
                    let is_nested_sep =
                        matches!(chars.peek(), Some(&(_, '.'))) && !at_map_target(&chars);
                    if !is_nested_sep {
                        break;
                    }
                    ident.push('.');
                    chars.next();
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
                    // A registered custom operator takes precedence over the
                    // built-in operator keywords below, so custom ops whose
                    // names collide with a keyword (e.g. `BETWEEN`) survive the
                    // text syntax. Structural tokens (`AND`/`OR`) and value
                    // literals (`true`/`false`/`null`) above are never shadowed.
                    _ if is_custom_op_name(custom_op_names, &ident) => {
                        tokens.push(Token::CustomOp(ident.into_boxed_str()));
                    }
                    "CONTAINS" => tokens.push(Token::Contains),
                    "IN" => tokens.push(Token::AnyOf), // IN is alias for ANY OF
                    "BETWEEN" => tokens.push(Token::Between),
                    "NOT" => {
                        let (next_word_pos, next_word) = read_keyword(&mut chars, pos);
                        if next_word.is_empty() {
                            return Err(DnfError::UnexpectedToken {
                                expected:
                                    "CONTAINS, IN, BETWEEN, STARTS, ENDS, ALL, or ANY (after NOT)"
                                        .to_string(),
                                found: "end of expression".to_string(),
                                position: next_word_pos,
                                input: input.to_string(),
                            });
                        }
                        match next_word.as_str() {
                            "CONTAINS" => tokens.push(Token::NotContains),
                            "IN" => tokens.push(Token::NotAnyOf), // NOT IN is alias for NOT ANY OF
                            "BETWEEN" => tokens.push(Token::NotBetween),
                            "STARTS" => {
                                expect_keyword(
                                    &mut chars,
                                    "WITH",
                                    pos,
                                    input,
                                    "WITH (after NOT STARTS)",
                                )?;
                                tokens.push(Token::NotStartsWith);
                            }
                            "ENDS" => {
                                expect_keyword(
                                    &mut chars,
                                    "WITH",
                                    pos,
                                    input,
                                    "WITH (after NOT ENDS)",
                                )?;
                                tokens.push(Token::NotEndsWith);
                            }
                            "ALL" => {
                                expect_keyword(&mut chars, "OF", pos, input, "OF (after NOT ALL)")?;
                                tokens.push(Token::NotAllOf);
                            }
                            // "NOT ANY" is not supported - use "NOT IN" instead
                            // A registered custom operator may follow NOT, e.g.
                            // `age NOT IS_ADULT`, yielding a negated custom op.
                            _ if is_custom_op_name(custom_op_names, &next_word) => {
                                tokens.push(Token::NotCustomOp(next_word.into_boxed_str()));
                            }
                            _ => {
                                return Err(DnfError::UnexpectedToken {
                                    expected:
                                        "CONTAINS, IN, BETWEEN, STARTS, ENDS, or ALL (after NOT)"
                                            .to_string(),
                                    found: next_word,
                                    position: next_word_pos,
                                    input: input.to_string(),
                                });
                            }
                        }
                    }
                    "STARTS" => {
                        expect_keyword(&mut chars, "WITH", pos, input, "WITH (after STARTS)")?;
                        tokens.push(Token::StartsWith);
                    }
                    "ENDS" => {
                        expect_keyword(&mut chars, "WITH", pos, input, "WITH (after ENDS)")?;
                        tokens.push(Token::EndsWith);
                    }
                    "ALL" => {
                        expect_keyword(&mut chars, "OF", pos, input, "OF (after ALL)")?;
                        tokens.push(Token::AllOf);
                    }
                    // "ANY" is not supported - use "IN" instead
                    // ANY OF has been replaced by IN operator
                    _ => tokens.push(Token::Identifier(ident.into_boxed_str())),
                }
            }

            _ => {
                return Err(DnfError::UnexpectedToken {
                    expected: "valid token".to_string(),
                    found: ch.to_string(),
                    position: pos,
                    input: input.to_string(),
                });
            }
        }

        // Record the span of the token pushed by this iteration.
        if tokens.len() > len_before {
            let end = chars.peek().map(|(p, _)| *p).unwrap_or(input.len());
            spans.push(start..end);
        }
    }

    Ok(tokens.into_iter().zip(spans).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tokenizes `input`, discarding spans, for tests that assert only on the
    /// token sequence.
    fn lex(input: &str) -> Result<Vec<Token>, DnfError> {
        tokenize(input, None).map(|toks| toks.into_iter().map(|(t, _)| t).collect())
    }

    /// Tokenizes `input` with `names` registered as custom operators,
    /// discarding spans.
    fn lex_custom(input: &str, names: &[&str]) -> Result<Vec<Token>, DnfError> {
        let names: Vec<String> = names.iter().map(|s| s.to_string()).collect();
        tokenize(input, Some(&names)).map(|toks| toks.into_iter().map(|(t, _)| t).collect())
    }

    #[test]
    fn test_tokenize_custom_op_precedence() {
        let cases = vec![
            // (input, custom names, expected tokens, description)
            (
                "score BETWEEN 5",
                vec!["BETWEEN"],
                vec![
                    Token::Identifier("score".into()),
                    Token::CustomOp("BETWEEN".into()),
                    Token::Number("5".into()),
                ],
                "registered name shadows the built-in operator keyword",
            ),
            (
                "score BETWEEN [1, 2]",
                vec![],
                vec![
                    Token::Identifier("score".into()),
                    Token::Between,
                    Token::LeftBracket,
                    Token::Number("1".into()),
                    Token::Comma,
                    Token::Number("2".into()),
                    Token::RightBracket,
                ],
                "built-in keyword wins when no custom op is registered",
            ),
            (
                "age IS_ADULT",
                vec!["IS_ADULT"],
                vec![
                    Token::Identifier("age".into()),
                    Token::CustomOp("IS_ADULT".into()),
                ],
                "non-reserved custom name tokenizes as custom op",
            ),
            (
                "age NOT IS_ADULT",
                vec!["IS_ADULT"],
                vec![
                    Token::Identifier("age".into()),
                    Token::NotCustomOp("IS_ADULT".into()),
                ],
                "NOT followed by a custom op yields a negated custom op",
            ),
            (
                "age NOT CONTAINS 1",
                vec!["CONTAINS"],
                vec![
                    Token::Identifier("age".into()),
                    Token::NotContains,
                    Token::Number("1".into()),
                ],
                "built-in NOT combo still wins after NOT",
            ),
        ];
        for (input, names, expected, desc) in cases {
            let actual = lex_custom(input, &names).expect("should tokenize");
            assert_eq!(actual, expected, "Failed: {}", desc);
        }
    }

    #[test]
    fn test_tokenize_structural_keywords_never_shadowed() {
        let cases = vec![
            // (input, custom names, expected token at index 0, description)
            ("AND", vec!["AND"], Token::And, "AND is never a custom op"),
            ("OR", vec!["OR"], Token::Or, "OR is never a custom op"),
            (
                "true",
                vec!["true"],
                Token::Boolean(true),
                "true literal is never a custom op",
            ),
            (
                "null",
                vec!["null"],
                Token::Null,
                "null literal is never a custom op",
            ),
        ];
        for (input, names, expected, desc) in cases {
            let actual = lex_custom(input, &names).expect("should tokenize");
            assert_eq!(actual.first(), Some(&expected), "Failed: {}", desc);
        }
    }

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
            let tokens = lex(case.input).unwrap_or_else(|e| {
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
            let tokens = lex(input).unwrap();
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
            let tokens = lex(input).unwrap();
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
            let tokens = lex(input).unwrap();
            assert_eq!(tokens[1], expected_op, "Failed for: {}", input);
        }
    }

    #[test]
    fn test_tokenize_case_sensitive() {
        // Operators are UPPERCASE, constants are lowercase
        let tokens = lex("age > 18 AND premium == true").unwrap();
        assert_eq!(tokens[3], Token::And, "AND should be recognized");
        assert_eq!(tokens[6], Token::Boolean(true), "true should be recognized");

        // Lowercase 'and' should be treated as identifier
        let tokens = lex("age > 18 and premium == true").unwrap();
        assert_eq!(
            tokens[3],
            Token::Identifier("and".into()),
            "lowercase 'and' should be identifier"
        );
        assert_eq!(tokens[6], Token::Boolean(true), "true should still work");

        // Uppercase 'TRUE' should be treated as identifier (constants are lowercase)
        let tokens = lex("age > 18 AND premium == TRUE").unwrap();
        assert_eq!(tokens[3], Token::And, "AND should be recognized");
        assert_eq!(
            tokens[6],
            Token::Identifier("TRUE".into()),
            "uppercase 'TRUE' should be identifier"
        );

        // Mixed case operators should be treated as identifiers
        let tokens = lex("age > 18 AnD premium == true").unwrap();
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
            let tokens = lex(case.input).unwrap();
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
            let tokens = lex(case.input).unwrap();
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
            let tokens = lex(input).unwrap();
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
            let tokens = lex(case.input).unwrap_or_else(|e| {
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

    #[test]
    fn test_tokenize_invalid_numbers() {
        let cases = vec![
            ("two dots", "x == 1.2.3"),
            ("trailing extra dot", "x == 1.2.3.4"),
            ("multiple dots no digits", "x == 1.."),
        ];

        for (name, input) in cases {
            let result = tokenize(input, None);
            assert!(
                matches!(result, Err(DnfError::InvalidNumber { .. })),
                "Expected InvalidNumber for '{}': {}",
                name,
                input
            );
        }
    }

    #[test]
    fn test_tokenize_exponents() {
        // `[eE][+-]?[0-9]+` exponents lex to a single `Token::Number` carrying the
        // literal text verbatim.
        let cases = vec![
            ("x == 1e5", "1e5"),
            ("x == 1E5", "1E5"),
            ("x == 1.5e3", "1.5e3"),
            ("x == 2e-3", "2e-3"),
            ("x == 2E+3", "2E+3"),
            ("x == -1.5e10", "-1.5e10"),
        ];

        for (input, expected_literal) in cases {
            let tokens =
                lex(input).unwrap_or_else(|e| panic!("Expected '{}' to tokenize: {:?}", input, e));
            let number = tokens
                .iter()
                .find_map(|t| match t {
                    Token::Number(n) => Some(n.as_ref()),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("No number token for '{}'", input));
            assert_eq!(number, expected_literal, "Failed: {}", input);
        }
    }

    #[test]
    fn test_tokenize_number_format_errors() {
        // Malformed numbers are rejected at lex time with a clear byte position.
        // (input, expected byte position of the offending char)
        let cases = vec![
            ("leading plus", "x == +5", 5),
            ("trailing dot", "x == 1.", 6),
            ("digits into identifier", "x == 18abc", 7),
            ("digits into underscore", "x == 5_000", 6),
            ("exponent without digits", "x == 1e", 6),
            ("exponent sign without digits", "x == 1e+", 6),
        ];

        for (name, input, expected_pos) in cases {
            match tokenize(input, None) {
                Err(DnfError::InvalidNumber { position, .. }) => {
                    assert_eq!(position, expected_pos, "Wrong position for '{}'", name);
                }
                other => panic!("Expected InvalidNumber for '{}', got: {:?}", name, other),
            }
        }
    }

    #[test]
    fn test_tokenize_spans() {
        // Each token carries the exact `start..end` byte range it spans.
        // (input, expected (token, span) pairs)
        type SpanCase = (&'static str, Vec<(Token, std::ops::Range<usize>)>);
        let cases: Vec<SpanCase> = vec![
            (
                "age > 18",
                vec![
                    (Token::Identifier("age".into()), 0..3),
                    (Token::Gt, 4..5),
                    (Token::Number("18".into()), 6..8),
                ],
            ),
            (
                r#"name == "US""#,
                vec![
                    (Token::Identifier("name".into()), 0..4),
                    (Token::Eq, 5..7),
                    (Token::String("US".into()), 8..12),
                ],
            ),
            (
                // `café` is 5 bytes (`é` is 2), so the identifier spans 0..5 and
                // the operator starts at byte 6.
                "café > 1",
                vec![
                    (Token::Identifier("café".into()), 0..5),
                    (Token::Gt, 6..7),
                    (Token::Number("1".into()), 8..9),
                ],
            ),
        ];

        for (input, expected) in cases {
            let tokens = tokenize(input, None)
                .unwrap_or_else(|e| panic!("Failed to tokenize '{}': {:?}", input, e));
            assert_eq!(tokens, expected, "Span mismatch for: {}", input);
        }
    }
}

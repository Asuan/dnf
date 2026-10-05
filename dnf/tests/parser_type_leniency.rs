//! Tests that the parser accepts cross-type numeric literals and that the
//! resulting queries evaluate correctly (parser type-leniency, Phase 2).

#![cfg(feature = "parser")]

use dnf::{DnfEvaluable, DnfQuery};
use std::collections::HashMap;

#[derive(DnfEvaluable, Debug)]
struct User {
    age: u32,
}

#[derive(DnfEvaluable, Debug)]
struct Account {
    name: String,
    nick: Option<String>,
}

#[test]
fn test_null_operand_evaluates_without_error() {
    // A `null` operand is accepted by the parser and resolved at eval time
    // rather than rejected: string operators yield `false`, and equality is a
    // real null/None check.
    let acc = Account {
        name: "alice".to_string(),
        nick: None,
    };

    // (query, expected eval result, description)
    let cases = vec![
        // String operators against a populated field: null operand → false.
        ("name CONTAINS null", false, "CONTAINS null → false"),
        ("name STARTS WITH null", false, "STARTS WITH null → false"),
        ("name ENDS WITH null", false, "ENDS WITH null → false"),
        // Equality is a genuine null check.
        (
            "name == null",
            false,
            "non-Option field is never null → false",
        ),
        (
            "name != null",
            true,
            "non-Option field is never null → true",
        ),
        ("nick == null", true, "None Option field equals null → true"),
        ("nick != null", false, "None Option field != null → false"),
    ];

    for (query_str, expected, desc) in cases {
        let query = DnfQuery::parse::<Account>(query_str)
            .unwrap_or_else(|e| panic!("'{}' should parse: {:?}", query_str, e));
        assert_eq!(query.evaluate(&acc), expected, "Failed: {}", desc);
    }
}

#[test]
fn test_parse_cross_numeric_evaluates() {
    // A numeric literal of any sign parses on an unsigned field and evaluates
    // cross-type: `age > -5` is a real comparison, not a parse error.
    let cases = vec![
        (0u32, "age > -5", true, "0 > -5 cross-type"),
        (10u32, "age > -5", true, "10 > -5 cross-type"),
        (3u32, "age == 3", true, "unsigned literal matches"),
        (3u32, "age < 10", true, "unsigned literal upper bound"),
        (
            20u32,
            "age < 10",
            false,
            "unsigned literal upper bound (false)",
        ),
    ];

    for (age, query_str, expected, desc) in cases {
        let user = User { age };
        let query = DnfQuery::parse::<User>(query_str)
            .unwrap_or_else(|e| panic!("Failed to parse '{}': {:?}", query_str, e));
        assert_eq!(query.evaluate(&user), expected, "Failed: {}", desc);
    }
}

#[test]
fn test_between_negative_lower_bound_on_unsigned_field() {
    // A negative lower bound on an unsigned field is clamped to 0 rather than
    // making the whole range unsatisfiable.
    let cases = vec![
        (3u32, "age BETWEEN [-5, 10]", true),
        (8u32, "age BETWEEN [-5, 10]", true),
        (11u32, "age BETWEEN [-5, 10]", false),
        (3u32, "age NOT BETWEEN [-5, 10]", false),
        // `0` sits at the clamped lower bound, so it must be included.
        (0u32, "age BETWEEN [-5, 10]", true),
        (0u32, "age NOT BETWEEN [-5, 10]", false),
        // `0` is also the explicit lower bound.
        (0u32, "age BETWEEN [0, 10]", true),
        // Upper bound of exactly `0`: the range collapses to {0}.
        (0u32, "age BETWEEN [-5, 0]", true),
        (1u32, "age BETWEEN [-5, 0]", false),
        // A negative upper bound leaves no unsigned value in range, even for 0.
        (0u32, "age BETWEEN [-5, -1]", false),
        (0u32, "age BETWEEN [-10, -5]", false),
        (0u32, "age NOT BETWEEN [-10, -5]", true),
    ];

    for (age, query_str, expected) in cases {
        let query = DnfQuery::parse::<User>(query_str)
            .unwrap_or_else(|e| panic!("'{}' should parse: {:?}", query_str, e));
        assert_eq!(
            query.evaluate(&User { age }),
            expected,
            "{query_str} @ {age}"
        );
    }
}

#[derive(DnfEvaluable, Debug)]
struct Tagged {
    name: String,
    tags: Vec<String>,
    meta: HashMap<String, i64>,
}

#[test]
fn test_map_field_operands_are_validated() {
    // Map-field conditions get the same operand-shape checks as other fields.
    let rejected = vec![
        r#"meta["k"] BETWEEN [1]"#,
        r#"meta["k"] BETWEEN [1, 2, 3]"#,
        r#"meta["k"] IN 1"#,
    ];
    for query_str in rejected {
        assert!(
            DnfQuery::parse::<Tagged>(query_str).is_err(),
            "'{query_str}' should be rejected"
        );
    }

    let accepted = vec![
        r#"meta["k"] BETWEEN [1, 2]"#,
        r#"meta["k"] IN [1, 2]"#,
        r#"meta["k"] == 1"#,
    ];
    for query_str in accepted {
        assert!(
            DnfQuery::parse::<Tagged>(query_str).is_ok(),
            "'{query_str}' should parse"
        );
    }
}

#[test]
fn test_array_operand_on_scalar_equality_is_rejected() {
    // An array can never equal a scalar field; membership operators exist for that.
    for query_str in [
        r#"name == ["a", "b"]"#,
        r#"name != ["a"]"#,
        r#"name CONTAINS ["a"]"#,
    ] {
        assert!(
            DnfQuery::parse::<Tagged>(query_str).is_err(),
            "'{query_str}' should be rejected"
        );
    }
    assert!(DnfQuery::parse::<Tagged>(r#"name IN ["a", "b"]"#).is_ok());
}

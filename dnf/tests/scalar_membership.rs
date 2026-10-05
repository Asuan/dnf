//! End-to-end tests for scalar `IN` / `ALL OF` membership through the parser and
//! derive macro (regression for scalar `IN`/`ALL OF` previously failing open).

#![cfg(feature = "parser")]

use dnf::{DnfEvaluable, DnfQuery};

#[derive(DnfEvaluable, Debug)]
struct Account {
    name: String,
    status: String,
    age: u32,
}

#[test]
fn test_scalar_in_membership_evaluates() {
    let acc = Account {
        name: "banned".to_string(),
        status: "active".to_string(),
        age: 5,
    };

    // (query, expected eval result, description)
    let cases = vec![
        // ANY OF / IN is real membership, not a hardwired false.
        (
            r#"status IN ["active", "pending"]"#,
            true,
            "IN matches a member",
        ),
        (
            r#"status IN ["deleted", "blocked"]"#,
            false,
            "IN misses a non-member",
        ),
        ("age IN [3, 5, 7]", true, "numeric IN matches"),
        ("age IN [3, 7]", false, "numeric IN misses"),
        // NOT IN inverts membership and no longer matches a listed value.
        (
            r#"name NOT IN ["banned", "deleted"]"#,
            false,
            "NOT IN a listed value is false",
        ),
        (
            r#"name NOT IN ["alice", "bob"]"#,
            true,
            "NOT IN an unlisted value is true",
        ),
        (
            "age NOT IN [5, 6]",
            false,
            "numeric NOT IN a member is false",
        ),
        // ALL OF holds only when every element equals the scalar.
        ("age ALL OF [5, 5]", true, "ALL OF all-equal"),
        ("age ALL OF [5, 6]", false, "ALL OF mixed"),
    ];

    for (query_str, expected, desc) in cases {
        let query = DnfQuery::parse::<Account>(query_str)
            .unwrap_or_else(|e| panic!("'{}' should parse: {:?}", query_str, e));
        assert_eq!(query.evaluate(&acc), expected, "Failed: {}", desc);
    }
}

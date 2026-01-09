//! Tests for HashMap field support in DNF queries.

#[cfg(feature = "parser")]
use dnf::QueryBuilder;
use dnf::{DnfEvaluable, DnfQuery, FieldKind, Op, Value};
use std::collections::HashMap;

// ==================== Basic HashMap Struct ====================

#[derive(DnfEvaluable, Debug)]
struct Document {
    title: String,
    metadata: HashMap<String, String>,
}

#[derive(DnfEvaluable, Debug)]
struct Config {
    name: String,
    settings: HashMap<String, i64>,
}

// ==================== Tests: AtKey Access ====================

#[test]
fn test_hashmap_at_key_equals() {
    let mut metadata = HashMap::new();
    metadata.insert("author".to_string(), "Alice".to_string());
    metadata.insert("version".to_string(), "1.0".to_string());

    let doc = Document {
        title: "README".to_string(),
        metadata,
    };

    // metadata["author"] == "Alice"
    let query = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::EQ, Value::at_key("author", "Alice")))
        .build();

    assert!(query.evaluate(&doc));
}

#[test]
fn test_hashmap_at_key_not_equals() {
    let mut metadata = HashMap::new();
    metadata.insert("author".to_string(), "Alice".to_string());

    let doc = Document {
        title: "README".to_string(),
        metadata,
    };

    // metadata["author"] != "Bob"
    let query = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::NE, Value::at_key("author", "Bob")))
        .build();

    assert!(query.evaluate(&doc));
}

#[test]
fn test_hashmap_at_key_missing() {
    let metadata = HashMap::new();
    let doc = Document {
        title: "README".to_string(),
        metadata,
    };

    // metadata["author"] == "Alice" -> false (key doesn't exist)
    let query = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::EQ, Value::at_key("author", "Alice")))
        .build();

    assert!(!query.evaluate(&doc));
}

#[test]
fn test_hashmap_at_key_missing_not_equals() {
    let metadata = HashMap::new();
    let doc = Document {
        title: "README".to_string(),
        metadata,
    };

    // metadata["author"] != "Alice" -> true (missing key != anything)
    let query = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::NE, Value::at_key("author", "Alice")))
        .build();

    assert!(query.evaluate(&doc));
}

#[test]
fn test_hashmap_at_key_numeric() {
    let mut settings = HashMap::new();
    settings.insert("timeout".to_string(), 30);
    settings.insert("retries".to_string(), 3);

    let config = Config {
        name: "default".to_string(),
        settings,
    };

    // settings["timeout"] > 20
    let query = DnfQuery::builder()
        .or(|c| c.and("settings", Op::GT, Value::at_key("timeout", 20)))
        .build();

    assert!(query.evaluate(&config));
}

// ==================== Tests: Keys Access ====================

#[test]
fn test_hashmap_keys_contains() {
    let mut metadata = HashMap::new();
    metadata.insert("author".to_string(), "Alice".to_string());
    metadata.insert("version".to_string(), "1.0".to_string());

    let doc = Document {
        title: "README".to_string(),
        metadata,
    };

    // metadata.@keys CONTAINS "author"
    let query = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::CONTAINS, Value::keys("author")))
        .build();

    assert!(query.evaluate(&doc));
}

#[test]
fn test_hashmap_keys_not_contains() {
    let mut metadata = HashMap::new();
    metadata.insert("author".to_string(), "Alice".to_string());

    let doc = Document {
        title: "README".to_string(),
        metadata,
    };

    // metadata.@keys NOT CONTAINS "missing_key"
    let query = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::NOT_CONTAINS, Value::keys("missing_key")))
        .build();

    assert!(query.evaluate(&doc));
}

#[test]
fn test_hashmap_keys_all_of() {
    let mut metadata = HashMap::new();
    metadata.insert("author".to_string(), "Alice".to_string());
    metadata.insert("version".to_string(), "1.0".to_string());
    metadata.insert("date".to_string(), "2024-01-01".to_string());

    let doc = Document {
        title: "README".to_string(),
        metadata,
    };

    // metadata.@keys ALL OF ["author", "version"]
    let query = DnfQuery::builder()
        .or(|c| {
            c.and(
                "metadata",
                Op::ALL_OF,
                Value::keys(vec!["author", "version"]),
            )
        })
        .build();

    assert!(query.evaluate(&doc));
}

#[test]
fn test_hashmap_keys_any_of() {
    let mut metadata = HashMap::new();
    metadata.insert("author".to_string(), "Alice".to_string());

    let doc = Document {
        title: "README".to_string(),
        metadata,
    };

    // metadata.@keys ANY OF ["author", "editor", "reviewer"]
    let query = DnfQuery::builder()
        .or(|c| {
            c.and(
                "metadata",
                Op::ANY_OF,
                Value::keys(vec!["author", "editor", "reviewer"]),
            )
        })
        .build();

    assert!(query.evaluate(&doc));
}

// ==================== Tests: Values Access ====================

#[test]
fn test_hashmap_values_contains() {
    let mut metadata = HashMap::new();
    metadata.insert("author".to_string(), "Alice".to_string());
    metadata.insert("editor".to_string(), "Bob".to_string());

    let doc = Document {
        title: "README".to_string(),
        metadata,
    };

    // metadata.@values CONTAINS "Alice"
    let query = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::CONTAINS, Value::values("Alice")))
        .build();

    assert!(query.evaluate(&doc));
}

#[test]
fn test_hashmap_values_any_of() {
    let mut metadata = HashMap::new();
    metadata.insert("author".to_string(), "Alice".to_string());

    let doc = Document {
        title: "README".to_string(),
        metadata,
    };

    // metadata.@values ANY OF ["Alice", "Charlie"]
    let query = DnfQuery::builder()
        .or(|c| {
            c.and(
                "metadata",
                Op::ANY_OF,
                Value::values(vec!["Alice", "Charlie"]),
            )
        })
        .build();

    assert!(query.evaluate(&doc));
}

#[test]
fn test_hashmap_values_numeric() {
    let mut settings = HashMap::new();
    settings.insert("timeout".to_string(), 30);
    settings.insert("retries".to_string(), 3);

    let config = Config {
        name: "default".to_string(),
        settings,
    };

    // settings.@values CONTAINS 30
    let query = DnfQuery::builder()
        .or(|c| c.and("settings", Op::CONTAINS, Value::values(30i64)))
        .build();

    assert!(query.evaluate(&config));
}

// ==================== Tests: FieldInfo ====================

#[test]
fn test_hashmap_field_kind() {
    let fields: Vec<_> = Document::fields().collect();

    let title_field = fields.iter().find(|f| f.name == "title").unwrap();
    let metadata_field = fields.iter().find(|f| f.name == "metadata").unwrap();

    assert_eq!(title_field.kind, FieldKind::Scalar);
    assert_eq!(metadata_field.kind, FieldKind::Map);
}

// ==================== Tests: Empty Maps ====================

#[test]
fn test_hashmap_empty_keys_contains() {
    let doc = Document {
        title: "Empty".to_string(),
        metadata: HashMap::new(),
    };

    // metadata.@keys CONTAINS "author" -> false
    let query = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::CONTAINS, Value::keys("author")))
        .build();

    assert!(!query.evaluate(&doc));
}

#[test]
fn test_hashmap_empty_keys_not_contains() {
    let doc = Document {
        title: "Empty".to_string(),
        metadata: HashMap::new(),
    };

    // metadata.@keys NOT CONTAINS "author" -> true
    let query = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::NOT_CONTAINS, Value::keys("author")))
        .build();

    assert!(query.evaluate(&doc));
}

// ==================== Tests: Complex Queries ====================

#[test]
fn test_hashmap_combined_with_scalar_fields() {
    let mut metadata = HashMap::new();
    metadata.insert("author".to_string(), "Alice".to_string());

    let doc = Document {
        title: "README".to_string(),
        metadata,
    };

    // title == "README" AND metadata["author"] == "Alice"
    let query = DnfQuery::builder()
        .or(|c| {
            c.and("title", Op::EQ, "README").and(
                "metadata",
                Op::EQ,
                Value::at_key("author", "Alice"),
            )
        })
        .build();

    assert!(query.evaluate(&doc));
}

#[test]
fn test_hashmap_or_with_keys_and_values() {
    let mut metadata = HashMap::new();
    metadata.insert("author".to_string(), "Alice".to_string());

    let doc = Document {
        title: "README".to_string(),
        metadata,
    };

    // metadata.@keys CONTAINS "author" OR metadata.@values CONTAINS "Bob"
    let query = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::CONTAINS, Value::keys("author")))
        .or(|c| c.and("metadata", Op::CONTAINS, Value::values("Bob")))
        .build();

    assert!(query.evaluate(&doc));
}

// ==================== Tests: Value Display ====================

#[test]
fn test_value_display() {
    let at_key = Value::at_key("author", "Alice");
    let keys = Value::keys("author");
    let values = Value::values("Alice");

    assert!(at_key.to_string().contains("author"));
    assert!(keys.to_string().contains("keys"));
    assert!(values.to_string().contains("values"));
}

// ==================== Tests: Serde ====================

#[cfg(feature = "serde")]
#[test]
fn test_value_serde_roundtrip() {
    let values = vec![
        Value::at_key("author", "Alice"),
        Value::keys("version"),
        Value::values(vec!["v1", "v2"]),
    ];

    for value in values {
        let json = serde_json::to_string(&value).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value, parsed, "Roundtrip failed for: {:?}", value);
    }
}

// ==================== Parser Tests ====================

#[cfg(feature = "parser")]
mod parser_tests {
    use super::*;

    #[test]
    fn test_parse_at_key_equals() {
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), "Alice".to_string());

        let doc = Document {
            title: "README".to_string(),
            metadata,
        };

        // metadata["author"] == "Alice"
        let query = QueryBuilder::from_query::<Document>(r#"metadata["author"] == "Alice""#)
            .expect("should parse");

        assert!(query.evaluate(&doc));
    }

    #[test]
    fn test_parse_at_key_not_equals() {
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), "Alice".to_string());

        let doc = Document {
            title: "README".to_string(),
            metadata,
        };

        // metadata["author"] != "Bob"
        let query = QueryBuilder::from_query::<Document>(r#"metadata["author"] != "Bob""#)
            .expect("should parse");

        assert!(query.evaluate(&doc));
    }

    #[test]
    fn test_parse_keys_contains() {
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), "Alice".to_string());

        let doc = Document {
            title: "README".to_string(),
            metadata,
        };

        // metadata.@keys CONTAINS "author"
        let query = QueryBuilder::from_query::<Document>(r#"metadata.@keys CONTAINS "author""#)
            .expect("should parse");

        assert!(query.evaluate(&doc));
    }

    #[test]
    fn test_parse_keys_not_contains() {
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), "Alice".to_string());

        let doc = Document {
            title: "README".to_string(),
            metadata,
        };

        // metadata.@keys NOT CONTAINS "missing"
        let query =
            QueryBuilder::from_query::<Document>(r#"metadata.@keys NOT CONTAINS "missing""#)
                .expect("should parse");

        assert!(query.evaluate(&doc));
    }

    #[test]
    fn test_parse_values_contains() {
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), "Alice".to_string());

        let doc = Document {
            title: "README".to_string(),
            metadata,
        };

        // metadata.@values CONTAINS "Alice"
        let query = QueryBuilder::from_query::<Document>(r#"metadata.@values CONTAINS "Alice""#)
            .expect("should parse");

        assert!(query.evaluate(&doc));
    }

    #[test]
    fn test_parse_combined_query() {
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), "Alice".to_string());

        let doc = Document {
            title: "README".to_string(),
            metadata,
        };

        // title == "README" AND metadata["author"] == "Alice"
        let query = QueryBuilder::from_query::<Document>(
            r#"title == "README" AND metadata["author"] == "Alice""#,
        )
        .expect("should parse");

        assert!(query.evaluate(&doc));
    }

    #[test]
    fn test_parse_keys_any_of() {
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), "Alice".to_string());

        let doc = Document {
            title: "README".to_string(),
            metadata,
        };

        // metadata.@keys IN ["author", "editor"]
        let query =
            QueryBuilder::from_query::<Document>(r#"metadata.@keys IN ["author", "editor"]"#)
                .expect("should parse");

        assert!(query.evaluate(&doc));
    }

    #[test]
    fn test_parse_error_non_map_with_bracket() {
        // title is not a map field, should error
        let result = QueryBuilder::from_query::<Document>(r#"title["key"] == "value""#);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_error_non_map_with_at_keys() {
        // title is not a map field, should error
        let result = QueryBuilder::from_query::<Document>(r#"title.@keys CONTAINS "x""#);
        assert!(result.is_err());
    }
}

// ==================== Validate Tests ====================

#[test]
fn test_validate_map_field_with_map_value() {
    // Valid: map field with map target value
    let query = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::EQ, Value::at_key("author", "Alice")))
        .build();

    let result = query.validate::<Document>();
    assert!(result.is_ok());
}

#[test]
fn test_validate_map_field_with_keys() {
    let query = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::CONTAINS, Value::keys("author")))
        .build();

    let result = query.validate::<Document>();
    assert!(result.is_ok());
}

#[test]
fn test_validate_map_field_with_values() {
    let query = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::CONTAINS, Value::values("Alice")))
        .build();

    let result = query.validate::<Document>();
    assert!(result.is_ok());
}

#[test]
fn test_validate_non_map_field_with_map_value_fails() {
    // Invalid: non-map field with AtKey value
    let query = DnfQuery::builder()
        .or(|c| c.and("title", Op::EQ, Value::at_key("key", "value")))
        .build();

    let result = query.validate::<Document>();
    assert!(result.is_err());

    let err = result.unwrap_err();
    match err {
        dnf::DnfError::InvalidMapTarget {
            field_name,
            field_kind,
        } => {
            assert_eq!(&*field_name, "title");
            assert_eq!(field_kind, FieldKind::Scalar);
        }
        _ => panic!("Expected InvalidMapTarget error, got {:?}", err),
    }
}

#[test]
fn test_validate_non_map_field_with_keys_fails() {
    let query = DnfQuery::builder()
        .or(|c| c.and("title", Op::CONTAINS, Value::keys("x")))
        .build();

    let result = query.validate::<Document>();
    assert!(result.is_err());
}

#[test]
fn test_validate_scalar_field_ok() {
    // Regular scalar field should validate fine
    let query = DnfQuery::builder()
        .or(|c| c.and("title", Op::EQ, "README"))
        .build();

    let result = query.validate::<Document>();
    assert!(result.is_ok());
}

#[test]
fn test_validate_unknown_field_fails() {
    let query = DnfQuery::builder()
        .or(|c| c.and("unknown_field", Op::EQ, "value"))
        .build();

    let result = query.validate::<Document>();
    assert!(result.is_err());

    let err = result.unwrap_err();
    match err {
        dnf::DnfError::UnknownField { field_name, .. } => {
            assert_eq!(&*field_name, "unknown_field");
        }
        _ => panic!("Expected UnknownField error, got {:?}", err),
    }
}

// ==================== Nested Field Validation Tests ====================

#[derive(DnfEvaluable, Debug)]
struct Address {
    city: String,
    zip: String,
}

#[derive(DnfEvaluable, Debug)]
struct Person {
    name: String,
    #[dnf(nested)]
    address: Address,
}

#[test]
fn test_validate_field_names() {
    // (field, expected_ok, description)
    let test_cases: Vec<(&str, bool, &str)> = vec![
        ("name", true, "valid root field"),
        ("address.city", true, "valid nested field"),
        ("address.zip", true, "valid nested field zip"),
        ("unknown", false, "unknown root field"),
        ("unknown.city", false, "unknown nested root"),
        ("address.unknown", true, "unknown nested leaf (root valid)"),
    ];

    for (field, expected_ok, desc) in test_cases {
        let query = DnfQuery::builder()
            .or(|c| c.and(field, Op::EQ, "test"))
            .build();

        let result = query.validate::<Person>();
        assert_eq!(result.is_ok(), expected_ok, "Failed: {}", desc);
    }
}

#[test]
fn test_validate_custom_ops() {
    // (has_registration, expected_ok, description)
    let test_cases: Vec<(bool, bool, &str)> = vec![
        (true, true, "registered custom op"),
        (false, false, "unregistered custom op"),
    ];

    for (has_registration, expected_ok, desc) in test_cases {
        let mut builder = DnfQuery::builder();

        if has_registration {
            builder = builder.with_custom_op(
                "IS_VALID",
                true,
                |field, _| matches!(field, Value::String(s) if !s.is_empty()),
            );
        }

        let query = builder
            .or(|c| c.and("title", Op::custom("IS_VALID"), Value::None))
            .build();

        let result = query.validate::<Document>();
        assert_eq!(result.is_ok(), expected_ok, "Failed: {}", desc);
    }
}

#[test]
fn test_validate_error_types() {
    // (query_builder, expected_error_variant, description)
    enum ExpectedError {
        UnknownField(&'static str),
        UnregisteredCustomOp(&'static str),
        InvalidMapTarget(&'static str),
    }

    let test_cases: Vec<(DnfQuery, ExpectedError, &str)> = vec![
        (
            DnfQuery::builder()
                .or(|c| c.and("unknown", Op::EQ, "x"))
                .build(),
            ExpectedError::UnknownField("unknown"),
            "unknown field error",
        ),
        (
            DnfQuery::builder()
                .or(|c| c.and("title", Op::custom("MISSING"), Value::None))
                .build(),
            ExpectedError::UnregisteredCustomOp("MISSING"),
            "unregistered custom op error",
        ),
        (
            DnfQuery::builder()
                .or(|c| c.and("title", Op::EQ, Value::at_key("k", "v")))
                .build(),
            ExpectedError::InvalidMapTarget("title"),
            "map target on non-map field",
        ),
    ];

    for (query, expected_error, desc) in test_cases {
        let result = query.validate::<Document>();
        assert!(result.is_err(), "Expected error for: {}", desc);

        let err = result.unwrap_err();
        match (&expected_error, &err) {
            (ExpectedError::UnknownField(name), dnf::DnfError::UnknownField { field_name, .. }) => {
                assert_eq!(&**field_name, *name, "Wrong field name for: {}", desc);
            }
            (
                ExpectedError::UnregisteredCustomOp(name),
                dnf::DnfError::UnregisteredCustomOp { operator_name },
            ) => {
                assert_eq!(&**operator_name, *name, "Wrong op name for: {}", desc);
            }
            (
                ExpectedError::InvalidMapTarget(name),
                dnf::DnfError::InvalidMapTarget { field_name, .. },
            ) => {
                assert_eq!(&**field_name, *name, "Wrong field for: {}", desc);
            }
            _ => panic!("Wrong error type for {}: got {:?}", desc, err),
        }
    }
}

#[test]
fn test_validate_chained_api() {
    // Validate returns Self for chaining
    let query = DnfQuery::builder()
        .or(|c| c.and("title", Op::EQ, "README"))
        .build()
        .validate::<Document>()
        .expect("should validate");

    assert!(query.uses_field("title"));
}

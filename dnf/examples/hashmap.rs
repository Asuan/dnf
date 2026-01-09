//! HashMap field support with builder and parser

#[cfg(feature = "parser")]
use dnf::QueryBuilder;
use dnf::{DnfEvaluable, DnfQuery, FieldKind, Op, Value};
use std::collections::HashMap;

#[derive(DnfEvaluable)]
struct Document {
    title: String,
    #[dnf(rename = "metadata")]
    meta: HashMap<String, String>,
    #[dnf(rename = "scores")]
    game_scores: HashMap<String, i32>,
}

fn main() {
    let doc = Document {
        title: "Getting Started with DNF Queries".to_string(),
        meta: HashMap::from([
            ("author".to_string(), "Alice".to_string()),
            ("version".to_string(), "1.0".to_string()),
            ("status".to_string(), "published".to_string()),
        ]),
        game_scores: HashMap::from([
            ("chess".to_string(), 1500),
            ("go".to_string(), 2100),
            ("poker".to_string(), 800),
        ]),
    };

    // Builder API

    // Query: metadata["author"] == "Alice" (specific key access)
    let q = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::EQ, Value::at_key("author", "Alice")))
        .build();
    assert!(q.evaluate(&doc));

    // Query: scores["chess"] > 1000 (numeric value)
    let q = DnfQuery::builder()
        .or(|c| c.and("scores", Op::GT, Value::at_key("chess", 1000i64)))
        .build();
    assert!(q.evaluate(&doc));

    // Query: metadata.@keys CONTAINS "author" (check key exists)
    let q = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::CONTAINS, Value::keys("author")))
        .build();
    assert!(q.evaluate(&doc));

    // Query: metadata.@keys ALL OF ["author", "version"]
    let q = DnfQuery::builder()
        .or(|c| {
            c.and(
                "metadata",
                Op::ALL_OF,
                Value::keys(vec!["author", "version"]),
            )
        })
        .build();
    assert!(q.evaluate(&doc));

    // Query: metadata.@values CONTAINS "Alice" (check value exists)
    let q = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::CONTAINS, Value::values("Alice")))
        .build();
    assert!(q.evaluate(&doc));

    // Query: metadata.@values ANY OF ["Alice", "Bob"]
    let q = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::ANY_OF, Value::values(vec!["Alice", "Bob"])))
        .build();
    assert!(q.evaluate(&doc));

    // Query: title CONTAINS "DNF" AND metadata["status"] == "published"
    let q = DnfQuery::builder()
        .or(|c| {
            c.and("title", Op::CONTAINS, "DNF").and(
                "metadata",
                Op::EQ,
                Value::at_key("status", "published"),
            )
        })
        .build();
    assert!(q.evaluate(&doc));

    // Parser API (requires "parser" feature)

    #[cfg(feature = "parser")]
    {
        // Parse: metadata["author"] == "Alice"
        let q = QueryBuilder::from_query::<Document>(r#"metadata["author"] == "Alice""#).unwrap();
        assert!(q.evaluate(&doc));

        // Parse: scores["chess"] > 1000
        let q = QueryBuilder::from_query::<Document>(r#"scores["chess"] > 1000"#).unwrap();
        assert!(q.evaluate(&doc));

        // Parse: metadata.@keys CONTAINS "author"
        let q =
            QueryBuilder::from_query::<Document>(r#"metadata.@keys CONTAINS "author""#).unwrap();
        assert!(q.evaluate(&doc));

        // Parse: metadata.@values ALL OF ["Alice", "1.0"]
        let q = QueryBuilder::from_query::<Document>(r#"metadata.@values ALL OF ["Alice", "1.0"]"#)
            .unwrap();
        assert!(q.evaluate(&doc));

        // Parse: metadata["status"] == "published" AND scores["go"] BETWEEN [2000, 2200]
        let q = QueryBuilder::from_query::<Document>(
            r#"metadata["status"] == "published" AND scores["go"] BETWEEN [2000, 2200]"#,
        )
        .unwrap();
        assert!(q.evaluate(&doc));
    }

    // Field introspection
    for field in Document::fields() {
        let kind = match field.kind {
            FieldKind::Scalar => "Scalar",
            FieldKind::Iter => "Iter",
            FieldKind::Map => "Map",
        };
        println!("{}: {} ({})", field.name, field.field_type, kind);
    }
}

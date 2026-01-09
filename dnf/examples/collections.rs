//! Vec, HashSet, and array operators

use dnf::{DnfEvaluable, DnfQuery, Op};
use std::collections::HashSet;

#[derive(DnfEvaluable)]
struct Product {
    id: u64,
    name: String,
    tags: Vec<String>,
    ratings: Vec<i64>,
    scores: Vec<f64>,
    category_ids: HashSet<u32>,
    feature_flags: HashSet<String>,
}

fn main() {
    let product = Product {
        id: 1,
        name: "Smart Watch".to_string(),
        tags: vec![
            "electronics".to_string(),
            "wearable".to_string(),
            "fitness".to_string(),
        ],
        ratings: vec![5, 4, 5, 3, 5],
        scores: vec![4.5, 4.8, 5.0],
        category_ids: HashSet::from([10, 20, 30]),
        feature_flags: HashSet::from(["premium".to_string(), "verified".to_string()]),
    };

    // Query: tags CONTAINS "electronics" (Vec<String>)
    let q = DnfQuery::builder()
        .or(|c| c.and("tags", Op::CONTAINS, "electronics"))
        .build();
    assert!(q.evaluate(&product));

    // Query: tags NOT CONTAINS "python"
    let q = DnfQuery::builder()
        .or(|c| c.and("tags", Op::NOT_CONTAINS, "python"))
        .build();
    assert!(q.evaluate(&product));

    // Query: scores CONTAINS 4.8 (Vec<f64>)
    let q = DnfQuery::builder()
        .or(|c| c.and("scores", Op::CONTAINS, 4.8))
        .build();
    assert!(q.evaluate(&product));

    // Query: tags STARTS WITH "electronics" (first element)
    let q = DnfQuery::builder()
        .or(|c| c.and("tags", Op::STARTS_WITH, "electronics"))
        .build();
    assert!(q.evaluate(&product));

    // Query: tags ENDS WITH "fitness" (last element)
    let q = DnfQuery::builder()
        .or(|c| c.and("tags", Op::ENDS_WITH, "fitness"))
        .build();
    assert!(q.evaluate(&product));

    // Query: tags ANY OF ["wearable", "outdoor"]
    let q = DnfQuery::builder()
        .or(|c| c.and("tags", Op::ANY_OF, vec!["wearable", "outdoor"]))
        .build();
    assert!(q.evaluate(&product));

    // Query: tags ALL OF ["electronics", "wearable"]
    let q = DnfQuery::builder()
        .or(|c| c.and("tags", Op::ALL_OF, vec!["electronics", "wearable"]))
        .build();
    assert!(q.evaluate(&product));

    // Query: category_ids CONTAINS 20 (HashSet<u32>)
    let q = DnfQuery::builder()
        .or(|c| c.and("category_ids", Op::CONTAINS, 20))
        .build();
    assert!(q.evaluate(&product));

    // Query: feature_flags ALL OF ["premium", "verified"] (HashSet<String>)
    let q = DnfQuery::builder()
        .or(|c| c.and("feature_flags", Op::ALL_OF, vec!["premium", "verified"]))
        .build();
    assert!(q.evaluate(&product));

    // Query: tags STARTS WITH "electronics" AND ratings ENDS WITH 5 AND name CONTAINS "Watch"
    let q = DnfQuery::builder()
        .or(|c| {
            c.and("tags", Op::STARTS_WITH, "electronics")
                .and("ratings", Op::ENDS_WITH, 5)
                .and("name", Op::CONTAINS, "Watch")
        })
        .build();
    assert!(q.evaluate(&product));
}

//! Serialize DNF queries to JSON and String formats
//! cargo run --example serialization --features serde,parser

use dnf::{DnfEvaluable, DnfQuery, Op};

#[derive(DnfEvaluable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct User {
    name: String,
    age: u32,
    premium: bool,
    country: String,
    tags: Vec<String>,
    score: f64,
}

fn main() {
    // Build a complex query:
    // (age >= 21 AND premium == true AND country == "US")
    // OR (age >= 18 AND tags CONTAINS "vip")
    // OR (name STARTS WITH "Admin")
    // OR (score BETWEEN [60.0, 100.0])
    let query = DnfQuery::builder()
        .or(|c| {
            c.and("age", Op::GTE, 21)
                .and("premium", Op::EQ, true)
                .and("country", Op::EQ, "US")
        })
        .or(|c| c.and("age", Op::GTE, 18).and("tags", Op::CONTAINS, "vip"))
        .or(|c| c.and("name", Op::STARTS_WITH, "Admin"))
        .or(|c| c.and("score", Op::BETWEEN, vec![60.0, 100.0]))
        .build();

    println!("=== Original Query ===\n");

    // 1. String representation (human-readable)
    let query_string = query.to_string();
    println!("String format:\n{}\n", query_string);

    // 2. JSON representation (machine-readable, portable)
    #[cfg(feature = "serde")]
    {
        let query_json = serde_json::to_string_pretty(&query).unwrap();
        println!("JSON format:\n{}\n", query_json);

        // Compact JSON (for storage/transmission)
        let query_json_compact = serde_json::to_string(&query).unwrap();
        println!("JSON compact:\n{}\n", query_json_compact);

        println!("=== Round-trip Deserialization ===\n");

        // Deserialize from JSON
        let deserialized: DnfQuery = serde_json::from_str(&query_json).unwrap();
        println!("Deserialized from JSON: {}\n", deserialized);

        // Verify they're equal
        assert_eq!(query, deserialized);
        println!("✓ Original and deserialized queries are equal\n");
    }

    // Parse from string
    #[cfg(feature = "parser")]
    {
        println!("=== Round-trip from String ===\n");
        let parsed = dnf::QueryBuilder::from_query::<User>(&query_string).unwrap();
        println!("Parsed from string: {}\n", parsed);
        assert_eq!(query, parsed);
        println!("✓ Original and parsed queries are equal\n");
    }

    println!("=== Testing Evaluation ===\n");

    let user1 = User {
        name: "Alice".to_string(),
        age: 25,
        premium: true,
        country: "US".to_string(),
        tags: vec![],
        score: 45.5,
    };

    let user2 = User {
        name: "Bob".to_string(),
        age: 20,
        premium: false,
        country: "UK".to_string(),
        tags: vec!["vip".to_string()],
        score: 55.0,
    };

    let user3 = User {
        name: "AdminUser".to_string(),
        age: 17,
        premium: false,
        country: "CA".to_string(),
        tags: vec![],
        score: 30.0,
    };

    let user4 = User {
        name: "Charlie".to_string(),
        age: 19,
        premium: false,
        country: "FR".to_string(),
        tags: vec![],
        score: 75.0,
    };

    println!(
        "User1 (age=25, premium=true, country=US, score=45.5): {}",
        query.evaluate(&user1)
    );
    println!(
        "User2 (age=20, tags=[vip], score=55.0): {}",
        query.evaluate(&user2)
    );
    println!(
        "User3 (name=AdminUser, score=30.0): {}",
        query.evaluate(&user3)
    );
    println!(
        "User4 (age=19, score=75.0 in [60,100]): {}",
        query.evaluate(&user4)
    );
}

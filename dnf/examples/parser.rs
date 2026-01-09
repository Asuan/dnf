//! Parse DNF queries from strings (requires "parser" feature)
//! cargo run --example parser --features parser

use dnf::{DnfEvaluable, DnfQuery, Op, QueryBuilder};

#[derive(DnfEvaluable)]
struct User {
    name: String,
    age: u32,
    premium: bool,
}

fn main() {
    let user = User {
        name: "Alice".to_string(),
        age: 25,
        premium: true,
    };

    // Parse: age > 18 AND premium == true
    let q = QueryBuilder::from_query::<User>(r#"age > 18 AND premium == true"#).unwrap();
    assert!(q.evaluate(&user));

    // Parse: (age >= 21 AND premium == true) OR name STARTS WITH "VIP"
    let q = QueryBuilder::from_query::<User>(
        r#"(age >= 21 AND premium == true) OR (name STARTS WITH "VIP")"#,
    )
    .unwrap();
    assert!(q.evaluate(&user));

    // Parse: name CONTAINS "lic"
    let q = QueryBuilder::from_query::<User>(r#"name CONTAINS "lic""#).unwrap();
    assert!(q.evaluate(&user));

    // Parse: name NOT ENDS WITH "xyz"
    let q = QueryBuilder::from_query::<User>(r#"name NOT ENDS WITH "xyz""#).unwrap();
    assert!(q.evaluate(&user));

    // Build -> Display -> Parse round-trip
    let original = DnfQuery::builder()
        .or(|c| c.and("age", Op::GT, 18).and("premium", Op::EQ, true))
        .build();
    let reparsed = QueryBuilder::from_query::<User>(&original.to_string()).unwrap();
    assert_eq!(original, reparsed);

    // Parse errors
    if let Err(e) = QueryBuilder::from_query::<User>("unknown_field > 18") {
        println!("Error: {}", e)
    }

    if let Err(e) = QueryBuilder::from_query::<User>(r#"age > "not_a_number""#) {
        println!("Error: {}", e)
    }
}

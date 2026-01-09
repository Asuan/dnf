//! #[derive(DnfEvaluable)] with rename, skip, nested, Option support

use dnf::{DnfEvaluable, DnfQuery, Op};

#[derive(DnfEvaluable)]
struct User {
    name: String,
    age: u32,
    premium: bool,
}

#[derive(DnfEvaluable)]
struct Product {
    #[dnf(rename = "product_name")]
    name: String,
    #[dnf(skip)]
    #[allow(dead_code)]
    internal_id: u64,
    price: f64,
}

#[derive(DnfEvaluable)]
struct Profile {
    username: String,
    email: Option<String>,
    age: Option<u32>,
}

#[derive(DnfEvaluable)]
struct Address {
    city: String,
    zip: String,
}

#[derive(DnfEvaluable)]
struct Employee {
    name: String,
    #[dnf(nested)]
    address: Address,
}

fn main() {
    let user = User {
        name: "Alice".to_string(),
        age: 25,
        premium: true,
    };

    // Query: age > 18 AND premium == true
    let q = DnfQuery::builder()
        .or(|c| c.and("age", Op::GT, 18).and("premium", Op::EQ, true))
        .build();
    assert!(q.evaluate(&user));

    let product = Product {
        name: "Widget".to_string(),
        internal_id: 12345,
        price: 29.99,
    };

    // Query: product_name == "Widget" (using renamed field)
    let q = DnfQuery::builder()
        .or(|c| c.and("product_name", Op::EQ, "Widget"))
        .build();
    assert!(q.evaluate(&product));

    let profile = Profile {
        username: "alice".to_string(),
        email: Some("alice@example.com".to_string()),
        age: Some(25),
    };

    // Query: email CONTAINS "example" (Option field)
    let q = DnfQuery::builder()
        .or(|c| c.and("email", Op::CONTAINS, "example"))
        .build();
    assert!(q.evaluate(&profile));

    let employee = Employee {
        name: "Jane".to_string(),
        address: Address {
            city: "Boston".to_string(),
            zip: "02101".to_string(),
        },
    };

    // Query: address.city == "Boston" (nested field)
    let q = DnfQuery::builder()
        .or(|c| c.and("address.city", Op::EQ, "Boston"))
        .build();
    assert!(q.evaluate(&employee));

    // Query: (age >= 21 AND premium == true) OR name STARTS WITH "VIP"
    let q = DnfQuery::builder()
        .or(|c| c.and("age", Op::GTE, 21).and("premium", Op::EQ, true))
        .or(|c| c.and("name", Op::STARTS_WITH, "VIP"))
        .build();
    assert!(q.evaluate(&user));
}

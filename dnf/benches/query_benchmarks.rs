use criterion::{criterion_group, criterion_main, Criterion};
use dnf::{DnfEvaluable, DnfQuery, Op};
use std::collections::HashSet;
use std::hint::black_box;

// Nested struct for benchmarks
#[derive(DnfEvaluable)]
struct Address {
    city: String,
    country: String,
}

// Test struct for benchmarks
#[derive(DnfEvaluable)]
struct User {
    age: u32,
    name: String,
    email: String,
    country: String,
    premium: bool,
    verified: bool,
    score: i32,
    tags: Vec<String>,
    skill_scores: Vec<i32>,
    categories: HashSet<String>,
    badge_ids: HashSet<i32>,
    #[dnf(nested)]
    address: Address,
}

impl User {
    fn sample() -> Self {
        User {
            age: 25,
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
            country: "US".to_string(),
            premium: true,
            verified: true,
            score: 85,
            tags: vec![
                "developer".to_string(),
                "rust".to_string(),
                "backend".to_string(),
            ],
            skill_scores: vec![85, 90, 78, 95],
            categories: ["engineering", "backend", "systems"]
                .into_iter()
                .map(String::from)
                .collect(),
            badge_ids: [101, 102, 103, 104, 105].into_iter().collect(),
            address: Address {
                city: "San Francisco".to_string(),
                country: "US".to_string(),
            },
        }
    }
}

fn bench_query_evaluation(c: &mut Criterion) {
    let user = User::sample();

    // Simple: single condition
    let simple_query = DnfQuery::builder()
        .or(|conj| conj.and("age", Op::GT, 18))
        .build();

    // Complex: multiple conditions with OR
    let complex_query = DnfQuery::builder()
        .or(|conj| {
            conj.and("age", Op::GT, 18)
                .and("country", Op::EQ, "US")
                .and("premium", Op::EQ, true)
        })
        .or(|conj| conj.and("verified", Op::EQ, true).and("score", Op::GTE, 80))
        .build();

    // String operations
    let string_query = DnfQuery::builder()
        .or(|conj| conj.and("name", Op::CONTAINS, "John"))
        .build();

    // Nested field access
    let nested_query = DnfQuery::builder()
        .or(|conj| conj.and("address.city", Op::EQ, "San Francisco"))
        .build();

    // Vec ANY OF
    let vec_any_of = DnfQuery::builder()
        .or(|conj| conj.and("tags", Op::ANY_OF, vec!["rust", "python"]))
        .build();

    // Vec ALL OF
    let vec_all_of = DnfQuery::builder()
        .or(|conj| conj.and("tags", Op::ALL_OF, vec!["developer", "rust"]))
        .build();

    // HashSet ANY OF
    let hashset_any_of = DnfQuery::builder()
        .or(|conj| conj.and("categories", Op::ANY_OF, vec!["backend", "frontend"]))
        .build();

    // HashSet ALL OF
    let hashset_all_of = DnfQuery::builder()
        .or(|conj| conj.and("categories", Op::ALL_OF, vec!["engineering", "backend"]))
        .build();

    c.bench_function("eval_simple", |b| {
        b.iter(|| black_box(&simple_query).evaluate(black_box(&user)))
    });

    c.bench_function("eval_complex", |b| {
        b.iter(|| black_box(&complex_query).evaluate(black_box(&user)))
    });

    c.bench_function("eval_string_contains", |b| {
        b.iter(|| black_box(&string_query).evaluate(black_box(&user)))
    });

    c.bench_function("eval_nested_field", |b| {
        b.iter(|| black_box(&nested_query).evaluate(black_box(&user)))
    });

    c.bench_function("eval_vec_any_of", |b| {
        b.iter(|| black_box(&vec_any_of).evaluate(black_box(&user)))
    });

    c.bench_function("eval_vec_all_of", |b| {
        b.iter(|| black_box(&vec_all_of).evaluate(black_box(&user)))
    });

    c.bench_function("eval_hashset_any_of", |b| {
        b.iter(|| black_box(&hashset_any_of).evaluate(black_box(&user)))
    });

    c.bench_function("eval_hashset_all_of", |b| {
        b.iter(|| black_box(&hashset_all_of).evaluate(black_box(&user)))
    });
}

fn bench_batch_evaluation(c: &mut Criterion) {
    let query = DnfQuery::builder()
        .or(|conj| conj.and("age", Op::GT, 18).and("country", Op::EQ, "US"))
        .build();

    let users: Vec<User> = (0..100)
        .map(|i| User {
            age: 20 + (i % 50),
            name: format!("User{}", i),
            email: format!("user{}@example.com", i),
            country: if i % 2 == 0 { "US" } else { "UK" }.to_string(),
            premium: i % 3 == 0,
            verified: i % 2 == 0,
            score: 50 + (i % 50) as i32,
            tags: vec![format!("tag{}", i % 5), "developer".to_string()],
            skill_scores: vec![50 + (i % 30) as i32, 60 + (i % 40) as i32],
            categories: [format!("cat{}", i % 3), "engineering".to_string()]
                .into_iter()
                .collect(),
            badge_ids: [100 + (i % 10) as i32, 200 + (i % 5) as i32]
                .into_iter()
                .collect(),
            address: Address {
                city: format!("City{}", i % 10),
                country: if i % 2 == 0 { "US" } else { "UK" }.to_string(),
            },
        })
        .collect();

    c.bench_function("eval_batch_100_users", |b| {
        b.iter(|| users.iter().filter(|u| query.evaluate(*u)).count())
    });
}

criterion_group!(benches, bench_query_evaluation, bench_batch_evaluation);
criterion_main!(benches);

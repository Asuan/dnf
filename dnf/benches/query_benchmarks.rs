use criterion::{criterion_group, criterion_main, Criterion};
#[cfg(feature = "parser")]
use dnf::QueryBuilder;
use dnf::{DnfEvaluable, DnfQuery, Op, Value};
use std::collections::{HashMap, HashSet};
use std::hint::black_box;

#[derive(DnfEvaluable)]
struct Address {
    city: String,
    country: String,
}

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
    #[dnf(rename = "metadata")]
    meta: HashMap<String, String>,
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
            meta: HashMap::from([
                ("author".to_string(), "Alice".to_string()),
                ("status".to_string(), "published".to_string()),
                ("version".to_string(), "1.0".to_string()),
            ]),
            address: Address {
                city: "San Francisco".to_string(),
                country: "US".to_string(),
            },
        }
    }
}

fn bench_query_evaluation(c: &mut Criterion) {
    let user = User::sample();

    let simple_query = DnfQuery::builder()
        .or(|conj| conj.and("age", Op::GT, 18))
        .build();

    let string_query = DnfQuery::builder()
        .or(|conj| conj.and("name", Op::CONTAINS, "John"))
        .build();

    let nested_query = DnfQuery::builder()
        .or(|conj| conj.and("address.city", Op::EQ, "San Francisco"))
        .build();

    let vec_any_of = DnfQuery::builder()
        .or(|conj| conj.and("tags", Op::ANY_OF, vec!["rust", "python"]))
        .build();

    let vec_all_of = DnfQuery::builder()
        .or(|conj| conj.and("tags", Op::ALL_OF, vec!["developer", "rust"]))
        .build();

    let hashset_any_of = DnfQuery::builder()
        .or(|conj| conj.and("categories", Op::ANY_OF, vec!["backend", "frontend"]))
        .build();

    let hashset_all_of = DnfQuery::builder()
        .or(|conj| conj.and("categories", Op::ALL_OF, vec!["engineering", "backend"]))
        .build();

    c.bench_function("eval_simple", |b| {
        b.iter(|| black_box(&simple_query).evaluate(black_box(&user)))
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

fn bench_custom_op(c: &mut Criterion) {
    let user = User::sample();

    // Baseline: same shape but native operator, no custom-op machinery.
    let baseline = DnfQuery::builder()
        .or(|c| c.and("name", Op::CONTAINS, "1"))
        .build();

    // Custom op registered AND used: exercises field_value() + registry lookup
    // + indirect call. The op operates on the String `name` field, so the
    // hot path includes a `Box<str>` allocation in `field_value`.
    let registered = DnfQuery::builder()
        .with_custom_op(
            "HAS_DIGIT",
            true,
            |field, _| matches!(field, Value::String(s) if s.chars().any(|c| c.is_ascii_digit())),
        )
        .or(|c| c.and("name", Op::custom("HAS_DIGIT"), Value::None))
        .build();

    // Registry attached but the requested op is missing: field_value() still
    // gets called (and allocated) but the registry returns None.
    let unregistered = DnfQuery::builder()
        .with_custom_op("OTHER_OP", true, |_, _| true)
        .or(|c| c.and("name", Op::custom("MISSING_OP"), Value::None))
        .build();

    c.bench_function("eval_custom_op_baseline", |b| {
        b.iter(|| black_box(&baseline).evaluate(black_box(&user)))
    });

    c.bench_function("eval_custom_op_registered", |b| {
        b.iter(|| black_box(&registered).evaluate(black_box(&user)))
    });

    c.bench_function("eval_custom_op_unregistered", |b| {
        b.iter(|| black_box(&unregistered).evaluate(black_box(&user)))
    });
}

fn bench_map_fields(c: &mut Criterion) {
    let user = User::sample();

    let at_key = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::EQ, Value::at_key("author", "Alice")))
        .build();

    let keys_contains = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::CONTAINS, Value::keys("author")))
        .build();

    let values_any_of = DnfQuery::builder()
        .or(|c| c.and("metadata", Op::ANY_OF, Value::values(vec!["Alice", "Bob"])))
        .build();

    c.bench_function("eval_map_at_key", |b| {
        b.iter(|| black_box(&at_key).evaluate(black_box(&user)))
    });

    c.bench_function("eval_map_keys_contains", |b| {
        b.iter(|| black_box(&keys_contains).evaluate(black_box(&user)))
    });

    c.bench_function("eval_map_values_any_of", |b| {
        b.iter(|| black_box(&values_any_of).evaluate(black_box(&user)))
    });
}

#[cfg(feature = "parser")]
fn bench_parser(c: &mut Criterion) {
    let simple = "age > 18";
    let complex = r#"(age > 18 AND country == "US" AND name CONTAINS "John") OR (premium == true AND verified == true AND score >= 80) OR tags IN ["rust", "python"] OR badge_ids NOT IN [999, 1000]"#;

    c.bench_function("parse_simple", |b| {
        b.iter(|| QueryBuilder::from_query::<User>(black_box(simple)).unwrap())
    });

    c.bench_function("parse_complex", |b| {
        b.iter(|| QueryBuilder::from_query::<User>(black_box(complex)).unwrap())
    });
}

#[cfg(feature = "parser")]
criterion_group!(
    benches,
    bench_query_evaluation,
    bench_custom_op,
    bench_map_fields,
    bench_parser
);

#[cfg(not(feature = "parser"))]
criterion_group!(
    benches,
    bench_query_evaluation,
    bench_custom_op,
    bench_map_fields
);

criterion_main!(benches);

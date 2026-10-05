//! Custom operators with builder and parser

use dnf::{DnfEvaluable, DnfQuery, Op, OpRegistry, Value};

#[derive(DnfEvaluable)]
struct Student {
    name: String,
    age: u32,
    salary: f64,
    score: i32,
}

fn main() {
    let student = Student {
        name: "Alice".to_string(),
        age: 25,
        salary: 85000.0,
        score: 85,
    };

    // Built-in BETWEEN operator

    // Query: age BETWEEN [18, 65]
    let q = DnfQuery::builder()
        .or(|c| c.and("age", Op::BETWEEN, vec![18, 65]))
        .build();
    assert!(q.evaluate(&student));

    // Query: salary BETWEEN [50000.0, 150000.0]
    let q = DnfQuery::builder()
        .or(|c| c.and("salary", Op::BETWEEN, vec![50000.0, 150000.0]))
        .build();
    assert!(q.evaluate(&student));

    // Query: salary NOT BETWEEN [90000.0, 100000.0]
    let q = DnfQuery::builder()
        .or(|c| c.and("salary", Op::NOT_BETWEEN, vec![90000.0, 100000.0]))
        .build();
    assert!(q.evaluate(&student));

    // Inline custom operators

    // Query: age IS_ADULT (inline custom operator)
    let q = DnfQuery::builder()
        .with_custom_op(
            "IS_ADULT",
            false,
            |field, _| matches!(field, Value::Uint(n) if *n >= 18),
        )
        .or(|c| c.and("age", Op::custom("IS_ADULT"), Value::None))
        .build();
    assert!(q.evaluate(&student));

    // Query: salary NOT IS_LOW (negated custom operator)
    let q = DnfQuery::builder()
        .with_custom_op(
            "IS_LOW",
            false,
            |field, _| matches!(field, Value::Float(n) if *n < 60000.0),
        )
        .or(|c| c.and("salary", Op::not_custom("IS_LOW"), Value::None))
        .build();
    assert!(q.evaluate(&student));

    // OpRegistry for reusable operators

    let mut registry = OpRegistry::new();
    registry.register(
        "IS_ADULT",
        true,
        |field, _| matches!(field, Value::Uint(n) if *n >= 18),
    );
    registry.register(
        "IS_PASSING",
        true,
        |field, _| matches!(field, Value::Int(n) if *n >= 60),
    );

    // Query: age IS_ADULT AND score IS_PASSING (using registry)
    let q = DnfQuery::builder()
        .with_custom_ops(registry)
        .or(|c| {
            c.and("age", Op::custom("IS_ADULT"), Value::None).and(
                "score",
                Op::custom("IS_PASSING"),
                Value::None,
            )
        })
        .build();
    assert!(q.evaluate(&student));

    // Query: name STARTS WITH "Ali" AND age IS_ADULT AND salary >= 80000.0 (mixed standard + custom)
    let q = DnfQuery::builder()
        .with_custom_op(
            "IS_ADULT",
            false,
            |field, _| matches!(field, Value::Uint(n) if *n >= 18),
        )
        .or(|c| {
            c.and("name", Op::STARTS_WITH, "Ali")
                .and("age", Op::custom("IS_ADULT"), Value::None)
                .and("salary", Op::GTE, 80000.0)
        })
        .build();
    assert!(q.evaluate(&student));

    // Parser API (requires "parser" feature)

    #[cfg(feature = "parser")]
    {
        // Parse: age BETWEEN [25, 65]
        let q = DnfQuery::builder()
            .parse::<Student>(r#"age BETWEEN [25, 65]"#)
            .unwrap()
            .build();
        assert!(q.evaluate(&student));

        // Parse: salary BETWEEN [50000.0, 150000.0]
        let q = DnfQuery::builder()
            .parse::<Student>(r#"salary BETWEEN [50000.0, 150000.0]"#)
            .unwrap()
            .build();
        assert!(q.evaluate(&student));

        // Parse: age IS_ADULT (novalue custom operator)
        let q = DnfQuery::builder()
            .with_custom_op(
                "IS_ADULT",
                true,
                |field, _| matches!(field, Value::Uint(n) if *n >= 18),
            )
            .parse::<Student>("age IS_ADULT")
            .unwrap()
            .build();
        assert!(q.evaluate(&student));

        // Parse: age IS_ADULT AND score HIGH_SCORE (multiple novalue operators)
        let q = DnfQuery::builder()
            .with_custom_op(
                "IS_ADULT",
                true,
                |field, _| matches!(field, Value::Uint(n) if *n >= 18),
            )
            .with_custom_op(
                "HIGH_SCORE",
                true,
                |field, _| matches!(field, Value::Int(n) if *n >= 80),
            )
            .parse::<Student>("age IS_ADULT AND score HIGH_SCORE")
            .unwrap()
            .build();
        assert!(q.evaluate(&student));

        // Parse: score MIN_SCORE 70 (custom operator with a scalar value parameter)
        let min_score = |field: &Value, query_value: &Value| {
            let (Value::Int(n), Value::Uint(threshold)) = (field, query_value) else {
                return false;
            };
            *n >= *threshold as i64
        };
        let q = DnfQuery::builder()
            .with_custom_op("MIN_SCORE", false, min_score)
            .parse::<Student>("score MIN_SCORE 70")
            .unwrap()
            .build();
        assert!(q.evaluate(&student)); // 85 >= 70

        // Parse: score NOT MIN_SCORE 90 (negated custom operator from text)
        let q = DnfQuery::builder()
            .with_custom_op("MIN_SCORE", false, min_score)
            .parse::<Student>("score NOT MIN_SCORE 90")
            .unwrap()
            .build();
        assert!(q.evaluate(&student)); // NOT (85 >= 90)
    }
}

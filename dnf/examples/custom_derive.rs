//! Manual DnfEvaluable for computed fields and custom types via DnfField

use dnf::{DnfEvaluable, DnfField, DnfQuery, FieldInfo, Op, Value};

// Manual DnfEvaluable for computed fields
struct Article {
    title: String,
    views: u64,
    draft: bool,
}

impl DnfEvaluable for Article {
    fn evaluate_field(&self, field: &str, op: &Op, value: &Value) -> bool {
        match field {
            "title" => self.title.evaluate(op, value),
            "views" => self.views.evaluate(op, value),
            "published" => (!self.draft).evaluate(op, value),
            "popular" => (self.views > 1000).evaluate(op, value),
            _ => false,
        }
    }

    fn fields() -> impl Iterator<Item = FieldInfo> {
        [
            FieldInfo::new("title", "String"),
            FieldInfo::new("views", "u64"),
            FieldInfo::new("published", "bool"),
            FieldInfo::new("popular", "bool"),
        ]
        .into_iter()
    }
}

// Custom types via DnfField trait
#[derive(Clone, Copy)]
struct Score(u32);

impl DnfField for Score {
    fn evaluate(&self, op: &Op, value: &Value) -> bool {
        (self.0 as i64).evaluate(op, value)
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
enum Status {
    Active,
    Inactive,
}

impl DnfField for Status {
    fn evaluate(&self, op: &Op, value: &Value) -> bool {
        let s = match self {
            Status::Active => "active",
            Status::Inactive => "inactive",
        };
        s.evaluate(op, value)
    }
}

#[derive(DnfEvaluable)]
struct Player {
    name: String,
    score: Score,
    status: Status,
}

fn main() {
    let article = Article {
        title: "Rust Guide".into(),
        views: 5000,
        draft: false,
    };

    // Query: published == true (computed field: !draft)
    let q = DnfQuery::builder()
        .or(|c| c.and("published", Op::EQ, true))
        .build();
    assert!(q.evaluate(&article));

    // Query: popular == true (computed field: views > 1000)
    let q = DnfQuery::builder()
        .or(|c| c.and("popular", Op::EQ, true))
        .build();
    assert!(q.evaluate(&article));

    let player = Player {
        name: "Alice".into(),
        score: Score(1500),
        status: Status::Active,
    };

    // Query: score >= 1000 (custom Score type)
    let q = DnfQuery::builder()
        .or(|c| c.and("score", Op::GTE, 1000))
        .build();
    assert!(q.evaluate(&player));

    // Query: status == "active" (custom Status enum)
    let q = DnfQuery::builder()
        .or(|c| c.and("status", Op::EQ, "active"))
        .build();
    assert!(q.evaluate(&player));
}

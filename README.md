# DNF

A Rust library for building and evaluating [DNF (Disjunctive Normal Form)](https://en.wikipedia.org/wiki/Disjunctive_normal_form) queries — basically OR-ed ANDs.

```
(age > 18 AND country == "US") OR (premium == true)
```

## Why DNF?

- **Type-safe**: derive macros generate the boilerplate
- **Fast**: zero-copy evaluation, minimal allocations
- **Flexible**: rich operators, custom logic, nested structs
- **Portable**: serialize queries with serde, parse from strings

## Quick Start

```rust
use dnf::{DnfEvaluable, DnfQuery, Op};

#[derive(DnfEvaluable)]
struct User { name: String, age: u32, premium: bool }

let query = DnfQuery::builder()
    .or(|c| c.and("age", Op::GTE, 18))
    .or(|c| c.and("premium", Op::EQ, true))
    .build();

let user = User { name: "Alice".into(), age: 25, premium: false };
assert!(query.evaluate(&user));
```

## Validation

The builder API doesn't check field names at compile time. Use `validate()` to catch typos:

```rust
let query = DnfQuery::builder()
    .or(|c| c.and("agee", Op::GT, 18))  // typo!
    .validate::<User>()?                 // catches it here
    .build();
```

Or use the parser — it validates automatically:

```rust
let query = DnfQuery::parse::<User>("age > 18")?;
```

## Features

```toml
[dependencies]
dnf = "0.1"                                       # derive only
dnf = { version = "0.1", features = ["serde"] }   # + serialization
dnf = { version = "0.1", features = ["parser"] }  # + string parsing
```

| Feature | What it does |
|---------|--------------|
| `derive` | `#[derive(DnfEvaluable)]` macro (default) |
| `serde` | Serialization support |
| `parser` | Parse queries from strings |

## Supported Types

| Category | Types |
|----------|-------|
| Integers | `i8`–`i64`, `isize`, `u8`–`u64`, `usize` |
| Floats | `f32`, `f64` |
| Strings | `String`, `&str`, `Box<str>`, `Cow<str>` |
| Other | `bool` |
| Collections | `Vec<T>`, `HashSet<T>` |
| Maps | `HashMap<String, V>`, `BTreeMap<String, V>` |
| Wrappers | `Option<T>` |
| Nested | Structs with `#[dnf(nested)]` |

## Operators

| Category | Operators |
|----------|-----------|
| Comparison | `==` `!=` `>` `<` `>=` `<=` |
| String | `CONTAINS` `STARTS WITH` `ENDS WITH` (+ NOT variants) |
| Collection | `ANY OF` `ALL OF` (+ NOT variants) |
| Range | `BETWEEN [min, max]` |
| Custom | `Op::custom("NAME")` |

## Collections & Range

```rust
use dnf::{DnfEvaluable, DnfQuery, Op, Value};

#[derive(DnfEvaluable)]
struct Product { tags: Vec<String>, score: f64 }

let q = DnfQuery::builder()
    .or(|c| c.and("tags", Op::CONTAINS, "rust"))
    .or(|c| c.and("score", Op::BETWEEN, vec![60.0, 100.0]))
    .build();
```

## Nested Structs

```rust
#[derive(DnfEvaluable)]
struct Address { city: String }

#[derive(DnfEvaluable)]
struct User {
    #[dnf(nested)]        // required for single struct
    address: Address,
    offices: Vec<Address>, // auto-detected
}

// Query: "address.city" or "offices.city"
```

## Custom Operators

```rust
let q = DnfQuery::builder()
    .with_custom_op("IS_ADULT", true, |field, _| {
        matches!(field, Value::Uint(n) if *n >= 18)
    })
    .or(|c| c.and("age", Op::custom("IS_ADULT"), Value::None))
    .build();
```

## Use Cases

- **Search filters**: let users build complex queries
- **Access control**: evaluate permission rules
- **Rule engines**: serialize business logic
- **Feature flags**: target users with complex conditions

## Examples

See dnf/examples/*

## License

MIT OR Apache-2.0

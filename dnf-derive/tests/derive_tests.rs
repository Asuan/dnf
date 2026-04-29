use dnf::{DnfEvaluable, DnfQuery, Op, Value};
use std::collections::HashSet;

// Test helper for single field conditions
fn test_field_condition<T: DnfEvaluable>(data: &T, field: &str, op: Op, value: Value) -> bool {
    let query = DnfQuery::builder()
        .or(|c| c.and(field.to_string(), op, value))
        .build();
    query.evaluate(data)
}

#[derive(DnfEvaluable)]
struct User {
    age: u32,
    name: String,
    active: bool,
    score: f64,
}

#[test]
fn test_derive_basic_fields() {
    let user = User {
        age: 25,
        name: "Alice".to_string(),
        active: true,
        score: 95.5,
    };

    // Test all basic fields with data-driven approach
    let test_cases = vec![
        ("age", Op::GT, Value::from(18), true),
        ("name", Op::EQ, Value::from("Alice"), true),
        ("active", Op::EQ, Value::from(true), true),
        ("score", Op::GTE, Value::from(90.0), true),
    ];

    for (field, op, value, expected) in test_cases {
        assert_eq!(
            test_field_condition(&user, field, op, value),
            expected,
            "Failed for field: {}",
            field
        );
    }
}

#[test]
fn test_derive_string_operations() {
    let user = User {
        age: 25,
        name: "Alice Johnson".to_string(),
        active: true,
        score: 95.5,
    };

    // Test string operations
    assert!(test_field_condition(
        &user,
        "name",
        Op::CONTAINS,
        Value::from("Johnson")
    ));
    assert!(test_field_condition(
        &user,
        "name",
        Op::STARTS_WITH,
        Value::from("Alice")
    ));
}

#[test]
fn test_derive_complex_query() {
    let user = User {
        age: 25,
        name: "Alice".to_string(),
        active: false,
        score: 95.5,
    };

    // Query: (age > 18 AND active = true) OR (score >= 90)
    let query = DnfQuery::builder()
        // First conjunction: age > 18 AND active = true (will fail)
        .or(|c| c.and("age", Op::GT, 18).and("active", Op::EQ, true))
        // Second conjunction: score >= 90 (will pass)
        .or(|c| c.and("score", Op::GTE, 90.0))
        .build();

    assert!(query.evaluate(&user)); // Should pass because second conjunction matches
}

#[derive(DnfEvaluable)]
struct Product {
    #[dnf(rename = "product_id")]
    id: u64,

    name: String,

    #[dnf(skip)]
    #[allow(dead_code)]
    internal_code: String,

    price: f32,
}

#[test]
fn test_derive_rename_attribute() {
    let product = Product {
        id: 12345,
        name: "Widget".to_string(),
        internal_code: "INTERNAL-123".to_string(),
        price: 99.99,
    };

    // Test renamed field
    assert!(test_field_condition(
        &product,
        "product_id",
        Op::EQ,
        Value::from(12345_u64)
    ));

    // Test that original name doesn't work
    assert!(!test_field_condition(
        &product,
        "id",
        Op::EQ,
        Value::from(12345_u64)
    ));
}

#[test]
fn test_derive_skip_attribute() {
    let product = Product {
        id: 12345,
        name: "Widget".to_string(),
        internal_code: "INTERNAL-123".to_string(),
        price: 99.99,
    };

    // Test that skipped field is not accessible
    assert!(!test_field_condition(
        &product,
        "internal_code",
        Op::EQ,
        Value::from("INTERNAL-123")
    ));
}

#[derive(DnfEvaluable)]
struct OptionalFields {
    name: String,
    age: Option<u32>,
    email: Option<String>,
    score: Option<f64>,
    verified: Option<bool>,
}

#[test]
fn test_derive_option_fields() {
    // Test with Some values
    let user1 = OptionalFields {
        name: "Alice".to_string(),
        age: Some(25),
        email: Some("alice@example.com".to_string()),
        score: Some(95.5),
        verified: Some(true),
    };

    assert!(test_field_condition(&user1, "age", Op::GT, Value::from(18)));

    // Test with None values
    let user2 = OptionalFields {
        name: "Bob".to_string(),
        age: None,
        email: None,
        score: None,
        verified: None,
    };

    assert!(test_field_condition(&user2, "age", Op::EQ, Value::None));
}

#[derive(DnfEvaluable)]
struct AllNumericTypes {
    u8_field: u8,
    u16_field: u16,
    u32_field: u32,
    u64_field: u64,
    i8_field: i8,
    i16_field: i16,
    i32_field: i32,
    i64_field: i64,
    f32_field: f32,
    f64_field: f64,
}

#[test]
fn test_derive_all_numeric_types() {
    let data = AllNumericTypes {
        u8_field: 255,
        u16_field: 65535,
        u32_field: 1000,
        u64_field: 100000,
        i8_field: -128,
        i16_field: -32768,
        i32_field: -1000,
        i64_field: -100000,
        f32_field: 3.11,
        f64_field: 2.118,
    };

    // Test various numeric types
    assert!(test_field_condition(
        &data,
        "u32_field",
        Op::EQ,
        Value::from(1000_u32)
    ));
    assert!(test_field_condition(
        &data,
        "i32_field",
        Op::LT,
        Value::from(0)
    ));
    assert!(test_field_condition(
        &data,
        "f64_field",
        Op::GT,
        Value::from(2.0)
    ));
}

#[test]
fn test_fields_method() {
    // Test basic fields
    let fields: Vec<_> = User::fields().collect();
    assert_eq!(fields.len(), 4);

    // Check field names
    let field_names: Vec<&str> = fields.iter().map(|f| f.name()).collect();
    let expected_fields = [
        ("age", true, "age field present"),
        ("name", true, "name field present"),
        ("active", true, "active field present"),
        ("score", true, "score field present"),
    ];
    for (field, present, desc) in expected_fields {
        assert_eq!(field_names.contains(&field), present, "User: {}", desc);
    }

    // Check field types
    let age_field = fields.iter().find(|f| f.name() == "age").unwrap();
    assert_eq!(age_field.field_type(), "u32");

    let name_field = fields.iter().find(|f| f.name() == "name").unwrap();
    assert_eq!(name_field.field_type(), "String");
}

#[test]
fn test_fields_with_attributes() {
    let fields: Vec<_> = Product::fields().collect();

    // Should have 3 fields (product_id, name, price - internal_code is skipped)
    assert_eq!(fields.len(), 3);

    // Check that renamed field uses new name
    let field_names: Vec<&str> = fields.iter().map(|f| f.name()).collect();
    let expected_fields = [
        ("product_id", true, "renamed field uses new name"),
        ("name", true, "name field present"),
        ("price", true, "price field present"),
        ("id", false, "original pre-rename name absent"),
        ("internal_code", false, "skipped field absent"),
    ];
    for (field, present, desc) in expected_fields {
        assert_eq!(field_names.contains(&field), present, "Product: {}", desc);
    }
}

// ==================== Nested Field Support Tests ====================

#[derive(DnfEvaluable, Debug)]
struct PersonName {
    first: String,
    last: String,
}

#[derive(DnfEvaluable, Debug)]
struct ContactInfo {
    email: String,
    phone: Option<String>,
}

#[derive(DnfEvaluable, Debug)]
struct Person {
    id: u64,
    #[dnf(nested)]
    name: PersonName,
    #[dnf(nested)]
    contact: ContactInfo,
    age: u32,
}

#[test]
fn test_nested_field_operations() {
    // Person with phone (Some)
    let person_with_phone = Person {
        id: 1,
        name: PersonName {
            first: "John".to_string(),
            last: "Doe".to_string(),
        },
        contact: ContactInfo {
            email: "john@example.com".to_string(),
            phone: Some("555-1234".to_string()),
        },
        age: 30,
    };

    // Person without phone (None)
    let person_no_phone = Person {
        id: 2,
        name: PersonName {
            first: "Alice".to_string(),
            last: "Johnson".to_string(),
        },
        contact: ContactInfo {
            email: "alice@company.com".to_string(),
            phone: None,
        },
        age: 25,
    };

    // Data-driven test cases: (person, field, operator, value, expected, description)
    let test_cases: Vec<(&Person, &str, Op, Value, bool, &str)> = vec![
        // Direct field access
        (
            &person_with_phone,
            "age",
            Op::EQ,
            Value::from(30),
            true,
            "age equals 30",
        ),
        // Nested field access: name.first, name.last
        (
            &person_with_phone,
            "name.first",
            Op::EQ,
            Value::from("John"),
            true,
            "name.first equals John",
        ),
        (
            &person_with_phone,
            "name.last",
            Op::EQ,
            Value::from("Doe"),
            true,
            "name.last equals Doe",
        ),
        // Nested field access: contact.email
        (
            &person_with_phone,
            "contact.email",
            Op::CONTAINS,
            Value::from("example.com"),
            true,
            "contact.email contains example.com",
        ),
        // Nested field with Option Some
        (
            &person_with_phone,
            "contact.phone",
            Op::EQ,
            Value::from("555-1234"),
            true,
            "contact.phone equals Some value",
        ),
        // Nested field with Option None
        (
            &person_no_phone,
            "contact.phone",
            Op::EQ,
            Value::None,
            true,
            "contact.phone equals None",
        ),
        // String operations on nested fields
        (
            &person_no_phone,
            "name.first",
            Op::STARTS_WITH,
            Value::from("Ali"),
            true,
            "name.first starts with Ali",
        ),
        (
            &person_no_phone,
            "name.last",
            Op::ENDS_WITH,
            Value::from("son"),
            true,
            "name.last ends with son",
        ),
    ];

    for (person, field, op, value, expected, desc) in test_cases {
        assert_eq!(
            test_field_condition(person, field, op, value),
            expected,
            "Failed: {}",
            desc
        );
    }

    // Complex query: (age > 30 AND name.first == "Bob") OR contact.email CONTAINS "work"
    let person_bob = Person {
        id: 42,
        name: PersonName {
            first: "Bob".to_string(),
            last: "Smith".to_string(),
        },
        contact: ContactInfo {
            email: "bob@work.com".to_string(),
            phone: Some("555-9999".to_string()),
        },
        age: 35,
    };

    let query = DnfQuery::builder()
        .or(|c| c.and("age", Op::GT, 30).and("name.first", Op::EQ, "Bob"))
        .or(|c| c.and("contact.email", Op::CONTAINS, "work"))
        .build();

    assert!(
        query.evaluate(&person_bob),
        "Complex DNF query should match"
    );
}

// ==================== Corner Case Tests ====================

#[derive(DnfEvaluable, Debug)]
struct NestedOptional {
    id: u64,
    #[dnf(nested)]
    details: OptionalDetails,
}

#[derive(DnfEvaluable, Debug)]
struct OptionalDetails {
    name: Option<String>,
    count: Option<u32>,
}

#[test]
fn test_nested_with_none_values() {
    let item = NestedOptional {
        id: 1,
        details: OptionalDetails {
            name: None,
            count: None,
        },
    };

    // Test nested None equality
    assert!(test_field_condition(
        &item,
        "details.name",
        Op::EQ,
        Value::None
    ));

    // Test nested None with Some value
    let item_with_value = NestedOptional {
        id: 2,
        details: OptionalDetails {
            name: Some("Test".to_string()),
            count: Some(42),
        },
    };

    assert!(test_field_condition(
        &item_with_value,
        "details.name",
        Op::NE,
        Value::None
    ));
}

#[derive(DnfEvaluable, Debug)]
struct DeepNested {
    #[dnf(nested)]
    level1: Level1,
}

#[derive(DnfEvaluable, Debug)]
struct Level1 {
    #[dnf(nested)]
    level2: Level2,
}

#[derive(DnfEvaluable, Debug)]
struct Level2 {
    #[dnf(nested)]
    level3: Level3,
}

#[derive(DnfEvaluable, Debug)]
struct Level3 {
    value: String,
}

#[test]
fn test_deeply_nested_fields() {
    let item = DeepNested {
        level1: Level1 {
            level2: Level2 {
                level3: Level3 {
                    value: "deep".to_string(),
                },
            },
        },
    };

    // Access deeply nested field
    assert!(test_field_condition(
        &item,
        "level1.level2.level3.value",
        Op::EQ,
        Value::from("deep")
    ));

    // String operations on deeply nested field
    assert!(test_field_condition(
        &item,
        "level1.level2.level3.value",
        Op::STARTS_WITH,
        Value::from("dee")
    ));
}

#[derive(DnfEvaluable, Debug)]
struct EmptyStringFields {
    name: String,
    description: String,
}

#[test]
fn test_empty_string_fields() {
    let item = EmptyStringFields {
        name: "".to_string(),
        description: "has content".to_string(),
    };

    // Empty string equality
    assert!(test_field_condition(&item, "name", Op::EQ, Value::from("")));

    // Empty string not equals non-empty
    assert!(test_field_condition(
        &item,
        "name",
        Op::NE,
        Value::from("something")
    ));

    // Contains empty string (should match anything)
    assert!(test_field_condition(
        &item,
        "description",
        Op::CONTAINS,
        Value::from("")
    ));
}

#[derive(DnfEvaluable, Debug)]
struct ZeroValues {
    int_val: i64,
    uint_val: u64,
    float_val: f64,
}

#[test]
fn test_zero_value_fields() {
    let item = ZeroValues {
        int_val: 0,
        uint_val: 0,
        float_val: 0.0,
    };

    // Zero equality
    assert!(test_field_condition(
        &item,
        "int_val",
        Op::EQ,
        Value::Int(0)
    ));

    // Zero comparisons
    assert!(test_field_condition(
        &item,
        "uint_val",
        Op::GTE,
        Value::Uint(0)
    ));

    // Zero float
    assert!(test_field_condition(
        &item,
        "float_val",
        Op::EQ,
        Value::Float(0.0)
    ));
}

#[test]
fn test_unknown_nested_field() {
    let person = Person {
        id: 1,
        name: PersonName {
            first: "John".to_string(),
            last: "Doe".to_string(),
        },
        contact: ContactInfo {
            email: "john@example.com".to_string(),
            phone: None,
        },
        age: 30,
    };

    // Unknown nested field should return false
    assert!(!test_field_condition(
        &person,
        "name.middle",
        Op::EQ,
        Value::from("X")
    ));

    // Unknown top-level prefix should return false
    assert!(!test_field_condition(
        &person,
        "unknown.field",
        Op::EQ,
        Value::from("value")
    ));
}

#[derive(DnfEvaluable, Debug)]
struct SpecialCharFields {
    unicode_name: String,
    with_newlines: String,
}

#[test]
fn test_unicode_and_special_chars() {
    let item = SpecialCharFields {
        unicode_name: "日本語 émoji 🎉".to_string(),
        with_newlines: "line1\nline2\ttab".to_string(),
    };

    // Unicode contains
    assert!(test_field_condition(
        &item,
        "unicode_name",
        Op::CONTAINS,
        Value::from("日本語")
    ));

    // Emoji
    assert!(test_field_condition(
        &item,
        "unicode_name",
        Op::CONTAINS,
        Value::from("🎉")
    ));

    // Newline
    assert!(test_field_condition(
        &item,
        "with_newlines",
        Op::CONTAINS,
        Value::from("\n")
    ));
}

// ==================== Vec Field Support Tests ====================

#[derive(DnfEvaluable, Debug)]
struct UserWithTags {
    id: u64,
    name: String,
    tags: Vec<String>,
    scores: Vec<i32>,
    ratings: Vec<f64>,
    flags: Vec<bool>,
}

#[test]
fn test_vec_field_operations() {
    let user = UserWithTags {
        id: 1,
        name: "Alice".to_string(),
        tags: vec![
            "rust".to_string(),
            "programming".to_string(),
            "web".to_string(),
        ],
        scores: vec![85, 90, 95, 100],
        ratings: vec![4.5, 4.8, 5.0],
        flags: vec![true, false, true],
    };

    // Data-driven test cases: (field, operator, value, expected, description)
    let test_cases: Vec<(&str, Op, Value, bool, &str)> = vec![
        // Vec<String> CONTAINS
        (
            "tags",
            Op::CONTAINS,
            Value::from("rust"),
            true,
            "tags contains 'rust'",
        ),
        (
            "tags",
            Op::CONTAINS,
            Value::from("programming"),
            true,
            "tags contains 'programming'",
        ),
        (
            "tags",
            Op::CONTAINS,
            Value::from("python"),
            false,
            "tags does not contain 'python'",
        ),
        (
            "tags",
            Op::NOT_CONTAINS,
            Value::from("java"),
            true,
            "tags not contains 'java'",
        ),
        // Vec<i32> CONTAINS
        (
            "scores",
            Op::CONTAINS,
            Value::Int(90),
            true,
            "scores contains 90",
        ),
        (
            "scores",
            Op::CONTAINS,
            Value::Int(100),
            true,
            "scores contains 100",
        ),
        (
            "scores",
            Op::CONTAINS,
            Value::Int(50),
            false,
            "scores does not contain 50",
        ),
        // Vec<f64> CONTAINS
        (
            "ratings",
            Op::CONTAINS,
            Value::Float(4.8),
            true,
            "ratings contains 4.8",
        ),
        (
            "ratings",
            Op::CONTAINS,
            Value::Float(3.0),
            false,
            "ratings does not contain 3.0",
        ),
        // Vec<bool> CONTAINS
        (
            "flags",
            Op::CONTAINS,
            Value::Bool(true),
            true,
            "flags contains true",
        ),
        (
            "flags",
            Op::CONTAINS,
            Value::Bool(false),
            true,
            "flags contains false",
        ),
    ];

    for (field, op, value, expected, desc) in test_cases {
        assert_eq!(
            test_field_condition(&user, field, op, value),
            expected,
            "Failed: {}",
            desc
        );
    }
}

#[test]
fn test_vec_all_of_operator() {
    let user = UserWithTags {
        id: 1,
        name: "Eve".to_string(),
        tags: vec![
            "rust".to_string(),
            "web".to_string(),
            "backend".to_string(),
            "api".to_string(),
        ],
        scores: vec![],
        ratings: vec![],
        flags: vec![],
    };

    // ALL OF - all specified tags must be present
    assert!(test_field_condition(
        &user,
        "tags",
        Op::ALL_OF,
        Value::from(vec!["rust", "web"])
    ));

    assert!(test_field_condition(
        &user,
        "tags",
        Op::ALL_OF,
        Value::from(vec!["backend", "api"])
    ));

    // Missing one tag - should fail
    assert!(!test_field_condition(
        &user,
        "tags",
        Op::ALL_OF,
        Value::from(vec!["rust", "python"])
    ));
}

#[test]
fn test_vec_any_of_operator() {
    let user = UserWithTags {
        id: 1,
        name: "Frank".to_string(),
        tags: vec!["rust".to_string(), "go".to_string()],
        scores: vec![80, 85, 90],
        ratings: vec![],
        flags: vec![],
    };

    // ANY OF - at least one tag must match
    assert!(test_field_condition(
        &user,
        "tags",
        Op::ANY_OF,
        Value::from(vec!["rust", "python", "java"])
    ));

    assert!(test_field_condition(
        &user,
        "tags",
        Op::ANY_OF,
        Value::from(vec!["go", "c++"])
    ));

    // No matching tags
    assert!(!test_field_condition(
        &user,
        "tags",
        Op::ANY_OF,
        Value::from(vec!["python", "java", "c++"])
    ));

    // Test ANY OF with numbers (IN operator behavior)
    assert!(test_field_condition(
        &user,
        "scores",
        Op::ANY_OF,
        Value::from(vec![70, 80, 100])
    ));
}

#[test]
fn test_vec_empty() {
    let user = UserWithTags {
        id: 1,
        name: "Grace".to_string(),
        tags: vec![],
        scores: vec![],
        ratings: vec![],
        flags: vec![],
    };

    // Empty vec should not contain anything
    assert!(!test_field_condition(
        &user,
        "tags",
        Op::CONTAINS,
        Value::from("rust")
    ));

    // Empty vec ALL OF empty is true
    assert!(test_field_condition(
        &user,
        "tags",
        Op::ALL_OF,
        Value::from(vec![""; 0])
    ));

    // Empty vec ANY OF anything is false
    assert!(!test_field_condition(
        &user,
        "tags",
        Op::ANY_OF,
        Value::from(vec!["rust"])
    ));
}

#[test]
fn test_vec_zero_copy_evaluation() {
    // This test verifies that Vec fields work without cloning
    let tags = vec!["a".to_string(), "b".to_string(), "c".to_string()];

    let user = UserWithTags {
        id: 1,
        name: "Test".to_string(),
        tags: tags.clone(),
        scores: vec![],
        ratings: vec![],
        flags: vec![],
    };

    // Evaluate multiple queries - should not require cloning the original vec
    assert!(test_field_condition(
        &user,
        "tags",
        Op::CONTAINS,
        Value::from("a")
    ));

    assert!(test_field_condition(
        &user,
        "tags",
        Op::CONTAINS,
        Value::from("b")
    ));

    // Original tags should still be usable
    assert_eq!(tags.len(), 3);
}

#[derive(DnfEvaluable, Debug)]
struct DataWithOptionalVec {
    id: u64,
    tags: Option<Vec<String>>,
    scores: Option<Vec<i32>>,
}

#[test]
fn test_optional_vec_field() {
    // Test with Some(vec)
    let data = DataWithOptionalVec {
        id: 1,
        tags: Some(vec!["rust".to_string(), "web".to_string()]),
        scores: Some(vec![90, 95, 100]),
    };

    assert!(test_field_condition(
        &data,
        "tags",
        Op::CONTAINS,
        Value::from("rust")
    ));

    assert!(test_field_condition(
        &data,
        "scores",
        Op::CONTAINS,
        Value::Int(95)
    ));

    // Test with None
    let data_none = DataWithOptionalVec {
        id: 2,
        tags: None,
        scores: None,
    };

    assert!(test_field_condition(
        &data_none,
        "tags",
        Op::EQ,
        Value::None
    ));

    assert!(test_field_condition(
        &data_none,
        "scores",
        Op::EQ,
        Value::None
    ));
}

// ==================== HashSet Field Support Tests ====================

#[derive(DnfEvaluable, Debug)]
struct UserWithHashSets {
    id: u64,
    name: String,
    tags: HashSet<String>,
    user_ids: HashSet<i32>,
    flags: HashSet<bool>,
}

#[test]
fn test_hashset_field_operations() {
    let mut tags = HashSet::new();
    tags.insert("rust".to_string());
    tags.insert("programming".to_string());
    tags.insert("web".to_string());

    let mut user_ids = HashSet::new();
    user_ids.insert(10);
    user_ids.insert(20);
    user_ids.insert(30);

    let mut flags = HashSet::new();
    flags.insert(true);
    flags.insert(false);

    let user = UserWithHashSets {
        id: 1,
        name: "Alice".to_string(),
        tags,
        user_ids,
        flags,
    };

    // Data-driven test cases: (field, operator, value, expected, description)
    let test_cases: Vec<(&str, Op, Value, bool, &str)> = vec![
        // HashSet<String> CONTAINS
        (
            "tags",
            Op::CONTAINS,
            Value::from("rust"),
            true,
            "tags contains 'rust'",
        ),
        (
            "tags",
            Op::CONTAINS,
            Value::from("programming"),
            true,
            "tags contains 'programming'",
        ),
        (
            "tags",
            Op::CONTAINS,
            Value::from("python"),
            false,
            "tags does not contain 'python'",
        ),
        // HashSet<i32> CONTAINS
        (
            "user_ids",
            Op::CONTAINS,
            Value::Int(20),
            true,
            "user_ids contains 20",
        ),
        (
            "user_ids",
            Op::CONTAINS,
            Value::Int(99),
            false,
            "user_ids does not contain 99",
        ),
        // HashSet<bool> CONTAINS
        (
            "flags",
            Op::CONTAINS,
            Value::Bool(true),
            true,
            "flags contains true",
        ),
        (
            "flags",
            Op::CONTAINS,
            Value::Bool(false),
            true,
            "flags contains false",
        ),
    ];

    for (field, op, value, expected, desc) in test_cases {
        assert_eq!(
            test_field_condition(&user, field, op, value),
            expected,
            "Failed: {}",
            desc
        );
    }
}

#[test]
fn test_hashset_all_of_operator() {
    let mut tags = HashSet::new();
    tags.insert("rust".to_string());
    tags.insert("web".to_string());
    tags.insert("backend".to_string());

    let user = UserWithHashSets {
        id: 1,
        name: "Diana".to_string(),
        tags,
        user_ids: HashSet::new(),
        flags: HashSet::new(),
    };

    // ALL OF - all specified tags must be present
    assert!(test_field_condition(
        &user,
        "tags",
        Op::ALL_OF,
        Value::from(vec!["rust", "web"])
    ));

    // Missing one tag
    assert!(!test_field_condition(
        &user,
        "tags",
        Op::ALL_OF,
        Value::from(vec!["rust", "python"])
    ));
}

#[test]
fn test_hashset_any_of_operator() {
    let mut tags = HashSet::new();
    tags.insert("rust".to_string());
    tags.insert("go".to_string());

    let user = UserWithHashSets {
        id: 1,
        name: "Eve".to_string(),
        tags,
        user_ids: HashSet::new(),
        flags: HashSet::new(),
    };

    // ANY OF - at least one tag must match
    assert!(test_field_condition(
        &user,
        "tags",
        Op::ANY_OF,
        Value::from(vec!["rust", "python", "java"])
    ));

    assert!(!test_field_condition(
        &user,
        "tags",
        Op::ANY_OF,
        Value::from(vec!["python", "java", "c++"])
    ));
}

#[test]
fn test_hashset_empty() {
    let user = UserWithHashSets {
        id: 1,
        name: "Frank".to_string(),
        tags: HashSet::new(),
        user_ids: HashSet::new(),
        flags: HashSet::new(),
    };

    // Empty hashset should not contain anything
    assert!(!test_field_condition(
        &user,
        "tags",
        Op::CONTAINS,
        Value::from("rust")
    ));
}

#[test]
fn test_hashset_deduplication() {
    // HashSet automatically deduplicates
    let mut user_ids = HashSet::new();
    user_ids.insert(1);
    user_ids.insert(2);
    user_ids.insert(2); // Duplicate - will be ignored
    user_ids.insert(3);

    assert_eq!(user_ids.len(), 3); // Only 3 unique values

    let user = UserWithHashSets {
        id: 1,
        name: "Grace".to_string(),
        tags: HashSet::new(),
        user_ids,
        flags: HashSet::new(),
    };

    assert!(test_field_condition(
        &user,
        "user_ids",
        Op::CONTAINS,
        Value::Int(2)
    ));
}

#[derive(DnfEvaluable, Debug)]
struct DataWithOptionalHashSet {
    id: u64,
    tags: Option<HashSet<String>>,
}

#[test]
fn test_optional_hashset_field() {
    // Test with Some(hashset)
    let mut tags = HashSet::new();
    tags.insert("rust".to_string());

    let data = DataWithOptionalHashSet {
        id: 1,
        tags: Some(tags),
    };

    assert!(test_field_condition(
        &data,
        "tags",
        Op::CONTAINS,
        Value::from("rust")
    ));

    // Test with None
    let data_none = DataWithOptionalHashSet { id: 2, tags: None };

    assert!(test_field_condition(
        &data_none,
        "tags",
        Op::EQ,
        Value::None
    ));
}

// ==================== Deep Nested Fields Tests (3 levels) ====================

#[derive(DnfEvaluable, Debug)]
struct Address {
    street: String,
    city: String,
    zip: String,
}

#[derive(DnfEvaluable, Debug)]
struct Location {
    #[dnf(nested)]
    address: Address,
    building: String,
    floor: u32,
}

#[derive(DnfEvaluable, Debug)]
struct Organization {
    id: u64,
    name: String,
    #[dnf(nested)]
    location: Location,
    employees: u32,
}

#[test]
fn test_deep_nested_fields_3_levels() {
    let org = Organization {
        id: 42,
        name: "Acme Corp".to_string(),
        location: Location {
            address: Address {
                street: "123 Main St".to_string(),
                city: "Boston".to_string(),
                zip: "02101".to_string(),
            },
            building: "Tower A".to_string(),
            floor: 5,
        },
        employees: 150,
    };

    // Test 1 level deep: direct field access
    assert!(test_field_condition(
        &org,
        "id",
        Op::EQ,
        Value::from(42_u64)
    ));
    assert!(test_field_condition(
        &org,
        "name",
        Op::EQ,
        Value::from("Acme Corp")
    ));

    // Test 2 levels deep: location.building
    assert!(test_field_condition(
        &org,
        "location.building",
        Op::EQ,
        Value::from("Tower A")
    ));
    assert!(test_field_condition(
        &org,
        "location.floor",
        Op::EQ,
        Value::from(5)
    ));

    // Test 3 levels deep: location.address.city
    assert!(test_field_condition(
        &org,
        "location.address.city",
        Op::EQ,
        Value::from("Boston")
    ));
    assert!(test_field_condition(
        &org,
        "location.address.street",
        Op::EQ,
        Value::from("123 Main St")
    ));
    assert!(test_field_condition(
        &org,
        "location.address.zip",
        Op::EQ,
        Value::from("02101")
    ));

    // Test 3 levels deep with string operations
    assert!(test_field_condition(
        &org,
        "location.address.city",
        Op::STARTS_WITH,
        Value::from("Bos")
    ));
    assert!(test_field_condition(
        &org,
        "location.address.street",
        Op::CONTAINS,
        Value::from("Main")
    ));

    // Test complex query with multiple nesting levels
    let query = DnfQuery::builder()
        .or(|c| {
            c.and("employees", Op::GT, 100)
                .and("location.address.city", Op::EQ, "Boston")
                .and("location.floor", Op::GTE, 1)
        })
        .build();
    assert!(query.evaluate(&org));

    // Test DNF query: (city == "Boston" AND floor > 3) OR (employees > 200)
    let query = DnfQuery::builder()
        // First conjunction: city == "Boston" AND floor > 3 (will pass)
        .or(|c| {
            c.and("location.address.city", Op::EQ, "Boston")
                .and("location.floor", Op::GT, 3)
        })
        // Second conjunction: employees > 200 (will fail)
        .or(|c| c.and("employees", Op::GT, 200))
        .build();

    assert!(query.evaluate(&org)); // Should pass because first conjunction matches
}

#[test]
fn test_deep_nested_fields_nonexistent_path() {
    let org = Organization {
        id: 1,
        name: "Test Inc".to_string(),
        location: Location {
            address: Address {
                street: "456 Oak Ave".to_string(),
                city: "Cambridge".to_string(),
                zip: "02138".to_string(),
            },
            building: "Building B".to_string(),
            floor: 2,
        },
        employees: 50,
    };

    // Test with incorrect nested field path - should not match
    assert!(!test_field_condition(
        &org,
        "location.address.country",
        Op::EQ,
        Value::from("USA")
    ));

    // Test with invalid path depth
    assert!(!test_field_condition(
        &org,
        "location.address.city.name",
        Op::EQ,
        Value::from("Boston")
    ));
}

// ==================== Custom Iterator Attribute Tests ====================

use std::collections::LinkedList;

#[derive(DnfEvaluable, Debug)]
struct DocumentWithLinkedList {
    id: u64,
    #[dnf(iter)]
    tags: LinkedList<String>,
    #[dnf(iter)]
    scores: LinkedList<i32>,
}

#[test]
fn test_dnf_iter_attribute_with_linked_list() {
    let mut tags = LinkedList::new();
    tags.push_back("rust".to_string());
    tags.push_back("programming".to_string());
    tags.push_back("web".to_string());

    let mut scores = LinkedList::new();
    scores.push_back(85);
    scores.push_back(90);
    scores.push_back(95);

    let doc = DocumentWithLinkedList {
        id: 1,
        tags,
        scores,
    };

    // Test CONTAINS on LinkedList<String>
    assert!(test_field_condition(
        &doc,
        "tags",
        Op::CONTAINS,
        Value::from("rust")
    ));

    // Test not CONTAINS
    assert!(!test_field_condition(
        &doc,
        "tags",
        Op::CONTAINS,
        Value::from("python")
    ));

    // Test ANY OF on LinkedList<String>
    assert!(test_field_condition(
        &doc,
        "tags",
        Op::ANY_OF,
        Value::from(vec!["rust", "java"])
    ));

    // Test ALL OF on LinkedList<i32>
    assert!(test_field_condition(
        &doc,
        "scores",
        Op::ALL_OF,
        Value::from(vec![85, 90])
    ));

    // Test CONTAINS on LinkedList<i32>
    assert!(test_field_condition(
        &doc,
        "scores",
        Op::CONTAINS,
        Value::from(90)
    ));
}

#[test]
fn test_dnf_iter_field_kind() {
    // Verify that fields with #[dnf(iter)] are reported as Iter kind
    let fields: Vec<_> = DocumentWithLinkedList::fields().collect();

    let tags_field = fields.iter().find(|f| f.name() == "tags").unwrap();
    assert_eq!(tags_field.kind(), dnf::FieldKind::Iter);

    let scores_field = fields.iter().find(|f| f.name() == "scores").unwrap();
    assert_eq!(scores_field.kind(), dnf::FieldKind::Iter);
}

// ==================== Nested Collections Tests ====================

/// Office location for testing nested collections
#[derive(DnfEvaluable, Debug, Clone)]
struct OfficeLocation {
    city: String,
    zip: String,
}

/// Company with Vec of nested OfficeLocation structs (auto-detected, no attribute needed)
#[derive(DnfEvaluable, Debug)]
struct CompanyWithOffices {
    name: String,
    offices: Vec<OfficeLocation>, // Auto-detected as nested
}

#[test]
fn test_nested_vec_any_semantics() {
    let company = CompanyWithOffices {
        name: "Acme Corp".to_string(),
        offices: vec![
            OfficeLocation {
                city: "Boston".to_string(),
                zip: "02101".to_string(),
            },
            OfficeLocation {
                city: "New York".to_string(),
                zip: "10001".to_string(),
            },
            OfficeLocation {
                city: "Chicago".to_string(),
                zip: "60601".to_string(),
            },
        ],
    };

    // offices.city == "Boston" -> any office has city == "Boston"
    assert!(test_field_condition(
        &company,
        "offices.city",
        Op::EQ,
        Value::from("Boston")
    ));

    // offices.city == "LA" -> no office has city == "LA"
    assert!(!test_field_condition(
        &company,
        "offices.city",
        Op::EQ,
        Value::from("LA")
    ));

    // offices.city CONTAINS "New" -> any office city contains "New"
    assert!(test_field_condition(
        &company,
        "offices.city",
        Op::CONTAINS,
        Value::from("New")
    ));

    // offices.zip STARTS WITH "100" -> any office zip starts with "100"
    assert!(test_field_condition(
        &company,
        "offices.zip",
        Op::STARTS_WITH,
        Value::from("100")
    ));
}

#[test]
fn test_nested_vec_empty() {
    let company = CompanyWithOffices {
        name: "Empty Corp".to_string(),
        offices: vec![],
    };

    // Empty collection -> any() returns false
    assert!(!test_field_condition(
        &company,
        "offices.city",
        Op::EQ,
        Value::from("Boston")
    ));
}

/// Test with HashMap containing nested structs
use std::collections::HashMap;

#[derive(DnfEvaluable, Debug)]
struct OrgWithBranches {
    name: String,
    branches: HashMap<String, OfficeLocation>, // Auto-detected as nested
}

#[test]
fn test_nested_hashmap_values_any_semantics() {
    let mut branches = HashMap::new();
    branches.insert(
        "hq".to_string(),
        OfficeLocation {
            city: "Boston".to_string(),
            zip: "02101".to_string(),
        },
    );
    branches.insert(
        "west".to_string(),
        OfficeLocation {
            city: "Seattle".to_string(),
            zip: "98101".to_string(),
        },
    );

    let org = OrgWithBranches {
        name: "Tech Inc".to_string(),
        branches,
    };

    // branches.@values.city == "Boston" -> explicit @values, any branch has city == "Boston"
    assert!(test_field_condition(
        &org,
        "branches.@values.city",
        Op::EQ,
        Value::from("Boston")
    ));

    // branches.@values.city == "Chicago" -> no branch has city == "Chicago"
    assert!(!test_field_condition(
        &org,
        "branches.@values.city",
        Op::EQ,
        Value::from("Chicago")
    ));

    // branches.@values.zip STARTS WITH "98" -> Seattle's zip
    assert!(test_field_condition(
        &org,
        "branches.@values.zip",
        Op::STARTS_WITH,
        Value::from("98")
    ));
}

#[test]
fn test_nested_hashmap_explicit_values_syntax() {
    let mut branches = HashMap::new();
    branches.insert(
        "hq".to_string(),
        OfficeLocation {
            city: "Boston".to_string(),
            zip: "02101".to_string(),
        },
    );
    branches.insert(
        "west".to_string(),
        OfficeLocation {
            city: "Seattle".to_string(),
            zip: "98101".to_string(),
        },
    );

    let org = OrgWithBranches {
        name: "Tech Inc".to_string(),
        branches,
    };

    // branches.@values.city == "Boston" -> explicit @values syntax
    assert!(test_field_condition(
        &org,
        "branches.@values.city",
        Op::EQ,
        Value::from("Boston")
    ));

    // branches.@values.zip CONTAINS "021" -> explicit @values syntax
    assert!(test_field_condition(
        &org,
        "branches.@values.zip",
        Op::CONTAINS,
        Value::from("021")
    ));
}

#[test]
fn test_nested_hashmap_keys_access() {
    let mut branches = HashMap::new();
    branches.insert(
        "headquarters".to_string(),
        OfficeLocation {
            city: "Boston".to_string(),
            zip: "02101".to_string(),
        },
    );
    branches.insert(
        "west_coast".to_string(),
        OfficeLocation {
            city: "Seattle".to_string(),
            zip: "98101".to_string(),
        },
    );

    let org = OrgWithBranches {
        name: "Tech Inc".to_string(),
        branches,
    };

    // branches.@keys CONTAINS "headquarters" -> keys iteration
    assert!(test_field_condition(
        &org,
        "branches.@keys",
        Op::CONTAINS,
        Value::from("headquarters")
    ));

    // branches.@keys CONTAINS "europe" -> no such key
    assert!(!test_field_condition(
        &org,
        "branches.@keys",
        Op::CONTAINS,
        Value::from("europe")
    ));
}

#[test]
fn test_nested_hashmap_specific_key_access() {
    let mut branches = HashMap::new();
    branches.insert(
        "hq".to_string(),
        OfficeLocation {
            city: "Boston".to_string(),
            zip: "02101".to_string(),
        },
    );
    branches.insert(
        "west".to_string(),
        OfficeLocation {
            city: "Seattle".to_string(),
            zip: "98101".to_string(),
        },
    );

    let org = OrgWithBranches {
        name: "Tech Inc".to_string(),
        branches,
    };

    // branches["hq"].city == "Boston" -> specific key access
    assert!(test_field_condition(
        &org,
        "branches.[\"hq\"].city",
        Op::EQ,
        Value::from("Boston")
    ));

    // branches["hq"].city == "Seattle" -> wrong value
    assert!(!test_field_condition(
        &org,
        "branches.[\"hq\"].city",
        Op::EQ,
        Value::from("Seattle")
    ));

    // branches["west"].zip STARTS WITH "98"
    assert!(test_field_condition(
        &org,
        "branches.[\"west\"].zip",
        Op::STARTS_WITH,
        Value::from("98")
    ));

    // branches["nonexistent"].city == "Boston" -> key doesn't exist
    assert!(!test_field_condition(
        &org,
        "branches.[\"nonexistent\"].city",
        Op::EQ,
        Value::from("Boston")
    ));
}

#[test]
fn test_nested_vec_with_query_builder() {
    let company = CompanyWithOffices {
        name: "Acme Corp".to_string(),
        offices: vec![
            OfficeLocation {
                city: "Boston".to_string(),
                zip: "02101".to_string(),
            },
            OfficeLocation {
                city: "New York".to_string(),
                zip: "10001".to_string(),
            },
        ],
    };

    // Build query: name == "Acme Corp" AND offices.city == "Boston"
    let query = DnfQuery::builder()
        .or(|c| {
            c.and("name", Op::EQ, "Acme Corp")
                .and("offices.city", Op::EQ, "Boston")
        })
        .build();

    assert!(query.evaluate(&company));

    // Build query: offices.city == "LA" (should fail)
    let query = DnfQuery::builder()
        .or(|c| c.and("offices.city", Op::EQ, "LA"))
        .build();

    assert!(!query.evaluate(&company));
}

// ==================== Cow<str> Tests ====================

use std::borrow::Cow;

#[derive(DnfEvaluable)]
struct DocWithCow {
    title: Cow<'static, str>,
    content: String,
    status: Cow<'static, str>,
}

#[test]
fn test_cow_str_field() {
    let test_cases = vec![
        // (field, op, value, expected, description)
        (
            "title",
            Op::EQ,
            Value::from("Hello"),
            true,
            "borrowed cow equals",
        ),
        (
            "title",
            Op::NE,
            Value::from("World"),
            true,
            "borrowed cow not equals",
        ),
        (
            "title",
            Op::CONTAINS,
            Value::from("ell"),
            true,
            "borrowed cow contains",
        ),
        (
            "title",
            Op::STARTS_WITH,
            Value::from("Hel"),
            true,
            "borrowed cow starts with",
        ),
        (
            "title",
            Op::ENDS_WITH,
            Value::from("lo"),
            true,
            "borrowed cow ends with",
        ),
        (
            "status",
            Op::EQ,
            Value::from("active"),
            true,
            "owned cow equals",
        ),
    ];

    let doc = DocWithCow {
        title: Cow::Borrowed("Hello"),
        content: "Some content".to_string(),
        status: Cow::Owned("active".to_string()),
    };

    for (field, op, value, expected, desc) in test_cases {
        assert_eq!(
            test_field_condition(&doc, field, op, value),
            expected,
            "Failed: {}",
            desc
        );
    }
}

#[test]
fn test_cow_str_with_complex_query() {
    let doc = DocWithCow {
        title: Cow::Borrowed("Rust Guide"),
        content: "Learn Rust programming".to_string(),
        status: Cow::Owned("published".to_string()),
    };

    // Query: title CONTAINS "Rust" AND status == "published"
    let query = DnfQuery::builder()
        .or(|c| {
            c.and("title", Op::CONTAINS, "Rust")
                .and("status", Op::EQ, "published")
        })
        .build();

    assert!(query.evaluate(&doc));

    // Query: title == "Rust Guide" OR status == "draft"
    let query2 = DnfQuery::builder()
        .or(|c| c.and("title", Op::EQ, "Rust Guide"))
        .or(|c| c.and("status", Op::EQ, "draft"))
        .build();

    assert!(query2.evaluate(&doc)); // Should pass because first conjunction matches
}

#[derive(DnfEvaluable)]
struct DocWithOptionalCow {
    title: Option<Cow<'static, str>>,
    tags: Option<Vec<Cow<'static, str>>>,
}

#[test]
fn test_optional_cow_str_field() {
    let test_cases = vec![
        // (field, op, value, expected, description)
        (
            "title",
            Op::EQ,
            Value::from("Test"),
            true,
            "Some cow equals",
        ),
        (
            "title",
            Op::CONTAINS,
            Value::from("es"),
            true,
            "Some cow contains",
        ),
    ];

    let doc = DocWithOptionalCow {
        title: Some(Cow::Borrowed("Test")),
        tags: None,
    };

    for (field, op, value, expected, desc) in test_cases {
        assert_eq!(
            test_field_condition(&doc, field, op, value),
            expected,
            "Failed: {}",
            desc
        );
    }

    // Test None case
    let doc_none = DocWithOptionalCow {
        title: None,
        tags: None,
    };

    assert!(test_field_condition(
        &doc_none,
        "title",
        Op::EQ,
        Value::None
    ));
}

// ==================== Box<str> Tests ====================

#[derive(DnfEvaluable)]
struct DocWithBoxStr {
    title: Box<str>,
    tag: Box<str>,
    nickname: Option<Box<str>>,
}

#[test]
fn test_box_str_field() {
    let doc = DocWithBoxStr {
        title: "Hello".into(),
        tag: "rust".into(),
        nickname: Some("rusty".into()),
    };

    let test_cases = vec![
        ("title", Op::EQ, Value::from("Hello"), true, "equals"),
        ("title", Op::NE, Value::from("World"), true, "not equals"),
        ("title", Op::CONTAINS, Value::from("ell"), true, "contains"),
        (
            "title",
            Op::STARTS_WITH,
            Value::from("Hel"),
            true,
            "starts with",
        ),
        ("title", Op::ENDS_WITH, Value::from("lo"), true, "ends with"),
        (
            "tag",
            Op::EQ,
            Value::from("rust"),
            true,
            "second field equals",
        ),
        (
            "nickname",
            Op::EQ,
            Value::from("rusty"),
            true,
            "Some<Box<str>> equals",
        ),
        (
            "nickname",
            Op::CONTAINS,
            Value::from("ust"),
            true,
            "Some<Box<str>> contains",
        ),
    ];

    for (field, op, value, expected, desc) in test_cases {
        assert_eq!(
            test_field_condition(&doc, field, op, value),
            expected,
            "Failed: {}",
            desc
        );
    }

    // Option<Box<str>>: None compares equal to Value::None.
    let doc_none = DocWithBoxStr {
        title: "x".into(),
        tag: "y".into(),
        nickname: None,
    };
    assert!(test_field_condition(
        &doc_none,
        "nickname",
        Op::EQ,
        Value::None
    ));
}

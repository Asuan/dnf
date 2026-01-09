use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;

// String types support cross-type comparisons via string representation.

impl PartialOrd<Value> for str {
    #[inline]
    fn partial_cmp(&self, value: &Value) -> Option<Ordering> {
        match value {
            Value::String(s) => self.partial_cmp(s.as_ref()),
            _ => self.partial_cmp(value.to_string_repr().as_ref()),
        }
    }
}

impl PartialOrd<Value> for &str {
    #[inline]
    fn partial_cmp(&self, value: &Value) -> Option<Ordering> {
        (*self).partial_cmp(value)
    }
}

// Macro for string wrapper types that delegate to &str
macro_rules! impl_partial_ord_str_delegate {
    ($($t:ty),+ $(,)?) => {
        $(
            impl PartialOrd<Value> for $t {
                #[inline]
                fn partial_cmp(&self, value: &Value) -> Option<Ordering> {
                    <$t as AsRef<str>>::as_ref(self).partial_cmp(value)
                }
            }
        )+
    };
}

impl_partial_ord_str_delegate!(String, Box<str>, Cow<'_, str>);

// macro for numeric PartialOrd<Value> implementations
macro_rules! impl_partial_ord_numeric {
    ($($t:ty),+ => $cast:ty) => {
        $(
            impl PartialOrd<Value> for $t {
                #[inline]
                fn partial_cmp(&self, value: &Value) -> Option<Ordering> {
                    (*self as $cast).partial_cmp(value)
                }
            }
        )+
    };
}

// Apply macro (i64, u64, f64 have manual implementations below)
impl_partial_ord_numeric!(i32, i16, i8, isize => i64);
impl_partial_ord_numeric!(u32, u16, u8, usize => u64);
impl_partial_ord_numeric!(f32 => f64);

// Bool type

impl PartialOrd<Value> for bool {
    #[inline]
    fn partial_cmp(&self, value: &Value) -> Option<Ordering> {
        if let Value::Bool(b) = value {
            self.partial_cmp(b)
        } else {
            None
        }
    }
}

/// Represents a value that can be used in DNF query conditions.
///
/// Supports common data types for comparisons.
/// Uses `Box<str>` for internal string storage - queries are typically built once and evaluated many times.
/// Accepts `String`, `&str`, `Box<str>`, and `Cow<str>` via `From` trait implementations.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", content = "value"))]
#[non_exhaustive]
pub enum Value {
    /// String value
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "serialize_box_str",
            deserialize_with = "deserialize_box_str"
        )
    )]
    String(Box<str>),
    /// Signed integer value
    Int(i64),
    /// Unsigned integer value
    Uint(u64),
    /// Floating point value
    Float(f64),
    /// Boolean value
    Bool(bool),
    /// Null/None value
    None,
    /// Array of strings
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "serialize_box_str_slice",
            deserialize_with = "deserialize_box_str_slice"
        )
    )]
    StringArray(Box<[Box<str>]>),
    /// Array of signed integers
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "serialize_box_i64_slice",
            deserialize_with = "deserialize_box_i64_slice"
        )
    )]
    IntArray(Box<[i64]>),
    /// Array of unsigned integers
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "serialize_box_u64_slice",
            deserialize_with = "deserialize_box_u64_slice"
        )
    )]
    UintArray(Box<[u64]>),
    /// Array of floats
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "serialize_box_f64_slice",
            deserialize_with = "deserialize_box_f64_slice"
        )
    )]
    FloatArray(Box<[f64]>),
    /// Array of booleans
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "serialize_box_bool_slice",
            deserialize_with = "deserialize_box_bool_slice"
        )
    )]
    BoolArray(Box<[bool]>),
    /// Set of strings - preserves set semantics
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "serialize_string_set",
            deserialize_with = "deserialize_string_set"
        )
    )]
    StringSet(Box<HashSet<Box<str>>>),
    /// Set of signed integers - preserves set semantics
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "serialize_int_set",
            deserialize_with = "deserialize_int_set"
        )
    )]
    IntSet(Box<HashSet<i64>>),
    /// Set of unsigned integers - preserves set semantics
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "serialize_uint_set",
            deserialize_with = "deserialize_uint_set"
        )
    )]
    UintSet(Box<HashSet<u64>>),
    /// Set of booleans - preserves set semantics
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "serialize_bool_set",
            deserialize_with = "deserialize_bool_set"
        )
    )]
    BoolSet(Box<HashSet<bool>>),

    // ==================== Map Target Wrappers ====================
    //
    // These variants encode the target (key access, keys, values)
    // along with the comparison value for HashMap/BTreeMap field operations.
    /// Access value at specific key: `metadata["author"] == "Alice"`
    /// Contains (key, value_to_compare)
    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "serialize_at_key",
            deserialize_with = "deserialize_at_key"
        )
    )]
    AtKey(Box<str>, Box<Value>),

    /// Match against map keys: `metadata.@keys CONTAINS "author"`
    /// Contains the value to compare against keys
    Keys(Box<Value>),

    /// Match against map values: `metadata.@values ANY OF ["v1", "v2"]`
    /// Contains the value to compare against values
    Values(Box<Value>),
}

impl Value {
    // ==================== Map Target Constructors ====================

    /// Create an AtKey wrapper for accessing a specific map key.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::Value;
    ///
    /// // Query: metadata["author"] == "Alice"
    /// let v = Value::at_key("author", "Alice");
    /// ```
    pub fn at_key(key: impl Into<Box<str>>, value: impl Into<Value>) -> Self {
        Value::AtKey(key.into(), Box::new(value.into()))
    }

    /// Create a Keys wrapper for matching against map keys.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::Value;
    ///
    /// // Query: metadata.@keys CONTAINS "author"
    /// let v = Value::keys("author");
    /// ```
    pub fn keys(value: impl Into<Value>) -> Self {
        Value::Keys(Box::new(value.into()))
    }

    /// Create a Values wrapper for matching against map values.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dnf::Value;
    ///
    /// // Query: metadata.@values ANY OF ["v1", "v2"]
    /// let v = Value::values(vec!["v1", "v2"]);
    /// ```
    pub fn values(value: impl Into<Value>) -> Self {
        Value::Values(Box::new(value.into()))
    }

    // ==================== Set Constructors ====================
    //
    // Use these for ALL OF / ANY OF operations to get O(1) lookup performance.
    // Arrays use O(n) linear search, Sets use O(1) HashSet lookup.

    /// Create a StringSet for efficient ALL OF / ANY OF operations.
    ///
    /// Uses HashSet for O(1) lookup instead of O(n) array search.
    pub fn string_set<I, S>(values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let set: HashSet<Box<str>> = values.into_iter().map(|s| Box::from(s.as_ref())).collect();
        Value::StringSet(Box::new(set))
    }

    /// Create an IntSet for efficient ALL OF / ANY OF operations.
    ///
    /// Uses HashSet for O(1) lookup instead of O(n) array search.
    pub fn int_set<I>(values: I) -> Self
    where
        I: IntoIterator<Item = i64>,
    {
        let set: HashSet<i64> = values.into_iter().collect();
        Value::IntSet(Box::new(set))
    }

    /// Create a UintSet for efficient ALL OF / ANY OF operations.
    ///
    /// Uses HashSet for O(1) lookup instead of O(n) array search.
    pub fn uint_set<I>(values: I) -> Self
    where
        I: IntoIterator<Item = u64>,
    {
        let set: HashSet<u64> = values.into_iter().collect();
        Value::UintSet(Box::new(set))
    }

    /// Create a BoolSet for efficient ALL OF / ANY OF operations.
    ///
    /// Uses HashSet for O(1) lookup instead of O(n) array search.
    pub fn bool_set<I>(values: I) -> Self
    where
        I: IntoIterator<Item = bool>,
    {
        let set: HashSet<bool> = values.into_iter().collect();
        Value::BoolSet(Box::new(set))
    }

    /// Convert value to string representation for string operations.
    ///
    /// This is used internally for string operators (CONTAINS, STARTS WITH, ENDS WITH)
    /// when applied to non-string values.
    ///
    /// Returns `Cow<'_, str>` to avoid allocations when the value is already a string.
    ///
    /// Note: `Value::None` returns an empty string `""` for string operations, not `"null"`.
    /// This distinguishes between the absence of a value (None) and the literal string "null".
    /// The Display trait still shows "null" for user-facing output.
    pub(crate) fn to_string_repr(&self) -> Cow<'_, str> {
        match self {
            Value::String(s) => Cow::Borrowed(s.as_ref()),
            Value::Int(i) => Cow::Owned(i.to_string()),
            Value::Uint(u) => Cow::Owned(u.to_string()),
            Value::Float(f) => Cow::Owned(f.to_string()),
            Value::Bool(b) => Cow::Owned(b.to_string()),
            Value::None => Cow::Borrowed(""),
            // For arrays, sets, and map wrappers, delegate to Display trait
            Value::StringArray(_)
            | Value::IntArray(_)
            | Value::UintArray(_)
            | Value::FloatArray(_)
            | Value::BoolArray(_)
            | Value::StringSet(_)
            | Value::IntSet(_)
            | Value::UintSet(_)
            | Value::BoolSet(_)
            | Value::AtKey(_, _)
            | Value::Keys(_)
            | Value::Values(_) => Cow::Owned(self.to_string()),
        }
    }
}

//
// Only implement T == Value (not Value == T) since operator code only uses
// `field == query_value` direction. Users build queries with Value objects
// and compare against primitive fields.

// String types
impl PartialEq<Value> for str {
    #[inline]
    fn eq(&self, other: &Value) -> bool {
        match other {
            Value::String(s) => self == s.as_ref(),
            _ => self == other.to_string_repr().as_ref(),
        }
    }
}

impl PartialEq<Value> for &str {
    #[inline]
    fn eq(&self, other: &Value) -> bool {
        (*self).eq(other)
    }
}

impl PartialEq<Value> for String {
    #[inline]
    fn eq(&self, other: &Value) -> bool {
        self.as_str().eq(other)
    }
}

impl PartialEq<Value> for Box<str> {
    #[inline]
    fn eq(&self, other: &Value) -> bool {
        match other {
            Value::String(s) => self == s,
            _ => self.as_ref() == other.to_string_repr().as_ref(),
        }
    }
}

impl PartialEq<Value> for Cow<'_, str> {
    #[inline]
    fn eq(&self, other: &Value) -> bool {
        match other {
            Value::String(s) => self.as_ref() == s.as_ref(),
            _ => self.as_ref() == other.to_string_repr().as_ref(),
        }
    }
}

// Numeric types
impl PartialEq<Value> for i64 {
    #[inline]
    fn eq(&self, other: &Value) -> bool {
        match other {
            Value::Int(n) => self == n,
            Value::Uint(u) => *self >= 0 && *self as u64 == *u,
            Value::Float(f) => (*self as f64 - f).abs() < f64::EPSILON,
            Value::String(s) => self.to_string() == s.as_ref(),
            _ => false,
        }
    }
}

impl PartialEq<Value> for u64 {
    #[inline]
    fn eq(&self, other: &Value) -> bool {
        match other {
            Value::Uint(u) => self == u,
            Value::Int(n) => *n >= 0 && *self == *n as u64,
            Value::Float(f) => (*self as f64 - f).abs() < f64::EPSILON,
            Value::String(s) => self.to_string() == s.as_ref(),
            _ => false,
        }
    }
}

impl PartialEq<Value> for f64 {
    #[inline]
    fn eq(&self, other: &Value) -> bool {
        match other {
            Value::Float(f) => (self - f).abs() < f64::EPSILON,
            Value::Int(n) => (self - *n as f64).abs() < f64::EPSILON,
            Value::Uint(u) => (self - *u as f64).abs() < f64::EPSILON,
            Value::String(s) => self.to_string() == s.as_ref(),
            _ => false,
        }
    }
}

impl PartialEq<Value> for bool {
    #[inline]
    fn eq(&self, other: &Value) -> bool {
        match other {
            Value::Bool(b) => self == b,
            Value::String(s) => s.as_ref() == if *self { "true" } else { "false" },
            _ => false,
        }
    }
}

// Unified macro for numeric PartialEq<Value> implementations
macro_rules! impl_partial_eq_numeric {
    ($($t:ty),+ => $cast:ty) => {
        $(
            impl PartialEq<Value> for $t {
                #[inline]
                fn eq(&self, other: &Value) -> bool {
                    (*self as $cast).eq(other)
                }
            }
        )+
    };
}

impl_partial_eq_numeric!(i32, i16, i8, isize => i64);
impl_partial_eq_numeric!(u32, u16, u8, usize => u64);

// f32 -> f64 comparison
impl PartialEq<Value> for f32 {
    #[inline]
    fn eq(&self, other: &Value) -> bool {
        (*self as f64).eq(other)
    }
}

// Implement PartialEq for custom equality logic
// Cross-type numeric comparisons handle precision carefully to avoid float conversion issues
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Uint(a), Value::Uint(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => (a - b).abs() < f64::EPSILON,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::None, Value::None) => true,
            (Value::StringArray(a), Value::StringArray(b)) => a == b,
            (Value::IntArray(a), Value::IntArray(b)) => a == b,
            (Value::UintArray(a), Value::UintArray(b)) => a == b,
            (Value::FloatArray(a), Value::FloatArray(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b.iter())
                        .all(|(x, y)| (x - y).abs() < f64::EPSILON)
            }
            (Value::BoolArray(a), Value::BoolArray(b)) => a == b,
            (Value::StringSet(a), Value::StringSet(b)) => a == b,
            (Value::IntSet(a), Value::IntSet(b)) => a == b,
            (Value::UintSet(a), Value::UintSet(b)) => a == b,
            (Value::BoolSet(a), Value::BoolSet(b)) => a == b,
            // Map target wrappers
            (Value::AtKey(k1, v1), Value::AtKey(k2, v2)) => k1 == k2 && v1 == v2,
            (Value::Keys(v1), Value::Keys(v2)) => v1 == v2,
            (Value::Values(v1), Value::Values(v2)) => v1 == v2,
            // Cross-type numeric comparisons - handle carefully to avoid precision loss
            (Value::Int(a), Value::Uint(b)) => {
                // If int is negative, they can't be equal
                if *a < 0 {
                    false
                } else {
                    // Compare as u64 if possible
                    *a as u64 == *b
                }
            }
            (Value::Uint(a), Value::Int(b)) => {
                if *b < 0 {
                    false
                } else {
                    *a == *b as u64
                }
            }
            // Int/Uint ↔ Float comparisons
            // WARNING: Precision loss for integers outside [-2^53, 2^53] range.
            // See MAX_SAFE_INTEGER_FOR_FLOAT for details.
            (Value::Int(a), Value::Float(b)) => (*a as f64 - b).abs() < f64::EPSILON,
            (Value::Float(a), Value::Int(b)) => (a - *b as f64).abs() < f64::EPSILON,
            (Value::Uint(a), Value::Float(b)) => (*a as f64 - b).abs() < f64::EPSILON,
            (Value::Float(a), Value::Uint(b)) => (a - *b as f64).abs() < f64::EPSILON,
            // String to non-string comparisons - convert both to string
            (Value::String(a), other) => a.as_ref() == other.to_string_repr(),
            (other, Value::String(b)) => other.to_string_repr() == b.as_ref(),
            _ => false,
        }
    }
}

//
// Only implement T <=> Value (not Value <=> T) since operator code only uses
// `field > query_value` direction.

impl PartialOrd<Value> for i64 {
    #[inline]
    fn partial_cmp(&self, value: &Value) -> Option<Ordering> {
        match value {
            Value::Int(n) => self.partial_cmp(n),
            Value::Uint(u) => {
                if *self < 0 {
                    Some(Ordering::Less)
                } else {
                    (*self as u64).partial_cmp(u)
                }
            }
            Value::Float(f) => {
                let self_f = *self as f64;
                if (self_f - f).abs() < f64::EPSILON {
                    Some(Ordering::Equal)
                } else {
                    self_f.partial_cmp(f)
                }
            }
            _ => None,
        }
    }
}

impl PartialOrd<Value> for u64 {
    #[inline]
    fn partial_cmp(&self, value: &Value) -> Option<Ordering> {
        match value {
            Value::Uint(u) => self.partial_cmp(u),
            Value::Int(n) => {
                if *n < 0 {
                    Some(Ordering::Greater)
                } else {
                    self.partial_cmp(&(*n as u64))
                }
            }
            Value::Float(f) => {
                let self_f = *self as f64;
                if (self_f - f).abs() < f64::EPSILON {
                    Some(Ordering::Equal)
                } else {
                    self_f.partial_cmp(f)
                }
            }
            _ => None,
        }
    }
}

impl PartialOrd<Value> for f64 {
    #[inline]
    fn partial_cmp(&self, value: &Value) -> Option<Ordering> {
        match value {
            Value::Float(f) => {
                if (self - f).abs() < f64::EPSILON {
                    Some(Ordering::Equal)
                } else {
                    self.partial_cmp(f)
                }
            }
            Value::Int(n) => {
                let other_f = *n as f64;
                if (self - other_f).abs() < f64::EPSILON {
                    Some(Ordering::Equal)
                } else {
                    self.partial_cmp(&other_f)
                }
            }
            Value::Uint(u) => {
                let other_f = *u as f64;
                if (self - other_f).abs() < f64::EPSILON {
                    Some(Ordering::Equal)
                } else {
                    self.partial_cmp(&other_f)
                }
            }
            _ => None,
        }
    }
}

// Implement PartialOrd for ordering comparisons
// Cross-type numeric comparisons handle precision carefully to avoid float conversion issues
impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Value::String(a), Value::String(b)) => a.partial_cmp(b),
            (Value::Int(a), Value::Int(b)) => a.partial_cmp(b),
            (Value::Uint(a), Value::Uint(b)) => a.partial_cmp(b),
            (Value::Float(a), Value::Float(b)) => {
                // Handle float comparison with epsilon for equality
                let diff = a - b;
                if diff.abs() < f64::EPSILON {
                    Some(Ordering::Equal)
                } else {
                    a.partial_cmp(b)
                }
            }
            (Value::Bool(a), Value::Bool(b)) => a.partial_cmp(b),
            (Value::None, Value::None) => Some(Ordering::Equal),
            // Array comparisons - lexicographic ordering
            (Value::StringArray(a), Value::StringArray(b)) => a.partial_cmp(b),
            (Value::IntArray(a), Value::IntArray(b)) => a.partial_cmp(b),
            (Value::UintArray(a), Value::UintArray(b)) => a.partial_cmp(b),
            (Value::FloatArray(a), Value::FloatArray(b)) => {
                // Lexicographic comparison with epsilon for floats
                for (x, y) in a.iter().zip(b.iter()) {
                    let diff = x - y;
                    if diff.abs() >= f64::EPSILON {
                        return x.partial_cmp(y);
                    }
                }
                a.len().partial_cmp(&b.len())
            }
            (Value::BoolArray(a), Value::BoolArray(b)) => a.partial_cmp(b),
            // Set comparisons - sets are unordered, so ordering doesn't make sense
            // Only return Equal if sets are equal, otherwise None
            (Value::StringSet(a), Value::StringSet(b)) => {
                if a == b {
                    Some(Ordering::Equal)
                } else {
                    None
                }
            }
            (Value::IntSet(a), Value::IntSet(b)) => {
                if a == b {
                    Some(Ordering::Equal)
                } else {
                    None
                }
            }
            (Value::UintSet(a), Value::UintSet(b)) => {
                if a == b {
                    Some(Ordering::Equal)
                } else {
                    None
                }
            }
            (Value::BoolSet(a), Value::BoolSet(b)) => {
                if a == b {
                    Some(Ordering::Equal)
                } else {
                    None
                }
            }
            // Map target wrappers - only equal if same type and contents match
            (Value::AtKey(k1, v1), Value::AtKey(k2, v2)) => {
                if k1 == k2 {
                    v1.partial_cmp(v2)
                } else {
                    k1.as_ref().partial_cmp(k2.as_ref())
                }
            }
            (Value::Keys(v1), Value::Keys(v2)) => v1.partial_cmp(v2),
            (Value::Values(v1), Value::Values(v2)) => v1.partial_cmp(v2),
            // Cross-type numeric comparisons - handle carefully to avoid precision loss
            (Value::Int(a), Value::Uint(b)) => {
                // Negative int is always less than any uint
                if *a < 0 {
                    Some(Ordering::Less)
                } else {
                    // Compare as u64 if possible
                    (*a as u64).partial_cmp(b)
                }
            }
            (Value::Uint(a), Value::Int(b)) => {
                // Any uint is greater than negative int
                if *b < 0 {
                    Some(Ordering::Greater)
                } else {
                    a.partial_cmp(&(*b as u64))
                }
            }
            // Int/Uint ↔ Float comparisons
            // WARNING: Precision loss for integers outside [-2^53, 2^53] range.
            // Large integers may compare incorrectly with floats.
            // See MAX_SAFE_INTEGER_FOR_FLOAT for details.
            (Value::Int(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
            (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f64)),
            (Value::Uint(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
            (Value::Float(a), Value::Uint(b)) => a.partial_cmp(&(*b as f64)),
            // String to non-string comparisons - use string comparison
            (Value::String(a), other) => {
                let other_str = other.to_string_repr();
                a.as_ref().partial_cmp(other_str.as_ref())
            }
            (other, Value::String(b)) => {
                let other_str = other.to_string_repr();
                other_str.as_ref().partial_cmp(b.as_ref())
            }
            // Different types that can't be compared
            _ => None,
        }
    }
}

/// Escape a string for use in query syntax.
/// Escapes: backslash, double quote, newline, tab, carriage return, forward slash.
fn escape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\t' => result.push_str("\\t"),
            '\r' => result.push_str("\\r"),
            '/' => result.push_str("\\/"),
            _ => result.push(ch),
        }
    }
    result
}

// Helper macro for Display implementation of collection types
macro_rules! fmt_collection {
    // Quoted variant (for strings)
    ($f:expr, $iter:expr, quoted) => {{
        write!($f, "[")?;
        for (i, val) in $iter.enumerate() {
            if i > 0 {
                write!($f, ", ")?;
            }
            write!($f, "\"{}\"", escape_string(val.as_ref()))?;
        }
        write!($f, "]")
    }};
    // Unquoted variant (for numbers, bools)
    ($f:expr, $iter:expr) => {{
        write!($f, "[")?;
        for (i, val) in $iter.enumerate() {
            if i > 0 {
                write!($f, ", ")?;
            }
            write!($f, "{}", val)?;
        }
        write!($f, "]")
    }};
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::String(s) => write!(f, "\"{}\"", escape_string(s)),
            Value::Int(i) => write!(f, "{}", i),
            Value::Uint(u) => write!(f, "{}", u),
            Value::Float(fl) => write!(f, "{}", fl),
            Value::Bool(b) => write!(f, "{}", b),
            Value::None => write!(f, "null"),
            // Arrays
            Value::StringArray(arr) => fmt_collection!(f, arr.iter(), quoted),
            Value::IntArray(arr) => fmt_collection!(f, arr.iter()),
            Value::UintArray(arr) => fmt_collection!(f, arr.iter()),
            Value::FloatArray(arr) => fmt_collection!(f, arr.iter()),
            Value::BoolArray(arr) => fmt_collection!(f, arr.iter()),
            // Sets
            Value::StringSet(set) => fmt_collection!(f, set.iter(), quoted),
            Value::IntSet(set) => fmt_collection!(f, set.iter()),
            Value::UintSet(set) => fmt_collection!(f, set.iter()),
            Value::BoolSet(set) => fmt_collection!(f, set.iter()),
            // Map target wrappers
            Value::AtKey(key, value) => write!(f, "@[\"{}\"]:{}", escape_string(key), value),
            Value::Keys(value) => write!(f, "@keys:{}", value),
            Value::Values(value) => write!(f, "@values:{}", value),
        }
    }
}

//
// Conversion implementations for convenient value creation.
// Uses macros to reduce boilerplate for numeric types.

/// Macro for `From<T>` -> Value
macro_rules! impl_from_owned {
    // Direct: $ty => $variant (no casting)
    ($($ty:ty => $variant:ident),+ $(,)?) => {
        $(
            impl From<$ty> for Value {
                fn from(val: $ty) -> Self {
                    Value::$variant(val)
                }
            }
        )+
    };
    // Cast: $types => $target, $variant
    ($($src_ty:ty),+ => $target_ty:ty, $variant:ident) => {
        $(
            impl From<$src_ty> for Value {
                fn from(val: $src_ty) -> Self {
                    Value::$variant(val as $target_ty)
                }
            }
        )+
    };
}

// String conversions
impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s.into_boxed_str())
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(Box::from(s))
    }
}

impl From<Box<str>> for Value {
    fn from(s: Box<str>) -> Self {
        Value::String(s)
    }
}

impl From<Cow<'_, str>> for Value {
    fn from(s: Cow<'_, str>) -> Self {
        Value::String(s.into_owned().into_boxed_str())
    }
}

// Direct (no cast needed)
impl_from_owned!(i64 => Int, u64 => Uint, f64 => Float, bool => Bool);

// Cast to target type
impl_from_owned!(i32, i16, i8, isize => i64, Int);
impl_from_owned!(u32, u16, u8, usize => u64, Uint);
impl_from_owned!(f32 => f64, Float);

//
// These macros reduce boilerplate for converting collections to Value.

/// Macro for `Vec<T>` -> Value
macro_rules! impl_from_vec {
    // Direct: $ty => $variant (no casting)
    ($($ty:ty => $variant:ident),+ $(,)?) => {
        $(
            impl From<Vec<$ty>> for Value {
                fn from(v: Vec<$ty>) -> Self {
                    Value::$variant(v.into_boxed_slice())
                }
            }
        )+
    };
    // Cast: $types => $target, $variant
    ($($src_ty:ty),+ => $target_ty:ty, $variant:ident) => {
        $(
            impl From<Vec<$src_ty>> for Value {
                fn from(v: Vec<$src_ty>) -> Self {
                    let values: Vec<$target_ty> = v.into_iter().map(|x| x as $target_ty).collect();
                    Value::$variant(values.into_boxed_slice())
                }
            }
        )+
    };
}

/// Macro for `&Vec<T>` -> Value
macro_rules! impl_from_vec_ref {
    // Direct: $ty => $variant (no casting)
    ($($ty:ty => $variant:ident),+ $(,)?) => {
        $(
            impl From<&Vec<$ty>> for Value {
                fn from(v: &Vec<$ty>) -> Self {
                    Value::$variant(Box::from(v.as_slice()))
                }
            }
        )+
    };
    // Cast: $types => $target, $variant
    ($($src_ty:ty),+ => $target_ty:ty, $variant:ident) => {
        $(
            impl From<&Vec<$src_ty>> for Value {
                fn from(v: &Vec<$src_ty>) -> Self {
                    let values: Vec<$target_ty> = v.iter().map(|&x| x as $target_ty).collect();
                    Value::$variant(values.into_boxed_slice())
                }
            }
        )+
    };
}

/// Macro for `&[T]` -> Value
macro_rules! impl_from_slice {
    // Direct: $ty => $variant (no casting)
    ($($ty:ty => $variant:ident),+ $(,)?) => {
        $(
            impl From<&[$ty]> for Value {
                fn from(s: &[$ty]) -> Self {
                    Value::$variant(Box::from(s))
                }
            }
        )+
    };
    // Cast: $types => $target, $variant
    ($($src_ty:ty),+ => $target_ty:ty, $variant:ident) => {
        $(
            impl From<&[$src_ty]> for Value {
                fn from(s: &[$src_ty]) -> Self {
                    let values: Vec<$target_ty> = s.iter().map(|&x| x as $target_ty).collect();
                    Value::$variant(values.into_boxed_slice())
                }
            }
        )+
    };
}

// String is special - needs Box<str> conversion

impl From<Vec<String>> for Value {
    fn from(v: Vec<String>) -> Self {
        let box_strs: Vec<Box<str>> = v.into_iter().map(|s| s.into_boxed_str()).collect();
        Value::StringArray(box_strs.into_boxed_slice())
    }
}

impl From<Vec<&str>> for Value {
    fn from(v: Vec<&str>) -> Self {
        let box_strs: Vec<Box<str>> = v.into_iter().map(Box::from).collect();
        Value::StringArray(box_strs.into_boxed_slice())
    }
}

impl From<Vec<Box<str>>> for Value {
    fn from(v: Vec<Box<str>>) -> Self {
        Value::StringArray(v.into_boxed_slice())
    }
}

// Direct (no cast needed)
impl_from_vec!(i64 => IntArray, u64 => UintArray, f64 => FloatArray, bool => BoolArray);

// Cast to target type
impl_from_vec!(i32, i16, i8, isize => i64, IntArray);
impl_from_vec!(u32, u16, u8, usize => u64, UintArray);
impl_from_vec!(f32 => f64, FloatArray);

//
// These implementations allow converting from borrowed Vec<T> and slices
// without cloning the original collection. They iterate and create Box-wrapped
// values directly, avoiding the intermediate clone.
//
// Used by the derive macro for efficient field evaluation:
// - `Value::from(&self.tags)` instead of `Value::from(self.tags.clone())`
// - Eliminates one allocation (the clone)

// String is special - needs Box<str> conversion
impl From<&Vec<String>> for Value {
    fn from(v: &Vec<String>) -> Self {
        let box_strs: Vec<Box<str>> = v.iter().map(|s| Box::from(s.as_str())).collect();
        Value::StringArray(box_strs.into_boxed_slice())
    }
}

// Direct (no cast needed)
impl_from_vec_ref!(i64 => IntArray, u64 => UintArray, f64 => FloatArray, bool => BoolArray);

// Cast to target type
impl_from_vec_ref!(i32, i16, i8, isize => i64, IntArray);
impl_from_vec_ref!(u32, u16, u8, usize => u64, UintArray);
impl_from_vec_ref!(f32 => f64, FloatArray);

// String slices - special handling for Box<str>
impl From<&[String]> for Value {
    fn from(s: &[String]) -> Self {
        let box_strs: Vec<Box<str>> = s.iter().map(|s| Box::from(s.as_str())).collect();
        Value::StringArray(box_strs.into_boxed_slice())
    }
}

impl From<&[&str]> for Value {
    fn from(s: &[&str]) -> Self {
        let box_strs: Vec<Box<str>> = s.iter().map(|&s| Box::from(s)).collect();
        Value::StringArray(box_strs.into_boxed_slice())
    }
}

// Direct (no cast needed)
impl_from_slice!(i64 => IntArray, u64 => UintArray, f64 => FloatArray, bool => BoolArray);

// Cast to target type
impl_from_slice!(i32, i16, i8, isize => i64, IntArray);
impl_from_slice!(u32, u16, u8, usize => u64, UintArray);
impl_from_slice!(f32 => f64, FloatArray);

//
// HashSet<T> is converted to arrays by iterating over the set.
// Note: Order is not preserved (HashSet is unordered).
// Used for efficient set-based queries without cloning the original HashSet.

/// Macro for `HashSet<T>` and `&HashSet<T>` -> Value
macro_rules! impl_from_hashset {
    // Direct: $ty => $variant (no casting)
    ($($ty:ty => $variant:ident),+ $(,)?) => {
        $(
            impl From<&HashSet<$ty>> for Value {
                fn from(set: &HashSet<$ty>) -> Self {
                    let set_clone: HashSet<$ty> = set.iter().copied().collect();
                    Value::$variant(Box::new(set_clone))
                }
            }

            impl From<HashSet<$ty>> for Value {
                fn from(set: HashSet<$ty>) -> Self {
                    Value::$variant(Box::new(set))
                }
            }
        )+
    };
    // Cast: $types => $target, $variant
    ($($src_ty:ty),+ => $target_ty:ty, $variant:ident) => {
        $(
            impl From<&HashSet<$src_ty>> for Value {
                fn from(set: &HashSet<$src_ty>) -> Self {
                    let values: HashSet<$target_ty> = set.iter().map(|&v| v as $target_ty).collect();
                    Value::$variant(Box::new(values))
                }
            }

            impl From<HashSet<$src_ty>> for Value {
                fn from(set: HashSet<$src_ty>) -> Self {
                    let values: HashSet<$target_ty> = set.into_iter().map(|v| v as $target_ty).collect();
                    Value::$variant(Box::new(values))
                }
            }
        )+
    };
}

// String is special - needs Box conversion
impl From<&HashSet<String>> for Value {
    fn from(set: &HashSet<String>) -> Self {
        let box_strs: HashSet<Box<str>> = set.iter().map(|s| Box::from(s.as_str())).collect();
        Value::StringSet(Box::new(box_strs))
    }
}

impl From<HashSet<String>> for Value {
    fn from(set: HashSet<String>) -> Self {
        let box_strs: HashSet<Box<str>> = set.into_iter().map(|s| s.into_boxed_str()).collect();
        Value::StringSet(Box::new(box_strs))
    }
}

// Direct (no cast needed)
impl_from_hashset!(i64 => IntSet, u64 => UintSet, bool => BoolSet);

// Cast to target type
impl_from_hashset!(i32, i16, i8, isize => i64, IntSet);
impl_from_hashset!(u32, u16, u8, usize => u64, UintSet);

/// Macro for `From<&T>` -> Value
macro_rules! impl_from_ref {
    // Direct: $ty => $variant (no casting)
    ($($ty:ty => $variant:ident),+ $(,)?) => {
        $(
            impl From<&$ty> for Value {
                fn from(val: &$ty) -> Self {
                    Value::$variant(*val)
                }
            }
        )+
    };
    // Cast: $types => $target, $variant
    ($($src_ty:ty),+ => $target_ty:ty, $variant:ident) => {
        $(
            impl From<&$src_ty> for Value {
                fn from(val: &$src_ty) -> Self {
                    Value::$variant(*val as $target_ty)
                }
            }
        )+
    };
}

// Direct (no cast needed)
impl_from_ref!(i64 => Int, u64 => Uint, f64 => Float, bool => Bool);

// Cast to target type
impl_from_ref!(i32, i16, i8, isize => i64, Int);
impl_from_ref!(u32, u16, u8, usize => u64, Uint);
impl_from_ref!(f32 => f64, Float);

// String (reference implementation)
impl From<&String> for Value {
    fn from(val: &String) -> Self {
        Value::String(Box::from(val.as_str()))
    }
}

// Cow<str> (reference implementation)
impl From<&Cow<'_, str>> for Value {
    fn from(val: &Cow<'_, str>) -> Self {
        Value::String(Box::from(val.as_ref()))
    }
}

// Note: Option<T> is handled directly in the derive macro to avoid
// recursive trait bound issues. Use `match` on the Option and call
// Value::from on the inner value when Some, Value::None when None.

//
// Custom serialize/deserialize functions for Box-wrapped types.
// Uses macros to reduce boilerplate.

// Serde helpers for Box<str> (single string, not array)
#[cfg(feature = "serde")]
#[allow(clippy::borrowed_box)]
fn serialize_box_str<S>(boxed: &Box<str>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(boxed.as_ref())
}

#[cfg(feature = "serde")]
fn deserialize_box_str<'de, D>(deserializer: D) -> Result<Box<str>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let s = String::deserialize(deserializer)?;
    Ok(s.into_boxed_str())
}

// Serde helpers for Box<[Box<str>]> (string array - special handling)
#[cfg(feature = "serde")]
#[allow(clippy::borrowed_box)]
fn serialize_box_str_slice<S>(boxed: &Box<[Box<str>]>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize;
    let strings: Vec<&str> = boxed.iter().map(|s| s.as_ref()).collect();
    strings.serialize(serializer)
}

#[cfg(feature = "serde")]
fn deserialize_box_str_slice<'de, D>(deserializer: D) -> Result<Box<[Box<str>]>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let strings = Vec::<String>::deserialize(deserializer)?;
    let box_strs: Vec<Box<str>> = strings.into_iter().map(|s| s.into_boxed_str()).collect();
    Ok(box_strs.into_boxed_slice())
}

// Serde helpers for Box<HashSet<Box<str>>> (string set - special handling)
#[cfg(feature = "serde")]
#[allow(clippy::borrowed_box)]
fn serialize_string_set<S>(set: &Box<HashSet<Box<str>>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize;
    let strings: Vec<&str> = set.iter().map(|s| s.as_ref()).collect();
    strings.serialize(serializer)
}

#[cfg(feature = "serde")]
#[allow(clippy::box_collection)]
fn deserialize_string_set<'de, D>(deserializer: D) -> Result<Box<HashSet<Box<str>>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let strings = Vec::<String>::deserialize(deserializer)?;
    let box_strs: HashSet<Box<str>> = strings.into_iter().map(|s| s.into_boxed_str()).collect();
    Ok(Box::new(box_strs))
}

/// Macro for generating serde helpers for `Box<[T]>` array types
#[cfg(feature = "serde")]
macro_rules! impl_serde_box_slice {
    ($ty:ty, $ser_name:ident, $de_name:ident) => {
        fn $ser_name<S>(boxed: &Box<[$ty]>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            use serde::Serialize;
            boxed.as_ref().serialize(serializer)
        }

        fn $de_name<'de, D>(deserializer: D) -> Result<Box<[$ty]>, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::Deserialize;
            let vec = Vec::<$ty>::deserialize(deserializer)?;
            Ok(vec.into_boxed_slice())
        }
    };
}

/// Macro for generating serde helpers for `Box<HashSet<T>>` set types
#[cfg(feature = "serde")]
macro_rules! impl_serde_box_hashset {
    ($ty:ty, $ser_name:ident, $de_name:ident) => {
        fn $ser_name<S>(set: &Box<HashSet<$ty>>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            use serde::Serialize;
            set.as_ref().serialize(serializer)
        }

        fn $de_name<'de, D>(deserializer: D) -> Result<Box<HashSet<$ty>>, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::Deserialize;
            let set = HashSet::<$ty>::deserialize(deserializer)?;
            Ok(Box::new(set))
        }
    };
}

// Generate serde helpers for array types
#[cfg(feature = "serde")]
impl_serde_box_slice!(i64, serialize_box_i64_slice, deserialize_box_i64_slice);
#[cfg(feature = "serde")]
impl_serde_box_slice!(u64, serialize_box_u64_slice, deserialize_box_u64_slice);
#[cfg(feature = "serde")]
impl_serde_box_slice!(f64, serialize_box_f64_slice, deserialize_box_f64_slice);
#[cfg(feature = "serde")]
impl_serde_box_slice!(bool, serialize_box_bool_slice, deserialize_box_bool_slice);

// Generate serde helpers for set types
#[cfg(feature = "serde")]
impl_serde_box_hashset!(i64, serialize_int_set, deserialize_int_set);
#[cfg(feature = "serde")]
impl_serde_box_hashset!(u64, serialize_uint_set, deserialize_uint_set);
#[cfg(feature = "serde")]
impl_serde_box_hashset!(bool, serialize_bool_set, deserialize_bool_set);

// AtKey: (Box<str>, Box<Value>) -> { "key": "...", "value": ... }
#[cfg(feature = "serde")]
#[allow(clippy::borrowed_box)]
fn serialize_at_key<S>(key: &Box<str>, value: &Value, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeStruct;
    let mut s = serializer.serialize_struct("AtKey", 2)?;
    s.serialize_field("key", key.as_ref())?;
    s.serialize_field("inner", value)?;
    s.end()
}

#[cfg(feature = "serde")]
fn deserialize_at_key<'de, D>(deserializer: D) -> Result<(Box<str>, Box<Value>), D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    #[derive(Deserialize)]
    struct AtKeyHelper {
        key: String,
        inner: Value,
    }
    let helper = AtKeyHelper::deserialize(deserializer)?;
    Ok((helper.key.into_boxed_str(), Box::new(helper.inner)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("simple"), "simple");
        assert_eq!(escape_string("with \"quotes\""), "with \\\"quotes\\\"");
        assert_eq!(escape_string("with\\backslash"), "with\\\\backslash");
        assert_eq!(escape_string("with\nnewline"), "with\\nnewline");
        assert_eq!(escape_string("with\ttab"), "with\\ttab");
        assert_eq!(escape_string("with/slash"), "with\\/slash");
        assert_eq!(
            escape_string("C:\\Path\\To\\File"),
            "C:\\\\Path\\\\To\\\\File"
        );
        assert_eq!(
            escape_string("He said \"Hello\"\nNext line"),
            "He said \\\"Hello\\\"\\nNext line"
        );
    }

    #[test]
    fn test_value_display_with_escapes() {
        // String with quotes
        let val = Value::String(Box::from("He said \"Hello\""));
        assert_eq!(val.to_string(), r#""He said \"Hello\"""#);

        // String with backslashes
        let val = Value::String(Box::from("C:\\Users\\Test"));
        assert_eq!(val.to_string(), r#""C:\\Users\\Test""#);

        // String with newlines and tabs
        let val = Value::String(Box::from("Line1\nLine2\tTabbed"));
        assert_eq!(val.to_string(), r#""Line1\nLine2\tTabbed""#);

        // String with forward slash
        let val = Value::String(Box::from("https://example.com"));
        assert_eq!(val.to_string(), r#""https:\/\/example.com""#);

        // String array with special characters
        let val = Value::from(vec![
            "simple".to_string(),
            "with \"quotes\"".to_string(),
            "with\\slash".to_string(),
        ]);
        assert_eq!(
            val.to_string(),
            r#"["simple", "with \"quotes\"", "with\\slash"]"#
        );
    }

    // ==================== Data-Driven Equality Tests ====================

    #[test]
    fn test_same_type_equality() {
        let test_cases = vec![
            // (left, right, expected_eq, description)
            (Value::Int(42), Value::Int(42), true, "int == int"),
            (Value::Int(42), Value::Int(43), false, "int != int"),
            (Value::Uint(42), Value::Uint(42), true, "uint == uint"),
            (
                Value::Float(3.34),
                Value::Float(3.34),
                true,
                "float == float",
            ),
            (Value::Bool(true), Value::Bool(true), true, "bool == bool"),
            (Value::Bool(true), Value::Bool(false), false, "bool != bool"),
            (
                Value::from("hello"),
                Value::from("hello"),
                true,
                "str == str",
            ),
            (
                Value::from("hello"),
                Value::from("world"),
                false,
                "str != str",
            ),
            (Value::None, Value::None, true, "none == none"),
        ];

        for (left, right, expected, desc) in test_cases {
            assert_eq!(left == right, expected, "Failed: {}", desc);
        }
    }

    #[test]
    fn test_cross_type_equality() {
        let test_cases = vec![
            // Int <-> Uint
            (
                Value::Int(42),
                Value::Uint(42),
                true,
                "positive int == uint",
            ),
            (
                Value::Uint(42),
                Value::Int(42),
                true,
                "uint == positive int",
            ),
            (
                Value::Int(-1),
                Value::Uint(0),
                false,
                "negative int != uint",
            ),
            (
                Value::Int(-42),
                Value::Uint(42),
                false,
                "negative int != uint (same abs)",
            ),
            // Int <-> Float
            (Value::Int(42), Value::Float(42.0), true, "int == float"),
            (Value::Float(42.0), Value::Int(42), true, "float == int"),
            // Uint <-> Float
            (Value::Uint(42), Value::Float(42.0), true, "uint == float"),
            (Value::Float(42.0), Value::Uint(42), true, "float == uint"),
            // String <-> numeric (uses string representation)
            (
                Value::from("42"),
                Value::Int(42),
                true,
                "string '42' == int 42",
            ),
            (
                Value::from("42"),
                Value::Uint(42),
                true,
                "string '42' == uint 42",
            ),
            (
                Value::from("3.34"),
                Value::Float(3.34),
                true,
                "string == float (representation)",
            ),
            (
                Value::from("true"),
                Value::Bool(true),
                true,
                "string 'true' == bool true",
            ),
            (
                Value::from("false"),
                Value::Bool(false),
                true,
                "string 'false' == bool false",
            ),
            // None comparisons
            (Value::None, Value::Int(0), false, "none != int"),
            (Value::None, Value::Bool(false), false, "none != bool"),
        ];

        for (left, right, expected, desc) in test_cases {
            assert_eq!(left == right, expected, "Failed: {}", desc);
        }
    }

    // ==================== Data-Driven Ordering Tests ====================

    #[test]
    fn test_same_type_ordering() {
        let test_cases = vec![
            // (left, right, expected_ordering, description)
            (
                Value::Int(10),
                Value::Int(5),
                Some(Ordering::Greater),
                "int > int",
            ),
            (
                Value::Int(5),
                Value::Int(10),
                Some(Ordering::Less),
                "int < int",
            ),
            (
                Value::Int(5),
                Value::Int(5),
                Some(Ordering::Equal),
                "int == int",
            ),
            (
                Value::Uint(10),
                Value::Uint(5),
                Some(Ordering::Greater),
                "uint > uint",
            ),
            (
                Value::Float(10.5),
                Value::Float(5.5),
                Some(Ordering::Greater),
                "float > float",
            ),
            (
                Value::from("b"),
                Value::from("a"),
                Some(Ordering::Greater),
                "str > str",
            ),
            (
                Value::from("a"),
                Value::from("b"),
                Some(Ordering::Less),
                "str < str",
            ),
            (
                Value::Bool(true),
                Value::Bool(false),
                Some(Ordering::Greater),
                "true > false",
            ),
            (
                Value::None,
                Value::None,
                Some(Ordering::Equal),
                "none == none",
            ),
        ];

        for (left, right, expected, desc) in test_cases {
            assert_eq!(left.partial_cmp(&right), expected, "Failed: {}", desc);
        }
    }

    #[test]
    fn test_cross_type_ordering() {
        let test_cases = vec![
            // Int <-> Uint
            (
                Value::Int(-1),
                Value::Uint(0),
                Some(Ordering::Less),
                "negative int < uint",
            ),
            (
                Value::Int(-1000),
                Value::Uint(1),
                Some(Ordering::Less),
                "negative int < any uint",
            ),
            (
                Value::Int(10),
                Value::Uint(5),
                Some(Ordering::Greater),
                "positive int > smaller uint",
            ),
            (
                Value::Int(5),
                Value::Uint(10),
                Some(Ordering::Less),
                "positive int < larger uint",
            ),
            (
                Value::Int(42),
                Value::Uint(42),
                Some(Ordering::Equal),
                "int == uint (same value)",
            ),
            (
                Value::Uint(0),
                Value::Int(-1),
                Some(Ordering::Greater),
                "uint > negative int",
            ),
            // Int <-> Float
            (
                Value::Int(10),
                Value::Float(5.5),
                Some(Ordering::Greater),
                "int > float",
            ),
            (
                Value::Float(10.5),
                Value::Int(10),
                Some(Ordering::Greater),
                "float > int",
            ),
            // Uint <-> Float
            (
                Value::Uint(10),
                Value::Float(5.5),
                Some(Ordering::Greater),
                "uint > float",
            ),
            (
                Value::Float(10.5),
                Value::Uint(10),
                Some(Ordering::Greater),
                "float > uint",
            ),
            // None with other types
            (
                Value::None,
                Value::Int(0),
                None,
                "none vs int: incomparable",
            ),
            (
                Value::Int(0),
                Value::None,
                None,
                "int vs none: incomparable",
            ),
        ];

        for (left, right, expected, desc) in test_cases {
            assert_eq!(left.partial_cmp(&right), expected, "Failed: {}", desc);
        }
    }

    // ==================== Large Number Precision Tests ====================

    #[test]
    fn test_large_number_precision() {
        const MAX_SAFE_INT: u64 = 9_007_199_254_740_992; // 2^53
        const LARGE_VAL: u64 = 1u64 << 54;

        let test_cases = vec![
            // Same type comparisons maintain precision
            (
                Value::Uint(LARGE_VAL + 1),
                Value::Uint(LARGE_VAL + 1),
                true,
                "large uint == large uint",
            ),
            (
                Value::Uint(LARGE_VAL + 1),
                Value::Uint(LARGE_VAL + 2),
                false,
                "large uint != different large uint",
            ),
            // Cross-type with large values
            (
                Value::Int(MAX_SAFE_INT as i64 + 1),
                Value::Uint(MAX_SAFE_INT + 1),
                true,
                "large int == large uint",
            ),
            // Boundary values
            (
                Value::Int(i64::MAX),
                Value::Uint(i64::MAX as u64),
                true,
                "max i64 == uint",
            ),
            (
                Value::Int(i64::MIN),
                Value::Uint(0),
                false,
                "min i64 != uint 0",
            ),
        ];

        for (left, right, expected, desc) in test_cases {
            assert_eq!(left == right, expected, "Failed: {}", desc);
        }

        // Ordering with large values
        assert_eq!(
            Value::Uint(MAX_SAFE_INT + 1).partial_cmp(&Value::Uint(MAX_SAFE_INT + 3)),
            Some(Ordering::Less),
            "Large uint ordering preserved"
        );

        // Max values ordering
        assert!(
            Value::Uint(u64::MAX) > Value::Int(i64::MAX),
            "max u64 > max i64"
        );
        assert!(Value::Int(i64::MIN) < Value::Uint(0), "min i64 < uint 0");
    }

    // ==================== Array Tests (Data-Driven) ====================

    #[test]
    fn test_array_from_conversions() {
        // Test that various types correctly convert to the expected array variant
        assert!(matches!(Value::from(vec!["a", "b"]), Value::StringArray(_)));
        assert!(matches!(
            Value::from(vec!["hello".to_string()]),
            Value::StringArray(_)
        ));
        assert!(matches!(Value::from(vec![1i64, 2]), Value::IntArray(_)));
        assert!(matches!(Value::from(vec![1i32, 2]), Value::IntArray(_)));
        assert!(matches!(Value::from(vec![1u64, 2]), Value::UintArray(_)));
        assert!(matches!(Value::from(vec![1u32, 2]), Value::UintArray(_)));
        assert!(matches!(
            Value::from(vec![1.0f64, 2.0]),
            Value::FloatArray(_)
        ));
        assert!(matches!(
            Value::from(vec![1.0f32, 2.0]),
            Value::FloatArray(_)
        ));
        assert!(matches!(
            Value::from(vec![true, false]),
            Value::BoolArray(_)
        ));
    }

    #[test]
    fn test_array_equality() {
        let test_cases: Vec<(Value, Value, bool, &str)> = vec![
            (
                Value::from(vec!["a", "b"]),
                Value::from(vec!["a", "b"]),
                true,
                "string arrays equal",
            ),
            (
                Value::from(vec!["a", "b"]),
                Value::from(vec!["a", "c"]),
                false,
                "string arrays differ",
            ),
            (
                Value::from(vec![1i64, 2]),
                Value::from(vec![1i64, 2]),
                true,
                "int arrays equal",
            ),
            (
                Value::from(vec![1i64, 2]),
                Value::from(vec![1i64, 3]),
                false,
                "int arrays differ",
            ),
            (
                Value::from(vec![1u64, 2]),
                Value::from(vec![1u64, 2]),
                true,
                "uint arrays equal",
            ),
            (
                Value::from(vec![1.0, 2.0]),
                Value::from(vec![1.0, 2.0]),
                true,
                "float arrays equal",
            ),
            (
                Value::from(vec![true, false]),
                Value::from(vec![true, false]),
                true,
                "bool arrays equal",
            ),
            // Different lengths
            (
                Value::from(vec![1i64, 2]),
                Value::from(vec![1i64, 2, 3]),
                false,
                "different length arrays",
            ),
        ];

        for (left, right, expected, desc) in test_cases {
            assert_eq!(left == right, expected, "Failed: {}", desc);
        }
    }

    #[test]
    fn test_array_ordering() {
        let test_cases: Vec<(Value, Value, Ordering, &str)> = vec![
            (
                Value::from(vec!["a", "b"]),
                Value::from(vec!["a", "c"]),
                Ordering::Less,
                "string array lexicographic",
            ),
            (
                Value::from(vec!["a"]),
                Value::from(vec!["a", "b"]),
                Ordering::Less,
                "shorter array < longer",
            ),
            (
                Value::from(vec![1i64, 2]),
                Value::from(vec![1i64, 3]),
                Ordering::Less,
                "int array lexicographic",
            ),
            (
                Value::from(vec![1u64, 2]),
                Value::from(vec![1u64, 3]),
                Ordering::Less,
                "uint array lexicographic",
            ),
            (
                Value::from(vec![1.0, 2.0]),
                Value::from(vec![1.0, 3.0]),
                Ordering::Less,
                "float array lexicographic",
            ),
            (
                Value::from(vec![false, false]),
                Value::from(vec![false, true]),
                Ordering::Less,
                "bool array lexicographic",
            ),
        ];

        for (left, right, expected, desc) in test_cases {
            assert_eq!(left.partial_cmp(&right), Some(expected), "Failed: {}", desc);
        }
    }

    #[test]
    fn test_array_display() {
        let test_cases = vec![
            (Value::from(vec!["a", "b", "c"]), "[\"a\", \"b\", \"c\"]"),
            (Value::from(vec![1i64, 2, 3]), "[1, 2, 3]"),
            (Value::from(vec![1u64, 2, 3]), "[1, 2, 3]"),
            (Value::from(vec![1.5, 2.5]), "[1.5, 2.5]"),
            (Value::from(vec![true, false]), "[true, false]"),
            (Value::IntArray(Vec::<i64>::new().into_boxed_slice()), "[]"),
        ];

        for (value, expected) in test_cases {
            assert_eq!(value.to_string(), expected);
        }
    }

    // ==================== Corner Cases ====================

    #[test]
    fn test_zero_values() {
        let test_cases = vec![
            (Value::Int(0), Value::Int(0), true, "int 0 == int 0"),
            (Value::Uint(0), Value::Uint(0), true, "uint 0 == uint 0"),
            (
                Value::Float(0.0),
                Value::Float(0.0),
                true,
                "float 0 == float 0",
            ),
            (Value::Float(-0.0), Value::Float(0.0), true, "-0.0 == 0.0"),
            (Value::Int(0), Value::Uint(0), true, "int 0 == uint 0"),
            (Value::Float(0.0), Value::Int(0), true, "float 0 == int 0"),
        ];

        for (left, right, expected, desc) in test_cases {
            assert_eq!(left == right, expected, "Failed: {}", desc);
        }
    }

    #[test]
    fn test_empty_values() {
        // Empty string
        assert_eq!(Value::from(""), Value::from(""));
        assert_ne!(Value::from(""), Value::from("x"));

        // Empty arrays
        let empty_int = Value::IntArray(vec![].into_boxed_slice());
        let empty_int2 = Value::IntArray(vec![].into_boxed_slice());
        assert_eq!(empty_int, empty_int2);

        // None
        assert_eq!(Value::None, Value::None);
        assert_ne!(Value::None, Value::Int(0));
    }

    #[test]
    fn test_string_to_repr_edge_cases() {
        let test_cases = vec![
            (Value::None, ""),
            (Value::from(Vec::<&str>::new()), "[]"),
            (Value::from(Vec::<i64>::new()), "[]"),
            (Value::from(vec!["a"]), "[\"a\"]"),
            (Value::from(vec![1i64]), "[1]"),
        ];

        for (value, expected) in test_cases {
            assert_eq!(value.to_string_repr(), expected);
        }
    }

    // ==================== Serde Tests ====================

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_roundtrip() {
        let test_cases = vec![
            Value::from("hello"),
            Value::Int(42),
            Value::Uint(42),
            Value::Float(3.34),
            Value::Bool(true),
            Value::None,
            Value::from(vec!["a", "b"]),
            Value::from(vec![1i64, 2, 3]),
            Value::from(vec![1u64, 2, 3]),
            Value::from(vec![1.5, 2.5]),
            Value::from(vec![true, false]),
        ];

        for value in test_cases {
            let json = serde_json::to_string(&value).unwrap();
            let deserialized: Value = serde_json::from_str(&json).unwrap();
            assert_eq!(
                value, deserialized,
                "Serde roundtrip failed for: {:?}",
                value
            );
        }
    }

    // ==================== From Implementation Coverage ====================

    #[test]
    fn test_from_all_primitive_types() {
        // Owned primitives
        assert!(matches!(Value::from(42i8), Value::Int(42)));
        assert!(matches!(Value::from(42i16), Value::Int(42)));
        assert!(matches!(Value::from(42i32), Value::Int(42)));
        assert!(matches!(Value::from(42i64), Value::Int(42)));
        assert!(matches!(Value::from(42isize), Value::Int(42)));
        assert!(matches!(Value::from(42u8), Value::Uint(42)));
        assert!(matches!(Value::from(42u16), Value::Uint(42)));
        assert!(matches!(Value::from(42u32), Value::Uint(42)));
        assert!(matches!(Value::from(42u64), Value::Uint(42)));
        assert!(matches!(Value::from(42usize), Value::Uint(42)));
        assert!(matches!(Value::from(3.34f32), Value::Float(_)));
        assert!(matches!(Value::from(3.34f64), Value::Float(_)));
        assert!(matches!(Value::from(true), Value::Bool(true)));

        // Reference primitives
        let i: i32 = 42;
        let u: u32 = 42;
        let f: f64 = 3.34;
        let b: bool = true;
        let s: String = "hello".to_string();

        assert!(matches!(Value::from(&i), Value::Int(42)));
        assert!(matches!(Value::from(&u), Value::Uint(42)));
        assert!(matches!(Value::from(&f), Value::Float(_)));
        assert!(matches!(Value::from(&b), Value::Bool(true)));
        assert!(matches!(Value::from(&s), Value::String(_)));
    }

    #[test]
    fn test_from_vec_types() {
        // Owned Vec
        assert!(matches!(Value::from(vec![1i16, 2]), Value::IntArray(_)));
        assert!(matches!(Value::from(vec![1i8, 2]), Value::IntArray(_)));
        assert!(matches!(Value::from(vec![1u16, 2]), Value::UintArray(_)));
        assert!(matches!(Value::from(vec![1u8, 2]), Value::UintArray(_)));
        assert!(matches!(Value::from(vec![1isize, 2]), Value::IntArray(_)));
        assert!(matches!(Value::from(vec![1usize, 2]), Value::UintArray(_)));

        // Borrowed Vec
        let vi: Vec<i32> = vec![1, 2, 3];
        let vu: Vec<u32> = vec![1, 2, 3];
        let vf: Vec<f64> = vec![1.0, 2.0];
        let vb: Vec<bool> = vec![true, false];

        assert!(matches!(Value::from(&vi), Value::IntArray(_)));
        assert!(matches!(Value::from(&vu), Value::UintArray(_)));
        assert!(matches!(Value::from(&vf), Value::FloatArray(_)));
        assert!(matches!(Value::from(&vb), Value::BoolArray(_)));

        // Slices
        let si: &[i32] = &[1, 2, 3];
        let su: &[u32] = &[1, 2, 3];
        assert!(matches!(Value::from(si), Value::IntArray(_)));
        assert!(matches!(Value::from(su), Value::UintArray(_)));
    }

    #[test]
    fn test_from_hashset_types() {
        use std::collections::HashSet;

        let mut set_i16: HashSet<i16> = HashSet::new();
        set_i16.insert(1);
        set_i16.insert(2);
        assert!(matches!(Value::from(set_i16), Value::IntSet(_)));

        let mut set_u64: HashSet<u64> = HashSet::new();
        set_u64.insert(1);
        assert!(matches!(Value::from(&set_u64), Value::UintSet(_)));

        let mut set_bool: HashSet<bool> = HashSet::new();
        set_bool.insert(true);
        assert!(matches!(Value::from(set_bool), Value::BoolSet(_)));

        let mut set_str: HashSet<String> = HashSet::new();
        set_str.insert("hello".to_string());
        assert!(matches!(Value::from(set_str), Value::StringSet(_)));
    }

    // ==================== Direct Comparison Tests ====================
    //
    // Tests for PartialEq<T> and PartialOrd<T> implementations that
    // allow comparing Value with primitives without conversion.

    #[test]
    fn test_direct_str_comparison() {
        let val = Value::from("hello");

        // Only primitive == Value is implemented (not Value == primitive)
        // str == Value
        assert!("hello" == val);
        assert!("world" != val);

        // With String
        let v = String::from("hello");
        assert!(v == val);

        // Non-string Value compared with str (uses string repr)
        assert!("42" == Value::Int(42));
        assert!("true" == Value::Bool(true));
    }

    #[test]
    fn test_direct_i64_comparison() {
        let val = Value::Int(42);

        // Only primitive == Value is implemented (not Value == primitive)
        // i64 == Value
        assert!(42i64 == val);
        assert!(43i64 != val);

        // Cross-type: i64 compared with Value::Uint
        assert!(42i64 == Value::Uint(42));
        assert!(-1i64 != Value::Uint(42));

        // Cross-type: i64 compared with Value::Float
        assert!(42i64 == Value::Float(42.0));
    }

    #[test]
    fn test_direct_u64_comparison() {
        let val = Value::Uint(42);

        // Only primitive == Value is implemented (not Value == primitive)
        // u64 == Value
        assert!(42u64 == val);
        assert!(43u64 != val);

        // Cross-type: u64 compared with Value::Int
        assert!(42u64 == Value::Int(42));
        assert!(42u64 != Value::Int(-1));
    }

    #[test]
    fn test_direct_f64_comparison() {
        let val = Value::Float(3.714);

        // Only primitive == Value is implemented (not Value == primitive)
        // f64 == Value
        assert!(3.714f64 == val);
        assert!(2.71f64 != val);

        // Cross-type comparisons
        assert!(42.0f64 == Value::Int(42));
        assert!(42.0f64 == Value::Uint(42));
    }

    #[test]
    fn test_direct_bool_comparison() {
        let val_true = Value::Bool(true);
        let val_false = Value::Bool(false);

        // Only primitive == Value is implemented (not Value == primitive)
        assert!(true == val_true);
        assert!(false != val_true);
        assert!(false == val_false);
        assert!(true == val_true);
        assert!(false == val_false);

        // String "true"/"false" comparisons
        assert!(true == Value::from("true"));
        assert!(false == Value::from("false"));
    }

    #[test]
    fn test_direct_ordering_i64() {
        let val = Value::Int(42);

        // Only primitive <=> Value is implemented (not Value <=> primitive)
        assert!(41i64 < val);
        assert!(43i64 > val);
        assert!(42i64 <= val);
        assert!(42i64 >= val);

        // Reverse direction
        assert!(43i64 > val);
        assert!(41i64 < val);

        // Cross-type
        assert!(40i64 < Value::Uint(50));
        assert!(3i64 < Value::Float(3.5));
    }

    #[test]
    fn test_direct_ordering_u64() {
        let val = Value::Uint(42);

        // Only primitive <=> Value is implemented (not Value <=> primitive)
        assert!(41u64 < val);
        assert!(43u64 > val);
        assert!(42u64 <= val);
        assert!(42u64 >= val);

        // Cross-type: negative i64 always less than uint
        assert!((-1i64).partial_cmp(&Value::Uint(42)) == Some(Ordering::Less));
    }

    #[test]
    fn test_direct_ordering_f64() {
        let val = Value::Float(3.74);

        // Only primitive <=> Value is implemented (not Value <=> primitive)
        assert!(3.0f64 < val);
        assert!(4.0f64 > val);

        // Cross-type
        assert!(4.5f64 < Value::Int(5));
        assert!(3.5f64 > Value::Uint(3));
    }

    #[test]
    fn test_direct_ordering_str() {
        let val = Value::from("hello");

        // Only primitive <=> Value is implemented (not Value <=> primitive)
        assert!("apple" < val);
        assert!("zebra" > val);
        assert!("hello" <= val);
        assert!("hello" >= val);

        // Same tests (duplicated for clarity)
        assert!("zebra" > val);
        assert!("apple" < val);
    }

    // ==================== Cow<str> Tests ====================

    #[test]
    fn test_cow_str_from() {
        let test_cases = vec![
            (Cow::Borrowed("hello"), "hello", "borrowed variant"),
            (Cow::Owned("world".to_string()), "world", "owned variant"),
            (Cow::Borrowed(""), "", "empty borrowed"),
            (Cow::Owned(String::new()), "", "empty owned"),
        ];

        for (cow, expected, desc) in test_cases {
            let val = Value::from(cow);
            assert!(matches!(val, Value::String(_)), "Failed: {}", desc);
            assert_eq!(val, Value::from(expected), "Failed: {}", desc);
        }
    }

    #[test]
    fn test_cow_str_equality() {
        let test_cases = vec![
            // (cow, value, expected_eq, description)
            (
                Cow::Borrowed("hello"),
                Value::from("hello"),
                true,
                "borrowed cow == string",
            ),
            (
                Cow::Borrowed("hello"),
                Value::from("world"),
                false,
                "borrowed cow != different string",
            ),
            (
                Cow::Owned("hello".to_string()),
                Value::from("hello"),
                true,
                "owned cow == string",
            ),
            (
                Cow::Owned("world".to_string()),
                Value::from("hello"),
                false,
                "owned cow != different string",
            ),
            (
                Cow::Borrowed("42"),
                Value::Int(42),
                true,
                "cow '42' == int 42",
            ),
            (
                Cow::Borrowed("true"),
                Value::Bool(true),
                true,
                "cow 'true' == bool true",
            ),
            (
                Cow::Borrowed(""),
                Value::from(""),
                true,
                "empty cow == empty string",
            ),
        ];

        for (cow, value, expected, desc) in test_cases {
            assert_eq!(cow == value, expected, "Failed: {}", desc);
        }
    }

    #[test]
    fn test_cow_str_ordering() {
        let test_cases = vec![
            // (cow, value, expected_ordering, description)
            (
                Cow::Borrowed("apple"),
                Value::from("hello"),
                Some(Ordering::Less),
                "apple < hello",
            ),
            (
                Cow::Borrowed("zebra"),
                Value::from("hello"),
                Some(Ordering::Greater),
                "zebra > hello",
            ),
            (
                Cow::Borrowed("hello"),
                Value::from("hello"),
                Some(Ordering::Equal),
                "hello == hello (borrowed)",
            ),
            (
                Cow::Owned("hello".to_string()),
                Value::from("hello"),
                Some(Ordering::Equal),
                "hello == hello (owned)",
            ),
            (
                Cow::Borrowed("a"),
                Value::from("b"),
                Some(Ordering::Less),
                "a < b",
            ),
            (
                Cow::Borrowed(""),
                Value::from(""),
                Some(Ordering::Equal),
                "empty == empty",
            ),
        ];

        for (cow, value, expected, desc) in test_cases {
            assert_eq!(cow.partial_cmp(&value), expected, "Failed: {}", desc);
        }
    }
}

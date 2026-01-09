use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Field, Fields};

/// Derive macro for `DnfEvaluable` trait.
///
/// # Supported Types
///
/// - Primitives: `i8`-`i64`, `u8`-`u64`, `f32`, `f64`, `bool`, `String`, `Cow<str>`
/// - Collections: `Vec<T>`, `HashSet<T>` (T: primitive)
/// - Maps: `HashMap<String, V>`, `BTreeMap<String, V>` (V: primitive)
/// - Options: `Option<T>` (T: any supported)
/// - Custom: any type implementing `DnfField`
///
/// # Custom Types
///
/// Implement `DnfField` to use custom types with derive:
///
/// ```rust,ignore
/// impl DnfField for Score {
///     fn evaluate(&self, op: &Op, value: &Value) -> bool {
///         (self.0 as i64).evaluate(op, value)
///     }
/// }
/// ```
///
/// For computed fields, implement `DnfEvaluable` manually instead.
///
/// # Attributes
///
/// - `#[dnf(skip)]` - exclude field
/// - `#[dnf(rename = "name")]` - custom query name
/// - `#[dnf(nested)]` - force nested access (auto-detected for non-primitives)
/// - `#[dnf(iter)]` or `#[dnf(iter = "method")]` - custom collection iterator
///
/// # Nested Access
///
/// Non-primitive types auto-detected. Query with dot notation: `address.city == "Boston"`
///
/// - `Vec<T>`: `items.field` - any item matches
/// - `HashMap<K,V>`: `map.@values.field`, `map.@keys`, `map.["key"].field`
#[proc_macro_derive(DnfEvaluable, attributes(dnf))]
pub fn derive_dnf_evaluable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;

    // Only support structs with named fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "DnfEvaluable can only be derived for structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "DnfEvaluable can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    // Generate match arms for each field
    let match_arms = fields.iter().filter_map(generate_field_match_arm);

    // Generate nested field match arms for fields marked with #[dnf(nested)]
    let nested_match_arms = fields.iter().filter_map(generate_nested_field_match_arm);

    // Generate field info list
    let field_infos = fields.iter().filter_map(generate_field_info);

    // Generate get_field_value match arms for custom operator support
    let field_value_arms = fields.iter().filter_map(generate_field_value_arm);

    let expanded = quote! {
        impl dnf::DnfEvaluable for #name {
            fn evaluate_field(
                &self,
                field_name: &str,
                operator: &dnf::Op,
                value: &dnf::Value
            ) -> bool {
                // Try direct field match first
                match field_name {
                    #(#match_arms)*
                    _ => {
                        // Try nested field access (field.subfield.subsubfield)
                        if let Some(dot_pos) = field_name.find('.') {
                            let (outer, inner) = field_name.split_at(dot_pos);
                            let inner = &inner[1..]; // Skip the dot
                            match outer {
                                #(#nested_match_arms)*
                                _ => false,
                            }
                        } else {
                            false // Unknown field
                        }
                    }
                }
            }

            fn get_field_value(&self, field_name: &str) -> Option<dnf::Value> {
                match field_name {
                    #(#field_value_arms)*
                    _ => None,
                }
            }

            fn fields() -> impl Iterator<Item = dnf::FieldInfo> {
                [
                    #(#field_infos),*
                ].into_iter()
            }
        }
    };

    TokenStream::from(expanded)
}

/// Generate a match arm for a field
fn generate_field_match_arm(field: &Field) -> Option<proc_macro2::TokenStream> {
    let field_name = field.ident.as_ref()?;
    let field_type = &field.ty;

    // Check for #[dnf(skip)] attribute
    if has_skip_attribute(field) {
        return None;
    }

    // Get type string for analysis
    let type_str = quote!(#field_type).to_string().replace(" ", "");

    // Skip nested fields - they're handled by generate_nested_field_match_arm
    // This includes explicit #[dnf(nested)] AND auto-detected nested types
    // BUT NOT fields with #[dnf(iter)] - those are iterator-based collections
    let has_iter = get_iter_attribute(field).is_some();
    if has_nested_attribute(field) || (!has_iter && is_nested_type(&type_str)) {
        return None;
    }

    // Get the query field name (either from rename attribute or field name)
    let query_name = get_rename_attribute(field).unwrap_or_else(|| field_name.to_string());

    // Generate the value conversion using DnfField::evaluate()
    let value_conversion = generate_value_conversion(field, field_name, field_type);

    Some(quote! {
        #query_name => #value_conversion,
    })
}

/// Generate a match arm for get_field_value (custom operator support).
/// Converts the field to Value for custom operator evaluation.
/// Only generates for types that have `From<&T>` implementation for Value.
fn generate_field_value_arm(field: &Field) -> Option<proc_macro2::TokenStream> {
    let field_name = field.ident.as_ref()?;
    let field_type = &field.ty;

    // Check for #[dnf(skip)] attribute
    if has_skip_attribute(field) {
        return None;
    }

    // Get type string for analysis
    let type_str = quote!(#field_type).to_string().replace(" ", "");

    // Skip nested fields - custom ops don't make sense for nested access
    let has_iter = get_iter_attribute(field).is_some();
    if has_nested_attribute(field) || (!has_iter && is_nested_type(&type_str)) {
        return None;
    }

    // Only generate for types we KNOW have From<&T> impl for Value
    // This is the whitelist approach - safer than blacklist
    if !is_value_convertible(&type_str) {
        return None;
    }

    // Get the query field name (either from rename attribute or field name)
    let query_name = get_rename_attribute(field).unwrap_or_else(|| field_name.to_string());

    // Generate Value conversion based on type
    let value_conversion = if type_str.starts_with("Option<") {
        // For Option<T>, convert to Value::None if None
        quote! {
            match &self.#field_name {
                Some(v) => Some(dnf::Value::from(v)),
                None => Some(dnf::Value::None),
            }
        }
    } else {
        // For direct types, convert to Value
        quote! {
            Some(dnf::Value::from(&self.#field_name))
        }
    };

    Some(quote! {
        #query_name => #value_conversion,
    })
}

/// Check if a type can be converted to Value via From<&T>.
/// Only returns true for types we know have this implementation.
fn is_value_convertible(type_str: &str) -> bool {
    // Primitives
    let primitives = [
        "i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64", "usize", "f32", "f64",
        "bool", "String",
    ];

    if primitives.contains(&type_str) {
        return true;
    }

    // &str variants
    if type_str.starts_with("&") && type_str.contains("str") {
        return true;
    }

    // Cow<str> variants (Cow<'_, str>, Cow<'static, str>, etc.)
    if type_str.starts_with("Cow<") && type_str.contains("str") {
        return true;
    }

    // Vec<T> where T is primitive
    if type_str.starts_with("Vec<") {
        if let Some(inner) = type_str
            .strip_prefix("Vec<")
            .and_then(|s| s.strip_suffix(">"))
        {
            return primitives.contains(&inner);
        }
    }

    // HashSet<T> where T is primitive (except floats)
    if type_str.starts_with("HashSet<") {
        if let Some(inner) = type_str
            .strip_prefix("HashSet<")
            .and_then(|s| s.strip_suffix(">"))
        {
            // HashSet doesn't work with f32/f64 (not Hash)
            return primitives.contains(&inner) && inner != "f32" && inner != "f64";
        }
    }

    // Option<T> where T is convertible
    if type_str.starts_with("Option<") {
        if let Some(inner) = type_str
            .strip_prefix("Option<")
            .and_then(|s| s.strip_suffix(">"))
        {
            return is_value_convertible(inner);
        }
    }

    false
}

/// Check if a type requires nested field access (collection/map of non-primitive inner type)
/// Returns true only for collections/maps that contain nested structs needing evaluate_field delegation.
/// Custom scalar types use DnfField::evaluate() directly and are NOT considered nested.
fn is_nested_type(type_str: &str) -> bool {
    // Check Vec<T> where T is non-primitive (nested struct)
    if type_str.starts_with("Vec<") {
        if let Some(inner) = type_str
            .strip_prefix("Vec<")
            .and_then(|s| s.strip_suffix(">"))
        {
            return !is_primitive_or_builtin(inner);
        }
    }

    // Check Option<Vec<T>> where T is non-primitive
    if type_str.starts_with("Option<Vec<") {
        if let Some(inner) = type_str
            .strip_prefix("Option<Vec<")
            .and_then(|s| s.strip_suffix(">>"))
        {
            return !is_primitive_or_builtin(inner);
        }
    }

    // Check HashMap<K, V> or BTreeMap<K, V> where V is non-primitive
    if is_map_type(type_str) {
        if let Some((_, value_type)) = extract_map_types(type_str) {
            return !is_primitive_or_builtin(&value_type);
        }
    }

    // Check Option<HashMap/BTreeMap>
    if type_str.starts_with("Option<HashMap<") || type_str.starts_with("Option<BTreeMap<") {
        if let Some(inner) = type_str
            .strip_prefix("Option<")
            .and_then(|s| s.strip_suffix(">"))
        {
            if let Some((_, value_type)) = extract_map_types(inner) {
                return !is_primitive_or_builtin(&value_type);
            }
        }
    }

    // Custom scalar types (Score, Status, etc.) are NOT nested.
    // They use DnfField::evaluate() directly.
    // Only explicit #[dnf(nested)] or collection/map of nested structs triggers nested handling.
    false
}

/// Check if a type is a map type (HashMap or BTreeMap)
fn is_map_type(type_str: &str) -> bool {
    type_str.starts_with("HashMap<") || type_str.starts_with("BTreeMap<")
}

/// Extract key and value types from HashMap<K, V> or BTreeMap<K, V>
/// Returns (key_type, value_type) or None if not a map type
fn extract_map_types(type_str: &str) -> Option<(String, String)> {
    let inner = type_str
        .strip_prefix("HashMap<")
        .or_else(|| type_str.strip_prefix("BTreeMap<"))?;
    let inner = inner.strip_suffix(">")?;

    // Find the first comma that's not inside angle brackets
    let mut depth = 0;
    let mut comma_pos = None;
    for (i, c) in inner.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                comma_pos = Some(i);
                break;
            }
            _ => {}
        }
    }

    let pos = comma_pos?;
    let key = inner[..pos].trim().to_string();
    let value = inner[pos + 1..].trim().to_string();
    Some((key, value))
}

/// Check if key type is string-like (String, &str, &'lifetime str)
fn is_string_key(key_type: &str) -> bool {
    let t = key_type.trim();
    // Exact matches for common cases
    matches!(t, "String" | "str" | "&str")
        // Reference with lifetime: &'static str, &'a str, etc.
        || (t.starts_with("&'") && (t.ends_with("str") || t.ends_with(" str")))
}

/// Check if field has #[dnf(skip)] attribute
fn has_skip_attribute(field: &Field) -> bool {
    for attr in &field.attrs {
        if attr.path().is_ident("dnf") {
            let mut has_skip = false;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("skip") {
                    has_skip = true;
                }
                Ok(())
            });
            if has_skip {
                return true;
            }
        }
    }
    false
}

/// Check if field has #[dnf(nested)] attribute
fn has_nested_attribute(field: &Field) -> bool {
    for attr in &field.attrs {
        if attr.path().is_ident("dnf") {
            let mut has_nested = false;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("nested") {
                    has_nested = true;
                }
                Ok(())
            });
            if has_nested {
                return true;
            }
        }
    }
    false
}

/// Generate a match arm for nested field access.
/// Auto-detects nested types (non-primitive inner types) or uses explicit #[dnf(nested)].
///
/// Supports:
/// - Scalar nested: `address.city` → delegates to `self.address.evaluate_field("city", ...)`
/// - Vec nested: `addresses.city` → `self.addresses.iter().any(|item| item.evaluate_field("city", ...))`
/// - HashMap nested: `branches.@values.city` → iterate values, delegate (explicit @values required)
fn generate_nested_field_match_arm(field: &Field) -> Option<proc_macro2::TokenStream> {
    let field_name = field.ident.as_ref()?;
    let field_type = &field.ty;

    // Skip fields marked with #[dnf(skip)]
    if has_skip_attribute(field) {
        return None;
    }

    // Detect collection type and generate appropriate code
    let type_str = quote!(#field_type).to_string().replace(" ", "");

    // Fields with #[dnf(iter)] are iterator-based collections, not nested
    let has_iter = get_iter_attribute(field).is_some();
    if has_iter {
        return None;
    }

    // Only generate for nested types (explicit attribute OR auto-detected)
    if !has_nested_attribute(field) && !is_nested_type(&type_str) {
        return None;
    }

    // Get the query field name (either from rename attribute or field name)
    let query_name = get_rename_attribute(field).unwrap_or_else(|| field_name.to_string());

    let delegation_code = if type_str.starts_with("Vec<") {
        // Vec<T> with nested: iterate and delegate with any() semantics
        quote! {
            self.#field_name.iter().any(|item| item.evaluate_field(inner, operator, value))
        }
    } else if type_str.starts_with("Option<Vec<") {
        // Option<Vec<T>> with nested
        quote! {
            match &self.#field_name {
                Some(vec) => vec.iter().any(|item| item.evaluate_field(inner, operator, value)),
                None => false,
            }
        }
    } else if type_str.starts_with("HashMap<") || type_str.starts_with("BTreeMap<") {
        // HashMap<K, V> with nested values
        // Requires explicit syntax: @values.field, @keys, ["key"].field
        quote! {
            if let Some(rest) = inner.strip_prefix("@values.") {
                // branches.@values.city -> iterate values, query city
                self.#field_name.values().any(|item| item.evaluate_field(rest, operator, value))
            } else if inner == "@keys" {
                // branches.@keys -> use any on keys (strings)
                operator.any(self.#field_name.keys(), value)
            } else if inner.starts_with("[\"") {
                // branches["key"].field -> access specific key, then nested field
                if let Some(end_bracket) = inner.find("\"]") {
                    let key = &inner[2..end_bracket];
                    let rest = inner.get(end_bracket + 2..).unwrap_or("").trim_start_matches('.');
                    if rest.is_empty() {
                        // branches["key"] alone - not meaningful for nested structs
                        false
                    } else {
                        match self.#field_name.get(key) {
                            Some(item) => item.evaluate_field(rest, operator, value),
                            None => false,
                        }
                    }
                } else {
                    false
                }
            } else {
                // No implicit @values - require explicit syntax
                false
            }
        }
    } else if type_str.starts_with("Option<HashMap<") || type_str.starts_with("Option<BTreeMap<") {
        // Option<HashMap<K, V>> with nested - requires explicit syntax
        quote! {
            match &self.#field_name {
                Some(map) => {
                    if let Some(rest) = inner.strip_prefix("@values.") {
                        map.values().any(|item| item.evaluate_field(rest, operator, value))
                    } else if inner == "@keys" {
                        operator.any(map.keys(), value)
                    } else if inner.starts_with("[\"") {
                        if let Some(end_bracket) = inner.find("\"]") {
                            let key = &inner[2..end_bracket];
                            let rest = inner.get(end_bracket + 2..).unwrap_or("").trim_start_matches('.');
                            if rest.is_empty() {
                                false
                            } else {
                                match map.get(key) {
                                    Some(item) => item.evaluate_field(rest, operator, value),
                                    None => false,
                                }
                            }
                        } else {
                            false
                        }
                    } else {
                        // No implicit @values - require explicit syntax
                        false
                    }
                },
                None => false,
            }
        }
    } else if type_str.starts_with("Option<") {
        // Option<T> scalar nested
        quote! {
            match &self.#field_name {
                Some(inner_val) => inner_val.evaluate_field(inner, operator, value),
                None => false,
            }
        }
    } else {
        // Scalar nested struct: direct delegation
        quote! {
            self.#field_name.evaluate_field(inner, operator, value)
        }
    };

    Some(quote! {
        #query_name => #delegation_code,
    })
}

/// Get rename attribute value if present
fn get_rename_attribute(field: &Field) -> Option<String> {
    for attr in &field.attrs {
        if attr.path().is_ident("dnf") {
            let mut rename_value = None;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename") {
                    if let Ok(value) = meta.value() {
                        if let Ok(lit_str) = value.parse::<syn::LitStr>() {
                            rename_value = Some(lit_str.value());
                        }
                    }
                }
                Ok(())
            });
            if let Some(name) = rename_value {
                return Some(name);
            }
        }
    }
    None
}

/// Get iter attribute value if present.
/// Returns:
/// - `Some(None)` for `#[dnf(iter)]` (uses `.iter()` method)
/// - `Some(Some("method"))` for `#[dnf(iter = "method")]` (uses custom method)
/// - `None` if not present
fn get_iter_attribute(field: &Field) -> Option<Option<String>> {
    for attr in &field.attrs {
        if attr.path().is_ident("dnf") {
            let mut has_iter = false;
            let mut iter_method = None;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("iter") {
                    has_iter = true;
                    // Check if it has a value (e.g., iter = "method")
                    if let Ok(value) = meta.value() {
                        if let Ok(lit_str) = value.parse::<syn::LitStr>() {
                            iter_method = Some(lit_str.value());
                        }
                    }
                }
                Ok(())
            });
            if has_iter {
                return Some(iter_method);
            }
        }
    }
    None
}

/// Generate value conversion code based on field type.
///
/// All types now use `DnfField::evaluate()` for type-safe comparison.
/// This simplifies code generation and keeps all logic in trait implementations.
///
/// Fields with `#[dnf(iter)]` attribute use custom iterator method.
fn generate_value_conversion(
    field: &Field,
    field_name: &syn::Ident,
    _field_type: &syn::Type,
) -> proc_macro2::TokenStream {
    // Check for #[dnf(iter)] attribute - use any for custom collection types
    if let Some(iter_method) = get_iter_attribute(field) {
        let method = iter_method.unwrap_or_else(|| "iter".to_string());
        let method_ident = syn::Ident::new(&method, field_name.span());
        return quote! {
            operator.any(self.#field_name.#method_ident(), value)
        };
    }

    // All types use DnfField::evaluate() - simple and type-safe
    quote! {
        dnf::DnfField::evaluate(&self.#field_name, operator, value)
    }
}

/// Check if a type is a built-in primitive that implements `DnfField`.
///
/// Returns true for types that can be used directly in queries without nested field access:
/// - Primitives: i8-i64, u8-u64, f32, f64, bool, String, &str
/// - Collections of primitives: `Vec<T>`, `HashSet<T>`, `HashMap<String, V>` where T/V is primitive
///
/// Returns false for custom types that require nested field access delegation.
fn is_primitive_or_builtin(type_str: &str) -> bool {
    // Primitives
    let primitives = [
        "i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64", "usize", "f32", "f64",
        "bool", "String",
    ];

    if primitives.contains(&type_str) {
        return true;
    }

    // &str variants
    if type_str.starts_with("&") && type_str.contains("str") {
        return true;
    }

    // Cow<str> variants (Cow<'_, str>, Cow<'static, str>, etc.)
    if type_str.starts_with("Cow<") && type_str.contains("str") {
        return true;
    }

    // Vec<T> variants - check if inner type is supported
    if type_str.starts_with("Vec<") {
        if let Some(inner) = type_str.strip_prefix("Vec<") {
            if let Some(inner) = inner.strip_suffix(">") {
                // Recursively check if inner type is supported
                return is_primitive_or_builtin(inner);
            }
        }
    }

    // HashSet<T> variants - check if inner type is supported
    // Note: floats (f32, f64) are NOT supported in HashSet because they don't implement Hash
    if type_str.starts_with("HashSet<") {
        if let Some(inner) = type_str.strip_prefix("HashSet<") {
            if let Some(inner) = inner.strip_suffix(">") {
                // Floats don't implement Hash, so they can't be used in HashSet
                if inner == "f32" || inner == "f64" {
                    return false;
                }
                // Recursively check if inner type is supported
                return is_primitive_or_builtin(inner);
            }
        }
    }

    // HashMap<K, V> or BTreeMap<K, V> - check key is string-like and value is supported
    if is_map_type(type_str) {
        if let Some((key_type, value_type)) = extract_map_types(type_str) {
            return is_string_key(&key_type) && is_primitive_or_builtin(&value_type);
        }
    }

    false
}

/// Generate a FieldInfo for a field
fn generate_field_info(field: &Field) -> Option<proc_macro2::TokenStream> {
    let field_name = field.ident.as_ref()?;
    let field_type = &field.ty;

    // Skip fields with #[dnf(skip)] attribute
    if has_skip_attribute(field) {
        return None;
    }

    // Get the query field name (either from rename attribute or field name)
    let query_name = get_rename_attribute(field).unwrap_or_else(|| field_name.to_string());

    // Get the type as a string
    let type_str = quote!(#field_type).to_string();
    let type_str_normalized = type_str.replace(" ", "");

    // Determine field kind
    let field_kind = if get_iter_attribute(field).is_some() {
        // Fields with #[dnf(iter)] are treated as iterator-based
        quote! { dnf::FieldKind::Iter }
    } else if is_map_type(&type_str_normalized) {
        quote! { dnf::FieldKind::Map }
    } else if type_str_normalized.starts_with("Vec<")
        || type_str_normalized.starts_with("HashSet<")
        || type_str_normalized.starts_with("BTreeSet<")
    {
        quote! { dnf::FieldKind::Iter }
    } else {
        quote! { dnf::FieldKind::Scalar }
    };

    Some(quote! {
        dnf::FieldInfo::with_kind(#query_name, #type_str, #field_kind)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitives_use_dnf_field() {
        // All types now use DnfField::evaluate() for consistency
        let primitives = vec!["String", "u32", "i64", "f64", "bool"];

        for type_str in primitives {
            let input_str = format!("struct User {{ field: {} }}", type_str);
            let input: proc_macro2::TokenStream = input_str.parse().unwrap();

            let parsed: DeriveInput = syn::parse2(input).unwrap();
            let fields = match &parsed.data {
                Data::Struct(data) => match &data.fields {
                    Fields::Named(fields) => &fields.named,
                    _ => continue,
                },
                _ => continue,
            };

            if let Some(field) = fields.first() {
                let conversion =
                    generate_value_conversion(field, field.ident.as_ref().unwrap(), &field.ty);
                let conversion_str = conversion.to_string();

                // All types use DnfField::evaluate()
                assert!(
                    conversion_str.contains("DnfField :: evaluate"),
                    "Type {} should use DnfField::evaluate(), got: {}",
                    type_str,
                    conversion_str
                );
            }
        }
    }

    #[test]
    fn test_collections_use_dnf_field() {
        // Collections also use DnfField::evaluate() (dispatches to Op::any internally)
        let collections = vec!["Vec<String>", "HashSet<i32>"];

        for type_str in collections {
            let input_str = format!("struct User {{ field: {} }}", type_str);
            let input: proc_macro2::TokenStream = input_str.parse().unwrap();

            let parsed: DeriveInput = syn::parse2(input).unwrap();
            let fields = match &parsed.data {
                Data::Struct(data) => match &data.fields {
                    Fields::Named(fields) => &fields.named,
                    _ => continue,
                },
                _ => continue,
            };

            if let Some(field) = fields.first() {
                let conversion =
                    generate_value_conversion(field, field.ident.as_ref().unwrap(), &field.ty);
                let conversion_str = conversion.to_string();

                // Collections use DnfField::evaluate()
                assert!(
                    conversion_str.contains("DnfField :: evaluate"),
                    "Collection {} should use DnfField::evaluate(), got: {}",
                    type_str,
                    conversion_str
                );
            }
        }
    }

    #[test]
    fn test_custom_types_use_dnf_field() {
        // Custom types should use DnfField::evaluate()
        let custom_types = vec!["Score", "CustomEnum", "MyStruct"];

        for type_str in custom_types {
            let input_str = format!("struct User {{ field: {} }}", type_str);
            let input: proc_macro2::TokenStream = input_str.parse().unwrap();

            let parsed: DeriveInput = syn::parse2(input).unwrap();
            let fields = match &parsed.data {
                Data::Struct(data) => match &data.fields {
                    Fields::Named(fields) => &fields.named,
                    _ => continue,
                },
                _ => continue,
            };

            if let Some(field) = fields.first() {
                let conversion =
                    generate_value_conversion(field, field.ident.as_ref().unwrap(), &field.ty);
                let conversion_str = conversion.to_string();

                // Custom types should use DnfField::evaluate()
                assert!(
                    conversion_str.contains("DnfField :: evaluate"),
                    "Custom type {} should use DnfField::evaluate(), got: {}",
                    type_str,
                    conversion_str
                );
            }
        }
    }

    #[test]
    fn test_iter_attribute_generates_any() {
        // Test that #[dnf(iter)] generates any call
        let input_str = "struct User { #[dnf(iter)] field: LinkedList<String> }";
        let input: proc_macro2::TokenStream = input_str.parse().unwrap();

        let parsed: DeriveInput = syn::parse2(input).unwrap();
        let fields = match &parsed.data {
            Data::Struct(data) => match &data.fields {
                Fields::Named(fields) => &fields.named,
                _ => panic!("Expected named fields"),
            },
            _ => panic!("Expected struct"),
        };

        let field = fields.first().unwrap();
        let conversion = generate_value_conversion(field, field.ident.as_ref().unwrap(), &field.ty);
        let conversion_str = conversion.to_string();

        // Should use any with .iter()
        assert!(
            conversion_str.contains("any") && conversion_str.contains(". iter ()"),
            "Expected any with .iter(), got: {}",
            conversion_str
        );
    }

    #[test]
    fn test_iter_attribute_with_custom_method() {
        // Test that #[dnf(iter = "items")] generates any with custom method
        let input_str = "struct User { #[dnf(iter = \"items\")] field: CustomList<i32> }";
        let input: proc_macro2::TokenStream = input_str.parse().unwrap();

        let parsed: DeriveInput = syn::parse2(input).unwrap();
        let fields = match &parsed.data {
            Data::Struct(data) => match &data.fields {
                Fields::Named(fields) => &fields.named,
                _ => panic!("Expected named fields"),
            },
            _ => panic!("Expected struct"),
        };

        let field = fields.first().unwrap();
        let conversion = generate_value_conversion(field, field.ident.as_ref().unwrap(), &field.ty);
        let conversion_str = conversion.to_string();

        // Should use any with .items()
        assert!(
            conversion_str.contains("any") && conversion_str.contains(". items ()"),
            "Expected any with .items(), got: {}",
            conversion_str
        );
    }
}

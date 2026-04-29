# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-04-29

### Added

- `thiserror = "2"` dependency in `dnf` for `Display` and `std::error::Error` derivation.
- `FieldInfo::new` and `FieldInfo::with_kind` constructors, plus `name()` / `field_type()` / `kind()` accessors.

### Changed

- `DnfEvaluable::get_field_value` renamed to `field_value`. Every manual trait impl must be updated.
- `FieldInfo` fields (`name`, `field_type`, `kind`) privatized.
- `DnfError::UnknownField.position` widened from `usize` to `Option<usize>` so non-parser call sites can omit a source position.
- `DnfError` and `FieldKind` are now `#[non_exhaustive]`.
- Repository-wide doc-comment overhaul to follow RFC 1574 / Rust standard library conventions

### Removed

- `ParseError` — folded into `DnfError`.
- `DnfError::FieldNotFound` variant — `UnknownField` is now the single not-found variant for both parse-time and evaluation-time errors.

## [0.1.0] - 2026-04-15

### Added

- Initial release
- DNF query builder API with fluent interface
- `DnfQuery`, `Conjunction`, and `Condition` types
- `#[derive(DnfEvaluable)]` proc macro for automatic trait implementation
- Support for operators: `==`, `!=`, `>`, `<`, `>=`, `<=`, `CONTAINS`, `STARTS WITH`, `ENDS WITH`, `ALL OF`, `ANY OF`
- `Value` type with variants: `Int`, `Uint`, `Float`, `Bool`, `String`, and array types
- Cross-type numeric comparisons
- `Vec<T>` and `HashSet<T>` field support with zero-copy iteration
- `Option<T>` field support
- Nested struct queries via `#[dnf(nested)]` attribute
- Field renaming via `#[dnf(rename = "...")]`
- Field skipping via `#[dnf(skip)]`
- Optional `parser` feature for string query parsing
- Optional `serde` feature for serialization support

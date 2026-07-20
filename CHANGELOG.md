# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.2] - 2026-07-20

### Added

- `DnfQuery::parse::<T>(query)` for string parsing; `QueryBuilder::from_query` now delegates to it.
- `LICENSE-MIT` and `LICENSE-APACHE` files so published crates ship their license text.

### Fixed

- Parser errors no longer panic on non-ASCII queries (snippet now slices on char boundaries).
- Unregistered inverse custom operators (e.g. a typo'd `Op::not_custom`) no longer match every target.
- Mixed-sign and int/float arrays now parse (`[-5, 0, 5]`, `[1, 2.5]`) via numeric promotion.

### Changed

- `ANY OF` / `ALL OF` over strings allocate once per option instead of per (field, option) pair (`eval_vec_any_of` ~65 ns → ~27 ns).
- Pinned `dnf-derive` to `=0.2.2`; trimmed a trailing space from the SPDX `license` string.
- Simplify custom op logic (be same with defaults)

## [0.2.1] - 2026-05-26

### Added

- `MAX_SAFE_INTEGER_FOR_FLOAT` constant (`2^53`) — re-exported at the crate root and referenced by the `Int`/`Uint` ↔ `Float` comparison docs as the precision-loss boundary.
- Benchmark coverage for custom operators, map-field queries (`@keys` / `@values`), and the string-query parser; duplicate benches removed.

### Changed

- `DnfQuery`'s `PartialEq` now also compares the set of registered custom-operator names, not just the conjunctions. Two queries with different custom-op registrations are no longer treated as equal even when their conjunctions match.
- Parser rejects numeric literals containing more than one decimal point (e.g. `1.2.3`) with `DnfError::InvalidNumber` instead of accepting them silently.
- Internal refactors with no public-API impact: tokenizer keyword-reading helpers, parser numeric-literal helper, derive-macro restructuring, `OpRegistry` storage compaction, deduplicated `DnfQuery::merge` / `QueryBuilder::or_query` and `DnfQuery::validate` / `QueryBuilder::validate` paths.

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

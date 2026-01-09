# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

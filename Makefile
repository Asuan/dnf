## Makefile mirroring .github/workflows (checks.yml, ci.yml, benchmarks.yml).
## Run `make help` for the list of targets.

export CARGO_TERM_COLOR ?= always
export RUST_BACKTRACE   ?= 1

CARGO ?= cargo

.PHONY: help \
        ci checks \
        check clippy doc fmt fmt-fix \
        test test-all \
        features feat-default feat-none feat-serde feat-derive feat-parser-derive \
        bench \
        coverage \
        install-tools clean

help:
	@echo "Targets:"
	@echo "  make ci                 - run everything CI runs (checks + tests + feature matrix)"
	@echo "  make checks             - fast checks: check, clippy, doc, fmt"
	@echo "  make check              - cargo check (workspace, all-targets, all-features)"
	@echo "  make clippy             - cargo clippy with -D warnings"
	@echo "  make doc                - cargo doc with -D warnings"
	@echo "  make fmt                - cargo fmt --all --check"
	@echo "  make fmt-fix            - cargo fmt --all (apply formatting)"
	@echo "  make test               - cargo test --workspace --all-features"
	@echo "  make test-all           - test + full feature combinations matrix"
	@echo "  make features           - run the feature combinations matrix"
	@echo "  make bench              - cargo bench --all-features"
	@echo "  make coverage           - cargo llvm-cov, produces lcov.info"
	@echo "  make install-tools      - install cargo-llvm-cov"

# ---------------------------------------------------------------- aggregates

ci: checks test-all

checks: check clippy doc fmt

# --------------------------------------------------------------- fast checks
# (mirror of .github/workflows/checks.yml)

check:
	$(CARGO) check --workspace --all-targets --all-features

clippy:
	RUSTFLAGS="-D warnings" $(CARGO) clippy --all-targets --all-features --tests

doc:
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --workspace --no-deps --document-private-items --all-features

fmt:
	$(CARGO) fmt --all --check

fmt-fix:
	$(CARGO) fmt --all

# --------------------------------------------------------------------- tests
# (mirror of .github/workflows/ci.yml)

test:
	$(CARGO) test --workspace --all-features

test-all: test features

# Feature combinations matrix from ci.yml -> feature-combinations job.
features: feat-default feat-none feat-serde feat-derive feat-parser-derive

feat-default:
	$(CARGO) test --workspace

feat-none:
	$(CARGO) test --workspace --no-default-features

feat-serde:
	$(CARGO) test --workspace --no-default-features --features serde

feat-derive:
	$(CARGO) test --workspace --no-default-features --features derive

feat-parser-derive:
	$(CARGO) test --workspace --no-default-features --features "parser,derive"

# Coverage target (mirror of ci.yml linux-x64 step). Requires cargo-llvm-cov.
coverage:
	$(CARGO) llvm-cov --workspace --all-features --lcov --output-path lcov.info

# ---------------------------------------------------------------- benchmarks
# (mirror of .github/workflows/benchmarks.yml)

bench:
	$(CARGO) bench --all-features

# -------------------------------------------------------------- housekeeping

install-tools:
	$(CARGO) install cargo-llvm-cov

clean:
	$(CARGO) clean

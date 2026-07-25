# Coding Standards

Standards for the Jotphant codebase. These rules are binding for all code in this
repository. Keep this document in sync with `Cargo.toml` and any tooling config.

## Table of Contents

1. [Formatting & Lints](#1-formatting--lints)

<!-- Sections 2–4 (Naming & Structure, Error Handling, Docs/Tests/Comments) to follow. -->

## 1. Formatting & Lints

### Formatting

- All code MUST be formatted with `cargo fmt` using the default (stable) rustfmt
  settings. We deliberately keep **no `rustfmt.toml`** and accept upstream defaults
  to avoid configuration drift and nightly-only options.
- Run `cargo fmt` before every commit. Reviews assume formatted code.

### Lints

- The lint policy is declared in `Cargo.toml` under the `[lints]` table so it applies
  to every build and every contributor automatically — no need to remember
  command-line flags.
- `clippy::all` is denied and all compiler warnings are treated as errors:

  ```toml
  [lints.rust]
  warnings = "deny"

  [lints.clippy]
  all = "deny"
  ```

- Run `cargo clippy` before every commit; it MUST pass with zero warnings.
- Do not disable lints globally. If a specific lint genuinely must be suppressed, use
  a narrowly-scoped `#[allow(...)]` on the smallest possible item, with a comment
  explaining why.

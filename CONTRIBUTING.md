# Contributing to Jotphant

Thanks for your interest! This document covers the workflow and the expectations
for changes.

## Getting started

- Rust **1.85+** (edition 2024). No other system dependencies — SQLite is
  bundled.
- `cargo run` builds and starts the app. Your development database and config
  live in the platform data/config directories (see the README), not in the
  repository.

## Before every commit

All three must pass — CI enforces them on every push and PR:

```bash
cargo fmt --all -- --check
```

```bash
cargo clippy --all-targets
```

```bash
cargo test
```

Clippy runs at deny level (configured in `Cargo.toml`'s `[lints]` table): a
warning is a build failure. Do not disable lints globally; a narrowly-scoped,
justified `#[allow]` is the only accepted suppression.

## Commit messages: Conventional Commits (required)

Commits MUST follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)
(`feat:`, `fix:`, `docs:`, `chore:`, `test:`, `ci:`, `refactor:`, …with optional
scope, e.g. `feat(ui): …`).

This is not just style: **commit types drive release version numbers and the
changelog** via release-plz — a `feat:` raises the minor version, a fix-only
release raises the patch, and a `!`/`BREAKING CHANGE:` commit signals a major
bump. See [RELEASING.md](RELEASING.md).

## Code expectations

[CODING_STANDARDS.md](CODING_STANDARDS.md) is binding. The short version:

- **Layering** — dependencies point inward only: `ui → app → domain ← storage`.
  The domain stays pure (no I/O, no egui, no SQLite); ports (traits) are defined
  by the layer that consumes them.
- **Test-first for `domain` and `app`** — state machines, reward math, and
  atomic operations ship with their tests. Storage changes come with round-trip
  tests against an in-memory database; cross-cutting behavior belongs in
  `tests/`. UI is verified by running the app.
- **Errors** — `Result` everywhere, `thiserror` enums per layer, no `.unwrap()`
  outside tests.
- **No singletons** — dependencies are constructor-injected; `main.rs` is the
  only composition root.
- Tests must be deterministic: time is always passed in, never read from the
  wall clock inside domain logic.

## Pull requests

- Keep PRs small and focused — one logical change per PR.
- CI (fmt + clippy + tests, Windows runner) must be green.
- Update the relevant docs (`SCOPE.md`, `README.md`) in the same PR when
  behavior changes; the changelog is generated automatically from your commit
  messages.

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

## Adding a language

Translations are YAML catalogs under `locales/`, compiled into the binary. To add
one (say Portuguese, `pt`):

1. Copy `locales/en.yaml` to `locales/pt.yaml` and translate **only the values**
   — the keys and the `{placeholders}` inside values must stay exactly as they
   are (placeholders are replaced with numbers/text at runtime).
2. In `src/domain/config.rs`, add a `Portuguese` variant to `Language`: extend
   `ALL`, `code()` (`"pt"`, must match the file name), and `native_name()`
   (`"Português"`).
3. In `src/localization.rs`, add the catalog to `catalog_source`:
   `Language::Portuguese => include_str!("../locales/pt.yaml")`.
4. Run `cargo test` — a completeness test verifies your catalog has exactly the
   same key set as English and names anything missing or extra.

That's it: the language appears in Settings and is auto-selected for matching
system locales. English is the reference catalog — when you add a *new string*
to the app, add it to `en.yaml` first and to every other catalog (the test will
remind you).

## Pull requests

- Keep PRs small and focused — one logical change per PR.
- CI (fmt + clippy + tests, Windows runner) must be green.
- Update the relevant docs (`SCOPE.md`, `README.md`) in the same PR when
  behavior changes; the changelog is generated automatically from your commit
  messages.

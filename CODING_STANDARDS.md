# Coding Standards

Standards for the Jotphant codebase. These rules are binding for all code in this
repository. Keep this document in sync with `Cargo.toml` and any tooling config.

## Table of Contents

1. [Formatting & Lints](#1-formatting--lints)
2. [Naming & Structure](#2-naming--structure)
3. [Error Handling](#3-error-handling)
4. [Documentation, Tests & Comments](#4-documentation-tests--comments)
5. [Architecture & Design Principles](#5-architecture--design-principles)
6. [Type-Driven Design](#6-type-driven-design)
7. [Ownership, Mutability & Performance](#7-ownership-mutability--performance)

## 1. Formatting & Lints

### Formatting

- All code MUST be formatted with `cargo fmt` using the default (stable) rustfmt
  settings. We deliberately keep **no `rustfmt.toml`** and accept upstream defaults
  to avoid configuration drift and nightly-only options.
- Run `cargo fmt` before every commit. Reviews assume formatted code.
- The defaults already match the official Rust Style Guide (4-space indentation, spaces
  not tabs, 100-column max width) — do not hand-format against them.

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

## 2. Naming & Structure

### Crate layout

Jotphant is a **single crate** with both a library and a binary target:

- `src/lib.rs` — the library root; declares the top-level modules and their public API.
- `src/main.rs` — a **thin** binary entry point. It wires configuration, the database,
  and the eframe run loop, then delegates. No domain logic lives here.

### Modules & dependency direction

Code is organized into four layers. Dependencies point **inward only**:

```text
ui  ─►  app  ─►  domain  ◄─  storage
```

- `domain` — pure types, enums, the task state machine, and reward math. Depends on
  **nothing** project-specific and on no I/O, GUI, or database crates. No `egui`,
  `rusqlite`, or `chrono`-wall-clock calls leak in here.
- `storage` — SQLite (rusqlite) persistence. Implements repository **traits defined by
  `domain`**; `domain` never depends on `storage`.
- `app` — application services that orchestrate `domain` + `storage` and own the atomic
  operations (start/complete/cancel task, quick-jot).
- `ui` — egui views and widgets. Talks only to `app` services, never to `storage` or
  SQLite directly.

Do not create upward or sideways dependencies (e.g. `domain` importing `storage`, or
`ui` calling `storage`). If a rule feels like it needs one, the abstraction belongs in
`domain` as a trait.

### File organization

- Use the `foo.rs` + `foo/` submodule style. **Do not** use `mod.rs` files.
- One cohesive concept per file. Split a module when it grows past a comfortable read or
  starts mixing concerns (e.g. task types vs. timer types).
- Keep items **private by default**; expose the intended surface deliberately via `pub`
  and module re-exports (`pub use`).

### Naming conventions

Follow standard Rust naming (RFC 430); clippy enforces most of it:

- Types, traits, enums, enum variants: `UpperCamelCase`.
- Functions, methods, variables, modules, fields: `snake_case`.
- Constants and statics: `SCREAMING_SNAKE_CASE`.
- Avoid abbreviations (`estimated_pomos`, not `est_pomos`). Names state intent.

### Type-driven modelling

- Wrap entity identifiers in **newtypes** (`TaskId`, `NoteId`, `PomodoroSessionId`, …)
  rather than passing bare integers, so IDs of different entities cannot be confused.
- Prefer enums over stringly-typed values. `TaskStatus`, `TimerPhase`, `SessionStatus`,
  and `BankTransactionType` are enums — never raw strings — inside the domain. String
  conversion happens only at the storage/serialization boundary.
- Make illegal states unrepresentable where practical: derive effort from session
  history rather than storing an editable counter (see `SCOPE.md`).

### API design conventions

Follow the official Rust API Guidelines so public types feel native to the ecosystem:

- **Conversion prefixes (C-CONV).** Name conversions by cost and ownership: `as_*` = a
  cheap borrow (`&self → &T`), `to_*` = an owned value from a borrow (may allocate),
  `into_*` = consumes `self`.
- **Getters omit `get_` (C-GETTER).** A field accessor is `balance(&self)`, not
  `get_balance`. Reserve `get_*` for key/index lookups or computed/remote fetches.
- **Iterators (C-ITER).** Collections expose `iter()`, `iter_mut()`, `into_iter()`, with
  iterator types named `Iter`, `IterMut`, `IntoIter`.
- **Derive the common traits (C-COMMON-TRAITS).** Derive `Debug` on essentially every
  public type, plus `Clone`, `Default`, `PartialEq`/`Eq`, `PartialOrd`/`Ord`, `Hash`
  whenever they are meaningful.
- **Conversions via `From`/`Into` (C-CONV-TRAITS).** Implement `From` (you get `Into`
  for free) instead of bespoke `from_x`/`to_x` methods; use `TryFrom`/`TryInto` for
  fallible ones and `AsRef`/`AsMut` for cheap reference views.
- **Private fields (C-STRUCT-PRIVATE).** Struct fields stay private behind constructors
  and accessors; public fields freeze the layout and block future refactoring.
- **Serde only at the boundary (C-SERDE).** Keep `serde` derives on storage/DTO row
  types, not on core `domain` types, so the persistence format never dictates the domain
  shape.
- **Seal internal traits (C-SEALED).** A trait meant only for our own types (a marker,
  or a port with invariants) uses the sealed pattern (private supertrait) to prevent
  outside implementations.

## 3. Error Handling

### Result vs. panic

- All fallible operations return `Result<T, E>`. I/O, database access, config parsing,
  and user-input validation are **always** fallible — model them as `Result`, never a
  panic.
- Panics are reserved for **programmer errors** — broken internal invariants that a
  caller cannot cause. They are not a control-flow tool.

### `.unwrap()` / `.expect()` policy

- `.unwrap()` is **forbidden** in non-test code.
- `.expect("…")` is allowed in non-test code **only** for an invariant that is provably
  infallible, and the message must explain *why* it cannot fail (e.g.
  `.expect("migrations are embedded at compile time")`). Prefer `?` and real error
  propagation wherever a failure is actually possible.
- In tests, `.unwrap()` / `.expect()` are fine.
- Do not ship `todo!`, `unimplemented!`, or `panic!` in reachable paths. `unreachable!`
  is permitted only with a comment justifying why the branch cannot occur.

### Error types

- Each library layer (`domain`, `storage`, `app`) defines its own concrete error
  **enum** using [`thiserror`]. Callers can then match on specific failure modes.
- **Do not use `anyhow` (or other type-erased errors) in the library layers.** A
  type-erased error is acceptable only at the binary boundary (`src/main.rs`) for
  top-level reporting, if we choose to add it.
- Map errors when crossing a layer (e.g. a `storage::Error` becomes an
  `app::Error::Storage`) so each layer exposes a vocabulary its callers understand.
- Error messages (thiserror `#[error("…")]`) are lowercase, without trailing
  punctuation, and include the relevant context.

### Propagation & conversions

- Propagate with `?`; add context at layer boundaries rather than swallowing errors.
- Never silently ignore a `Result`. If an error is genuinely irrelevant, discard it
  explicitly with a comment.
- Do not use `as` for potentially-lossy numeric casts (e.g. `u32`↔`i32` for pomo
  counts/ledger amounts); use `TryFrom`/`TryInto` and handle the error. `clippy`'s cast
  lints back this up.

### Transactions

- Multi-step domain operations that must be atomic (task completion, cancellation) run
  inside a single database transaction and return `Result`. Any error aborts the
  transaction so **nothing** partial persists, exactly as specified in `SCOPE.md`.

### Destructors (Drop / RAII)

- Resources are released deterministically via RAII — prefer scope-end release over
  manual teardown.
- **A `Drop` impl MUST NOT panic (C-DTOR-FAIL).** A panic while another panic is
  unwinding aborts the whole process.
- **A `Drop` impl MUST NOT block (C-DTOR-BLOCK).** `drop` runs synchronously; no blocking
  I/O or long work inside it. If a resource needs fallible or lengthy teardown, expose an
  explicit `close()` / `shutdown()` method and treat `Drop` as a best-effort fallback.

## 4. Documentation, Tests & Comments

### Documentation

- Every public item (module, type, trait, function) carries a doc comment (`///`, and
  `//!` for module-level overviews) describing its purpose and any invariants or
  panics — **what and why**, not a restatement of the code.
- Keep docs truthful and current; update them in the same change that alters behavior.
- Crate-wide `missing_docs` enforcement is deferred for v1, but new public API should be
  documented as it is written.
- Where an example aids understanding, include a **runnable** doc example (C-EXAMPLE);
  examples use the `?` operator, not `.unwrap()` (C-QUESTION-MARK), so they also model
  correct error handling.

### Comments

- Comments explain **why** — intent, trade-offs, non-obvious constraints — not **what**
  the code already says. Match the comment density of the surrounding code.
- Delete stale or commented-out code rather than leaving it; git history is the archive.

### Tests

- **Test-first** for `domain` and `app`: write the unit tests alongside or before the
  logic (state-machine transitions, reward math, atomic operations). UI is verified
  manually.
- Unit tests live in an in-module `#[cfg(test)] mod tests`. Cross-layer and storage
  tests (against an **in-memory** SQLite database) live in `tests/`.
- Test names describe behavior: `state_condition_expectedresult`
  (e.g. `start_task_when_another_active_is_rejected`).
- Cover, at minimum: every valid **and rejected** state transition; the single-active
  task invariant; reward calculation; the completion/cancellation transactions
  **including rollback on failure**; config round-trip; migrations; and the wiki-link
  parser.
- Tests must be **deterministic and fast**. Inject time (a clock abstraction or explicit
  timestamps) instead of reading the wall clock, and model timer progress as a pure
  function of elapsed duration — **never** `sleep` to advance a timer in a test.
- Any test helpers, fakes, or synthetic-data builders a module exposes for other
  modules' tests are gated behind a `test-util` feature (M-TEST-UTIL) so they never
  compile into the shipping binary.

## 5. Architecture & Design Principles

### Dependency injection — no singletons

- **No global mutable state.** `static mut` is forbidden, and `once_cell` /
  `lazy_static` / thread-locals MUST NOT be used as a service locator or global
  singleton for services, the database connection, configuration, the clock, or the
  notifier.
- **Constructor injection.** A type receives its collaborators as fields through its
  `new(...)` (or a builder). It does not reach out to a global to fetch them. If a
  function needs a dependency, it takes it as a parameter.
- **Depend on abstractions, not concretions.** Services accept the repository/port
  **traits defined in `domain`**, never a concrete SQLite type. Prefer generic bounds
  (`fn new(repo: R) where R: TaskRepository`) for zero-cost static dispatch; use
  `Arc<dyn Trait>` / `Box<dyn Trait>` only when runtime polymorphism or heterogeneous
  storage is genuinely required.
- **`src/main.rs` is the composition root** — the single place that constructs concrete
  implementations (SQLite repositories, loaded config, real clock/notifier) and wires
  them into `app` services, which are then handed to the UI. Nothing deeper in the tree
  news up its own infrastructure.
- This is what makes `domain` and `app` testable: tests inject in-memory or fake
  implementations of the same traits (see §4).

### SOLID

- **S — Single Responsibility.** Each type and module has one reason to change. The
  layer split in §2 is the coarse expression of this; within a layer, a struct that
  both computes rewards *and* writes SQL is a smell — separate them.
- **O — Open/Closed.** Extend behavior by adding a trait implementation or an enum
  variant, not by editing unrelated code. Closed sets (`TaskStatus`, `TimerPhase`) are
  enums; open sets (repositories, notifiers, audio backends) are traits.
- **L — Liskov Substitution.** Every trait implementation MUST honor the trait's
  documented contract — preconditions, postconditions, and error semantics — so any
  implementation, real or fake, is interchangeable. The tests depend on this.
- **I — Interface Segregation.** Prefer several small, capability-focused traits over
  one fat "god repository". A caller that only reads tasks depends on a read-only port,
  not on write/delete it never uses. Split ports by capability
  (`TaskRepository`, `NoteRepository`, `BankLedger`, …).
- **D — Dependency Inversion.** High-level policy (`domain`, `app`) depends on
  abstractions **it owns**; low-level detail (`storage`, notifications, audio)
  implements them. Ports are declared in the layer that consumes them; adapters live at
  the edge. This is the inward-only dependency rule from §2.

### Clean code

- Small, single-purpose functions. Keep nesting shallow with early returns and `?`
  rather than deep `if`/`match` pyramids.
- **Immutability by default:** prefer `let` over `let mut`, and `&self` over `&mut self`
  unless mutation is actually required.
- Keep `domain` **pure** — no I/O or side effects. Side effects live in `storage`,
  `ui`, and `main`. Pure logic is trivially testable and reusable.
- **Don't over-abstract speculatively.** Introduce a trait or generic when there is a
  real second implementation or a real test seam — not "just in case". Avoid duplication
  (DRY), but a little duplication is cheaper than the wrong abstraction.
- **Encapsulate invariants.** Expose the minimum `pub` surface; guard construction so
  illegal states cannot be built from outside the module (e.g. validate on `new`).

## 6. Type-Driven Design

Push correctness into the type system so the compiler rejects invalid states instead of
relying on runtime checks.

- **Parse, don't validate.** Convert untrusted input (config files, DB rows, UI text)
  into strongly-typed domain values **once, at the boundary**. Downstream code then
  receives values that are correct by construction and does not re-check them — a parsed
  `EstimatedPomos` beats a raw `u32` that every function must re-validate.
- **NewType wrappers.** Beyond IDs (§2), wrap any primitive whose meaning matters
  (durations, pomo counts, reward rates) in a tuple struct so unrelated values cannot be
  swapped. The wrapper is erased at compile time — zero runtime cost.
- **Make illegal states unrepresentable.** Model bounded, closed sets as enums with
  per-variant payloads rather than a bag of optional fields or boolean flags, so only
  valid combinations can be constructed.
- **TypeState where it fits.** For in-memory flows whose stage is known at compile time
  (a builder, or a not-yet-persisted session under construction), consider the TypeState
  pattern (zero-sized marker generics) so invalid transitions fail to compile. Note: our
  persisted `TaskStatus` is inherently **runtime** data loaded from SQLite, so it stays a
  validated enum plus a transition function (see the domain state machine) — TypeState is
  not a substitute there.
- **Composition over inheritance — no `Deref` abuse.** Model "has-a" with fields and
  shared behavior with traits. **Never** implement `Deref` on a domain struct to fake
  field inheritance; `Deref` is only for genuine smart pointers, and inherent methods on
  smart-pointer wrappers are avoided (C-SMART-PTR) to prevent method-resolution
  surprises.

## 7. Ownership, Mutability & Performance

- **Immutability and downward data flow.** Default to immutable bindings and let data
  flow down through ownership rather than sharing mutable references widely. Scope
  mutability as tightly as possible.
- **Clone deliberately.** Do not reach for `.clone()` to silence the borrow checker,
  especially in hot paths (per-frame egui rendering, timer ticks) — it adds allocations
  and hurts cache locality. Prefer borrowing or restructuring ownership. Cloning a small
  `Copy`-ish value or a genuine owned handoff is fine; cloning large collections every
  frame is not.
- **Interior mutability is a last resort.** Avoid reflexively wrapping state in
  `Rc<RefCell<T>>` or `Arc<Mutex<T>>`; it moves borrow checking to runtime and can panic
  (`RefCell`) or deadlock (`Mutex`). Use it only for a real shared-ownership need, not to
  dodge ownership design.
- **Zero-cost abstractions.** Favor static dispatch (generics, `impl Trait`) when the
  type is known; reserve `dyn` trait objects for genuine runtime polymorphism.

### Concurrency & async (future-proofing)

Jotphant is **synchronous** today — rusqlite + egui on the main thread, no async runtime
— so async cancellation-safety rules do not apply to current code. **If** async is ever
introduced (e.g. a background sync worker), it MUST follow these rules:

- Do not hold a lock guard across an `.await` point.
- Keep operational state in persistent structures (channels/actors), not inside
  short-lived futures, so a dropped/cancelled future cannot corrupt state.
- Coordinate shutdown with a `CancellationToken` / `watch` channel; do not rely on
  `JoinHandle::abort()`.
- Keep CPU-bound work off executor threads (`spawn_blocking`) or yield periodically.

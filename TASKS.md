# Jotphant — Task List

A living checklist of the build. Each unchecked item is **one reviewed piece = one
commit**; mark it `[x]` once that commit lands. Milestones are refined into finer steps
just before we start them, so later details may shift.

See [`SCOPE.md`](SCOPE.md) for the product scope and
[`CODING_STANDARDS.md`](CODING_STANDARDS.md) for the engineering rules.

## Foundations

- [x] Project scaffold + `CLAUDE.md` guide
- [x] `.gitignore` for Rust (build artifacts, IDE, local `CLAUDE.md`; `Cargo.lock` tracked)
- [x] `SCOPE.md` — v1 product scope (tasks + Pomodoro + bank + notes)
- [x] `CODING_STANDARDS.md` §1 Formatting & Lints + `Cargo.toml` `[lints]`
- [x] `CODING_STANDARDS.md` §2–§4 (Naming & Structure, Error Handling, Docs/Tests/Comments)
- [x] `CODING_STANDARDS.md` §5 Architecture & Design Principles (DI, SOLID, clean code)
- [x] `CODING_STANDARDS.md` §6–§7 (Type-Driven Design; Ownership/Mutability/Performance)
- [x] Implementation plan + this task list

## M0 — Walking skeleton (thin end-to-end: create → start → timer → complete → bank)

- [x] 0.1 Boot egui window + module scaffold (`domain`/`storage`/`app`/`ui`) — *adds `eframe`*
- [x] 0.2 Domain core (test-first): entities + enums, transition validation, reward math
- [x] 0.3 SQLite storage (test-first, in-memory): connection, migrations
      (`tasks`/`pomodoro_sessions`/`bank_transactions`), repository traits + rusqlite impl —
      *adds `rusqlite` bundled (`chrono` landed in 0.2)*
- [x] 0.4 App services (test-first): `create_task`, `start_task` (single active), record
      completed focus session, atomic `complete_task` → bank credit
- [x] 0.5 Minimal egui UI wired end-to-end: create/list task, Start, live countdown,
      Complete, bank balance

## M1 — Full task & timer domain

- [ ] 1.1 `paused` status + all transitions; single-active-task auto-pause on switch (test-first)
- [ ] 1.2 Pomodoro cycle engine: Focus→ShortBreak→LongBreak, `long_break_after`,
      auto-start rules, skip-break, abandon-pomo (test-first)
- [ ] 1.3 Cancellation flow: discard unbanked, keep sessions as history (test-first)
- [ ] 1.4 Timer persistence & restore across restart from timestamps (test-first + wiring)

## M2 — Configuration

- [ ] 2.1 TOML config load/save + defaults (`[pomodoro]`/`[rewards]`) (test-first) —
      *adds `serde`, `toml`*
- [ ] 2.2 In-app settings screen to edit config

## M3 — Notes

- [ ] 3.1 Notes domain + storage: `Note`, tags, links; migrations (test-first)
- [ ] 3.2 Notes UI: list, Markdown edit + preview, search, tags, pin/archive —
      *adds a markdown renderer*
- [ ] 3.3 `[[wiki-link]]` parsing + backlinks view (test-first parser)
- [ ] 3.4 Quick-jot box during active focus → attaches note to active task

## M4 — Notifications & polish

- [ ] 4.1 Desktop notification + sound at phase transitions — *adds `notify-rust` + an audio crate*
- [ ] 4.2 History / reports view
- [ ] 4.3 Linked follow-up tasks + editable estimate

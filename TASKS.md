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

## M0.5 — Trello-style board (UI redesign)

4 columns (`Todo · In Progress · Paused · Done`; Cancelled hidden), action buttons first,
click-to-open card detail, task descriptions.

- [x] B1 Add `description` to `Task` (domain + storage migration v2), test-first
- [x] B2 App `pause_task` / `cancel_task` services (board needs them), test-first
- [x] B3 Board layout: 4 status columns of cards with action buttons (replaces flat UI)
- [x] B4 Card detail modal (view/edit description, timer, actions)
- [x] B5 Drag-and-drop between columns

## M1 — Full task & timer domain

- [x] 1.1 `paused` status + all transitions; single-active-task auto-pause on switch (test-first)
- [x] 1.2 Pomodoro cycle engine: Focus→ShortBreak→LongBreak, `long_break_after`,
      auto-start rules, skip-break, abandon-pomo (test-first)
- [x] 1.3 Cancellation flow: discard unbanked, keep sessions as history (verified)
- [x] 1.4 Timer persistence & restore across restart from timestamps (test-first + wiring)

## M2 — Configuration

- [x] 2.1 TOML config load/save + defaults (`[pomodoro]`/`[rewards]`) (test-first) —
      *adds `serde`, `toml`*
- [x] 2.2 In-app settings screen to edit config (+ manual start-next-phase when auto-start off)

## M3 — Notes

- [x] 3.1 Notes domain + storage: `Note`, tags, links; migrations (test-first)
- [x] 3.2 Notes UI: list, Markdown edit + preview, search, tags, pin/archive —
      *adds `egui_commonmark`*
- [x] 3.3 `[[wiki-link]]` parsing + backlinks view (test-first parser)
- [x] 3.4 Quick-jot box during active focus → attaches note to active task

## M4 — Notifications & polish

- [x] 4.1 Desktop notification + sound at phase transitions — *adds `notify-rust` (toast sound; no audio crate)*
- [x] 4.2 History / reports view
- [x] 4.3 Linked follow-up tasks + editable estimate

## M5 — UX & Juice (Trello-level fluency)

Warm theme, fluent drag-and-drop with ghost card, Enter-first inputs, comment-grade
jots, subtle motion.

- [x] 5.1 Warm theme (light + dark) + toggle in Settings, persisted in config
- [x] 5.2 Drag rework: whole-card drag with click threshold, ghost card, drop-target
      highlight, invalid-drop feedback
- [x] 5.3 Enter-first creation: task add submits on Enter, inputs keep focus
- [x] 5.4 Jots as comments: relative timestamps, Markdown, edit/delete, focus retention;
      task jots hidden from the Notes tab
- [x] 5.5 Subtle motion pass: hover lift, smooth transitions, pomo-complete flash

## M6 — Release pipeline

- [x] R1 Release-build polish: no console window in release, `[profile.release]` (LTO, strip)
- [x] R2 `ci.yml`: fmt-check + clippy + tests on every push/PR (Windows runner)
- [x] R3 `release.yml`: `v*` tag → build, zip, publish GitHub Release + release walkthrough
- [x] R4 Automated versioning: release-plz Release PR (fix→patch, feat→minor), RELEASING.md rules

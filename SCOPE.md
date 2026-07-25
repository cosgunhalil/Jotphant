# Jotphant — Product Scope (v1)

Jotphant is a **desktop application that combines a Pomodoro-based task tracker with
a Markdown note-taking tool**. Focused work on tasks earns bankable "pomo" credits;
notes provide a free-form capture space. Tasks and notes are independent features that
live in one app, bridged only by a lightweight quick-jot that can attach a note to the
task you are actively focusing on.

This document is the authoritative scope reference for v1. Keep it in sync as decisions
change.

## 1. Platform & Stack

- **GUI:** egui / eframe (pure Rust, immediate mode) — a live timer redraws trivially.
- **Storage:** SQLite (single local database file).
- **Configuration:** TOML file, editable from an in-app settings screen.
- **Users:** single local user; no accounts, sync, or multi-user in v1.

## 2. Core Product Loop (Tasks)

Create a task → move it to **In Progress** (Pomodoro auto-starts) → completed focus
pomos accumulate → **complete** the task → pomos are credited to your **bank**.

> Select a task, start working automatically, finish the task, and bank the pomos you
> earned. Jotphant measures work and calculates earned leisure; it does not monitor how
> that leisure is spent.

## 3. Task Model & States

States: `todo`, `in_progress`, `paused`, `done`, `cancelled`.

Valid transitions:

- `todo → in_progress`
- `in_progress → paused`
- `in_progress → done`
- `in_progress → cancelled`
- `paused → in_progress`
- `paused → done`
- `paused → cancelled`

`done` and `cancelled` are **terminal** — a task cannot be reopened. Follow-up work is a
**new task**, optionally linked to the original (`linked_from_task_id`).

**Single active task invariant:** at most one task is `in_progress` at a time. Moving
another task to `in_progress` **auto-pauses** the current active task — its completed
pomos are retained; its in-flight pomo is abandoned.

## 4. Timer — single, task-bound, strict auto

- One application-level focus timer, always bound to the active task.
- Moving a task to `in_progress` is atomic: verify no other active task → set status →
  create a Pomodoro session → start the timer. The user never starts a timer separately.
- **Strict measurement:**
  - A pomo completes **only** when its timer reaches zero — there is no manual
    "Complete Pomo" control.
  - Breaks auto-start; the next focus pomo auto-starts after a break.
  - **Pause:** no effort accrues.
  - **Abandon** current pomo: partial time is discarded.
  - **Skip break:** allowed — ends the current break and jumps to focus.
- **Cycle:** `Focus → Short Break → Focus → … → Long Break` every `long_break_after`
  completed focus pomos.
- **Phases:** `Focus`, `ShortBreak`, `LongBreak`. Only `Focus` counts as effort.
- **Persistence:** the running timer survives app restart — the active session and
  elapsed time are reconstructed from stored timestamps.
- **Notifications:** desktop notification **+ sound** at phase transitions.

## 5. Bank & Rewards

- **Authoritative unit: pomos.** 1 completed focus pomo = 1 bank credit.
- Modeled as a **ledger** of signed `BankTransaction`s. v1 is **earn-only** — no
  redeem/spend action yet, but the ledger shape supports adding it later.
- **Reward timing:** pending while the task is active → **credited only on Done** →
  **discarded on Cancel**. This preserves the incentive to finish.
- **Display:** leisure minutes are **derived** from `leisure_minutes_per_pomo`
  (e.g. 11 pomos × 5 = 55 min); pomos remain the source of truth.
- **Cancelled tasks** keep their focus sessions as historical measured effort but earn
  **0** credit.

## 6. Completion & Cancellation — atomic

**Completion** (all-or-nothing transaction):

1. Verify the task is `in_progress` or `paused`.
2. Stop the timer; abandon any incomplete current pomo.
3. Count completed focus sessions not yet banked.
4. Mark the task `done`.
5. Append a `TaskReward` bank transaction (`+N` pomos).
6. Commit — if any step fails, nothing persists.

**Cancellation:**

1. Stop the timer; discard the incomplete pomo.
2. Discard completed-but-unbanked pomos (0 credit).
3. Mark the task `cancelled`; completed sessions remain in history.

## 7. Notes — independent Markdown notebook

- Notes are a **first-class, standalone feature**, not filed under tasks.
- **Content:** Markdown, with a rendered preview.
- **Optional task link:** a note may carry a nullable `task_id`, normally empty
  (independent). The **quick-jot** box shown during an active focus session is the one
  place that sets it, attaching the jotted note to the active task.
- **v1 note features:**
  - Full-text search across titles and bodies.
  - Tags / labels.
  - Note-to-note wiki links and backlinks (`[[...]]`).
  - Pin & archive.

## 8. Configuration (TOML)

```toml
[pomodoro]
focus_minutes = 25
short_break_minutes = 5
long_break_minutes = 15
long_break_after = 4
auto_start_break = true
auto_start_focus = true

[rewards]
leisure_minutes_per_pomo = 5
```

## 9. Data Model (indicative)

```text
Task            { id, title, status, estimated_pomos, created_at, completed_at, linked_from_task_id? }
PomodoroSession { id, task_id, phase, status, configured_duration_seconds, started_at, finished_at }
BankTransaction { id, task_id?, amount_pomos: i32, transaction_type, created_at }
Note            { id, title, body_markdown, task_id?, pinned, archived, created_at, updated_at }
NoteTag         { note_id, tag }
NoteLink        { from_note_id, to_note_id }
```

Enums:

- `TaskStatus`: `Todo`, `InProgress`, `Paused`, `Done`, `Cancelled`
- `TimerPhase`: `Focus`, `ShortBreak`, `LongBreak`
- `SessionStatus`: `Running`, `Completed`, `Abandoned`
- `BankTransactionType`: `TaskReward` (extensible; e.g. future `Spend`)

**Derived effort:** `completed_pomos = count(PomodoroSession where phase = Focus and
status = Completed)`. Effort is never stored as an editable counter on `Task`; "banked"
is tracked via the ledger, not a session flag, so re-banking is impossible.

## 10. v1 Scope Boundaries

**In:** tasks + strict Pomodoro + pomo bank (earn-only); Markdown notes with search,
tags, backlinks, pin/archive; quick-jot onto the active task; history / reports view;
linked follow-up tasks; editable estimate; skip break; TOML config with in-app settings;
timer persistence across restart; desktop notifications + sound.

**Deferred:** bank spending / redeem; multi-user & accounts; cloud sync; mobile;
rich-text (WYSIWYG) notes; note attachments / images.

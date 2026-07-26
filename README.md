# Jotphant 🐘🍅

A warm desktop companion that combines a Trello-style task board, a strict
Pomodoro timer, and a Markdown notebook. Focused work earns pomos; finishing a
task banks them as leisure time you can feel good about spending.

**The core loop:** create a task → drag it to *In Progress* (the timer starts by
itself) → focus pomos accumulate → complete the task → its pomos land in your
bank. Jotphant measures the work and computes the earned leisure — how you spend
it is up to you.

## Features

- **Task board** — Todo / In Progress / Paused / Done columns with fluent drag &
  drop: ghost card, drop-target highlights, invalid columns fade. Quick-click a
  card to open its detail.
- **Strict Pomodoro engine** — focus → short break → focus → long break, fully
  automatic; only one task is ever active, and starting another auto-pauses the
  current one. A running timer survives an app restart.
- **Pomo bank** — completing a task credits its completed focus pomos; cancelled
  work keeps its history but earns nothing. The bank shows the equivalent
  leisure minutes at your configured exchange rate.
- **Jots** — Trello-style comments on every task: Markdown, relative timestamps,
  edit and delete, submitted with Enter.
- **Notebook** — standalone Markdown notes with live preview, full-text search,
  tags, `[[wiki-links]]` with backlinks, pin and archive.
- **Warm themes** — cream-and-amber light or charcoal-and-amber dark, switchable
  in Settings.
- **Desktop notifications** at every phase transition, and a history view with
  per-task measured effort.

## Install

**Windows (x86_64):** download the latest zip from
[Releases](../../releases), extract, and run `Jotphant.exe`.

**From source** (Rust 1.85+):

```bash
cargo run --release
```

## Where your data lives

| What | Where |
|---|---|
| Database (tasks, sessions, bank, notes) | `%APPDATA%\jotphant\Jotphant\data\jotphant.db` |
| Configuration | `%APPDATA%\jotphant\Jotphant\config\config.toml` |

Durations, the long-break cadence, auto-start behavior, the leisure exchange
rate, and the theme are all editable in-app (Settings) or directly in
`config.toml`.

## Development

```bash
cargo test         # 106 tests: domain, storage, services, UI helpers, integration
```

```bash
cargo clippy       # deny-level lints, enforced in CI
```

```bash
cargo fmt          # rustfmt defaults
```

Built with [egui/eframe](https://github.com/emilk/egui) and
[rusqlite](https://github.com/rusqlite/rusqlite), in a layered architecture
(`ui → app → domain ← storage`).

Project documentation:

- [SCOPE.md](SCOPE.md) — the product scope and domain rules
- [CODING_STANDARDS.md](CODING_STANDARDS.md) — binding engineering standards
- [CONTRIBUTING.md](CONTRIBUTING.md) — how to contribute
- [RELEASING.md](RELEASING.md) — versioning rules and the release pipeline
- [CHANGELOG.md](CHANGELOG.md) — auto-generated release history

## License

[MIT](LICENSE)

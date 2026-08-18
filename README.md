# Jotphant

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
- **Speaks your language** — English, Türkçe, Español, Azərbaycanca; picked up
  from your system locale on first run, switchable in Settings. Adding a
  language is a single YAML file — see
  [CONTRIBUTING.md](CONTRIBUTING.md#adding-a-language).
- **Gentle phase alerts** — the taskbar flashes and the card lights up when a
  focus session or break ends (no OS notification pop-ups), plus a history view
  with per-task measured effort and a due-date timeline.

## Install

**Windows (x86_64):** download the latest zip from
[Releases](../../releases), extract, and run `Jotphant.exe`.

**From source** (Rust 1.85+):

```bash
cargo run --release
```

### A note on antivirus warnings

Windows Defender sometimes flags new releases with a generic machine-learning
detection (e.g. `Wacatac!ml`). This is a **false positive** common to new,
unsigned open-source binaries: Jotphant's releases are built automatically by
[public GitHub Actions CI](../../actions) straight from the tagged source code —
there is no hand-built binary involved. We report each false positive to
Microsoft, keep the app free of behavior that trips heuristics (no network
access, no OS notification APIs, no installers — a single portable exe), and
will pursue code signing as the project grows.

You can cryptographically verify that a download was built by this repository's
CI (requires the [GitHub CLI](https://cli.github.com)):

```bash
gh attestation verify jotphant-v0.2.1-windows-x86_64.zip --repo cosgunhalil/Jotphant
```

If in doubt, you can also scan the file at [VirusTotal](https://www.virustotal.com)
or build from source yourself.

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

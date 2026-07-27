# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/cosgunhalil/Jotphant/releases/tag/v0.1.0) - 2026-07-26

### Added

- *(ui)* hover lift and pomo-complete flash
- *(ui)* make task jots trello-style comments
- *(ui)* add enter-first input flow
- *(ui)* add trello-style drag with ghost card and drop feedback
- *(ui)* warm light/dark themes with settings toggle
- store data in platform dirs and configure the window
- editable estimate and linked follow-up tasks
- *(notify)* desktop notification at phase transitions
- *(ui)* show task jots as comments in the card detail
- *(notes)* parse wiki-links and show backlinks
- *(ui)* add the notes screen with markdown preview
- *(notes)* add note domain and SQLite storage
- *(ui)* add a settings screen and manual phase start
- *(config)* load and save configuration as TOML
- restore the running timer across restarts
- add the pomodoro cycle engine
- *(app)* auto-pause the active task when starting another
- *(ui)* add a card detail modal
- *(ui)* replace flat UI with a Trello-style board
- *(app)* add pause and cancel task services
- *(domain)* add task description with schema migration
- *(ui)* wire the walking skeleton end-to-end
- *(app)* add task-workflow services with atomic completion
- *(storage)* add SQLite store, migrations, and repository ports
- *(domain)* add core entities, state machine, and reward math
- scaffold app shell and module layout

### Fixed

- *(ui)* move quick-jot into the card detail modal
- *(ui)* separate card click-to-open from drag

### Other

- Update project title in README
- add README, CONTRIBUTING, and changelog scaffold
- automate versioning with release-plz
- publish a GitHub release on version tags
- add fmt, clippy, and test checks
- polish release binaries
- add migration and file persistence integration tests
- *(app)* verify cancellation semantics
- add project task list
- fold Rust best practices into coding standards
- add v1 product scope
- add coding standards with formatting & lint policy

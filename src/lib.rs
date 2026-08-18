//! Jotphant — a desktop Pomodoro task tracker with a Markdown notebook.
//!
//! The crate is organised into layers whose dependencies point inward
//! (`ui → app → domain ← storage`); see `CODING_STANDARDS.md` §2.

pub mod app;
pub mod domain;
pub mod localization;
pub mod storage;
pub mod ui;

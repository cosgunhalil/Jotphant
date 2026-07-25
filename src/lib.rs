//! Jotphant — a desktop Pomodoro task tracker with a Markdown notebook.
//!
//! The crate is organised into four layers whose dependencies point inward
//! (`ui → app → domain ← storage`); see `CODING_STANDARDS.md` §2. Each module is a
//! stub for now and is filled in by later pieces.

pub mod app;
pub mod domain;
pub mod storage;
pub mod ui;

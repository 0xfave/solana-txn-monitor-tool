#![allow(dead_code, unused_imports)]
//! TUI layer stubs. Application state, screens and event handling.

pub mod screens;

/// Application state (minimal stub).
pub struct AppState {
    pub running: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self { running: true }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }
}

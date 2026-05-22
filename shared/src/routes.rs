//! API route paths shared between server and TUI.
//!
//! These are the JSON API routes the TUI client calls. As more endpoints are
//! added for the TUI, their paths should be defined here so both crates stay
//! in sync.
//!
//! As a convention, all TUI/JSON endpoints should be prefixed with `api/tui`.

/// GET dashboard summary (JSON).
pub const DASHBOARD: &str = "/api/tui/dashboard";

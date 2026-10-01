//! Intégration Claude Code.
//!
//! - [`hook`] : le relais lancé par Claude Code à chaque événement
//!   (`boringwindows hook`). Il résume l'événement, l'envoie à l'app via
//!   [`ipc`] et rend la main en ≤ 300 ms si l'app ne répond pas.
//! - [`ClaudeModule`] : côté app, reçoit les événements, suit les sessions
//!   ([`tracker`]) et répond aux demandes de permission.
//! - [`install`] : ajoute/retire nos hooks dans `~/.claude/settings.json`.

mod config;
pub mod doctor;
pub mod event;
pub mod hook;
pub mod install;
pub mod ipc;
mod module;
pub mod tracker;

pub use config::ClaudeConfig;
pub use module::{ClaudeModule, MODULE_ID};
pub use tracker::{PromptView, SessionKind, SessionView, Snapshot};

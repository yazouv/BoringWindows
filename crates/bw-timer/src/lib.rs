//! Minuteur : quelques durées prédéfinies (Pomodoro, cuisson…), compte à
//! rebours dans l'île, alerte à la fin.
//!
//! Pas de tic chaque seconde : le module ne se réveille qu'à la minute (pour
//! la pilule) et à la fin ; l'île calcule elle-même les secondes pendant
//! qu'elle est ouverte.

mod config;
mod machine;
mod module;

pub use config::TimerConfig;
pub use machine::{Phase, TimerSnapshot};
pub use module::{MODULE_ID, TimerModule};

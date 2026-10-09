//! Notifications des applications (Discord, Slack, Teams…) : chaque nouvelle
//! notification Windows s'annonce dans l'île, et les dernières restent dans
//! son onglet « Notifications ».
//!
//! Sous Windows : lecture du centre de notifications via
//! `UserNotificationListener` (autorisé par Paramètres › Confidentialité ›
//! Notifications). Ailleurs, le module ne fait rien.

mod config;
mod inbox;
#[cfg(windows)]
mod listener;
mod module;

pub use config::NotifyConfig;
pub use inbox::{Notification, NotifySnapshot, brand_color, summary};
pub use module::{MODULE_ID, NotifyModule};

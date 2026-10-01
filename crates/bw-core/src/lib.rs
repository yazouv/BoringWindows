//! Noyau de BoringWindows : modules, bus d'événements et arbitrage d'attention.
//!
//! L'UI ne fait qu'afficher un état. Les modules (Claude, musique, calendrier…)
//! tournent sur un runtime tokio dédié et publient des [`ModuleEvent`] ; l'
//! [`Arbiter`] décide lequel occupe la pilule en mode compact.

mod attention;
mod host;

pub use attention::{Arbiter, Attention, Winner};
pub use host::{Action, Module, ModuleCtx, ModuleEvent, ModuleEventKind, ModuleHost};

//! Plugins WASM : des modules tiers sandboxés qui publient une ligne de texte
//! (et un niveau d'attention) dans l'île. Voir docs/*/src/plugins.md pour l'ABI.

mod config;
mod manifest;
mod module;
mod runtime;

pub use config::PluginsConfig;
pub use manifest::{Manifest, PluginSpec, discover, plugins_dir};
pub use module::{MODULE_ID, PluginItem, PluginsModule, PluginsSnapshot};
pub use runtime::{MAX_TEXT_CHARS, Output, Plugin};

//! Musique en cours, via les contrôles média du système.
//!
//! Sous Windows : `GlobalSystemMediaTransportControlsSessionManager` (GSMTC),
//! qui couvre Spotify, Apple Music, les navigateurs, VLC… sans clé d'API.
//! Sous macOS : Spotify et Musique, en AppleScript, réveillés par leurs
//! notifications distribuées.
//! Tout est événementiel : aucun polling.

// Logique pure : testée partout, utilisée sous macOS seulement.
#[cfg(any(test, target_os = "macos"))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod applescript;
pub mod artwork;
mod config;
#[cfg(windows)]
mod gsmtc;
#[cfg(target_os = "macos")]
mod macos;
mod module;
mod snapshot;

pub use artwork::Artwork;
pub use config::MediaConfig;
pub use module::{MODULE_ID, MediaModule};
pub use snapshot::{MediaSnapshot, NowPlaying, display_name, ignore_token};

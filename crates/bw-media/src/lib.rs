//! Musique en cours, via les contrôles média du système.
//!
//! Sous Windows : `GlobalSystemMediaTransportControlsSessionManager` (GSMTC),
//! qui couvre Spotify, Apple Music, les navigateurs, VLC… sans clé d'API.
//! Tout est événementiel : aucun polling.

pub mod artwork;
mod config;
#[cfg(windows)]
mod gsmtc;
mod module;
mod snapshot;

pub use artwork::Artwork;
pub use config::MediaConfig;
pub use module::{MODULE_ID, MediaModule};
pub use snapshot::{MediaSnapshot, NowPlaying, display_name, ignore_token};

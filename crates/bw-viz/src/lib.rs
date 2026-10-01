//! Visualiseur audio : capture « loopback » du son joué par le système
//! (WASAPI), analysé en bandes de fréquences pour animer l'île.
//!
//! C'est la seule fonction qui travaille en continu : elle est donc éteinte par
//! défaut, et ne tourne que lorsque l'île est ouverte et qu'une musique joue
//! (l'application l'active et la coupe par les actions `start` / `stop`).

#[cfg(windows)]
mod capture;
mod config;
#[cfg_attr(not(windows), allow(dead_code))]
mod dsp;
mod module;

pub use config::VizConfig;
pub use module::{MODULE_ID, VizModule, VizSnapshot};

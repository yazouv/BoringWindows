//! Volume du système : chaque changement (touches du clavier, mélangeur,
//! application) s'affiche un instant dans la pilule. Événementiel : un
//! callback de l'API audio de Windows, aucun polling.

mod config;
mod module;
#[cfg(windows)]
mod wasapi;

pub use config::VolumeConfig;
pub use module::{MODULE_ID, VolumeModule};

/// Texte affiché : « Volume 45 % » ou « Muet ».
pub fn label(level: f32, muted: bool) -> String {
    if muted || level <= 0.0 {
        bw_i18n::tr!("Muted", "Muet")
    } else {
        bw_i18n::tr!("Volume {} %", "Volume {} %", (level.clamp(0.0, 1.0) * 100.0).round() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        bw_i18n::set(bw_i18n::Lang::Fr);
        assert_eq!(label(0.456, false), "Volume 46 %");
        assert_eq!(label(0.5, true), "Muet");
        assert_eq!(label(0.0, false), "Muet");
        assert_eq!(label(1.7, false), "Volume 100 %");
    }
}

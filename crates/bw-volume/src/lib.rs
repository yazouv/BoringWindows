//! Volume du système et luminosité de l'écran intégré : chaque changement
//! (touches du clavier, mélangeur, centre de notifications, application)
//! s'affiche un instant dans la pilule. Le volume passe par un callback de
//! l'API audio de Windows, la luminosité par un événement WMI.

mod config;
mod module;
#[cfg(windows)]
mod wasapi;
#[cfg(windows)]
mod wmi;

pub use config::VolumeConfig;
pub use module::{BRIGHTNESS_ID, MODULE_ID, VolumeModule};

/// Monte ou baisse le volume principal de `step` (-1 à 1) : nouveau niveau et
/// état muet. Windows seulement ; à appeler sur le thread UI (COM initialisé).
pub fn nudge(step: f32) -> anyhow::Result<(f32, bool)> {
    #[cfg(windows)]
    return wasapi::nudge(step);
    #[cfg(not(windows))]
    {
        let _ = step;
        anyhow::bail!("volume : Windows seulement")
    }
}

/// Texte affiché : « Volume 45 % » ou « Muet ».
pub fn label(level: f32, muted: bool) -> String {
    if muted || level <= 0.0 {
        bw_i18n::tr!("Muted", "Muet")
    } else {
        bw_i18n::tr!(
            "Volume {} %",
            "Volume {} %",
            (level.clamp(0.0, 1.0) * 100.0).round() as u32
        )
    }
}

/// Texte affiché pour la luminosité : « Luminosité 70 % ».
pub fn brightness_label(level: u8) -> String {
    bw_i18n::tr!("Brightness {} %", "Luminosité {} %", level.min(100))
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
        assert_eq!(brightness_label(70), "Luminosité 70 %");
        assert_eq!(brightness_label(255), "Luminosité 100 %");
    }
}

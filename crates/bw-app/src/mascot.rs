//! La mascotte : un petit blob à yeux dans la pilule. Son humeur suit Claude
//! Code, la musique, l'heure et ta présence ; elle se déguise selon la saison.
//!
//! Tout ici est pur : l'heure, la date et l'état sont passés en paramètres.
//! L'UI (`island.slint`, composant `Mascot`) dessine l'humeur reçue.

use bw_i18n::tr;
use serde::Deserialize;

/// Section `[modules.mascot]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MascotConfig {
    pub enabled: bool,
    /// Toujours animée (respire, cligne des yeux) ; sinon seulement quand il
    /// se passe quelque chose (Claude, musique), pour rester à 0 % au repos.
    pub always_animated: bool,
    /// La mascotte et la pilule bougent au rythme des basses.
    pub music: bool,
    /// Déguisements et décorations de saison (Halloween, Noël, Nouvel An).
    pub seasonal: bool,
    /// Minutes sans clavier ni souris avant qu'elle s'endorme.
    pub sleep_after_minutes: u32,
    /// Accessoire de la grande mascotte (voir `ACCESSORIES`).
    pub accessory: String,
    /// Couleur du corps (« #RRGGBB ») ; vide : celle du texte du thème.
    pub color: String,
}

/// Accessoires possibles, dans l'ordre de la liste des réglages.
pub const ACCESSORIES: [&str; 5] = ["sprout", "glasses", "headphones", "bow", "none"];

impl Default for MascotConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            always_animated: false,
            music: true,
            seasonal: true,
            sleep_after_minutes: 10,
            accessory: "sprout".into(),
            color: String::new(),
        }
    }
}

impl MascotConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            (1..=240).contains(&config.sleep_after_minutes),
            tr!(
                "modules.mascot.sleep_after_minutes must be between 1 and 240",
                "modules.mascot.sleep_after_minutes doit être entre 1 et 240"
            )
        );
        anyhow::ensure!(
            ACCESSORIES.contains(&config.accessory.as_str()),
            tr!(
                "modules.mascot.accessory must be one of: {}",
                "modules.mascot.accessory doit valoir : {}",
                ACCESSORIES.join(", ")
            )
        );
        anyhow::ensure!(
            config.color.is_empty() || config.color.parse::<bw_config::Color>().is_ok(),
            tr!(
                "modules.mascot.color must be a color like \"#FFB3C7\" (or empty)",
                "modules.mascot.color doit être une couleur comme « #FFB3C7 » (ou vide)"
            )
        );
        Ok(config)
    }

    /// Couleur du corps choisie, s'il y en a une.
    pub fn body_color(&self) -> Option<bw_config::Color> {
        self.color.parse().ok()
    }
}

/// Ce que fait Claude, du plus pressant au moins pressant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeState {
    /// Une session attend une réponse (permission, question).
    Waiting,
    Working,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mood {
    Idle,
    /// Le matin : yeux rieurs.
    Morning,
    /// Le soir : yeux mi-clos.
    Evening,
    /// Personne au clavier depuis un moment.
    Sleep,
    Work,
    Wait,
    Happy,
    Music,
}

impl Mood {
    /// Nom passé à l'UI (`Mascot.mood`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Morning => "morning",
            Self::Evening => "evening",
            Self::Sleep => "sleep",
            Self::Work => "work",
            Self::Wait => "wait",
            Self::Happy => "happy",
            Self::Music => "music",
        }
    }

    /// Humeurs qui bougent même en mode « animée seulement quand utile ».
    pub fn is_lively(self) -> bool {
        matches!(self, Self::Work | Self::Wait | Self::Happy | Self::Music)
    }
}

/// Humeur du moment. Claude passe avant la musique, la musique avant le
/// sommeil, le sommeil avant l'heure de la journée.
pub fn mood(claude: Option<ClaudeState>, music: bool, away: bool, hour: u32) -> Mood {
    match claude {
        Some(ClaudeState::Waiting) => return Mood::Wait,
        Some(ClaudeState::Working) => return Mood::Work,
        Some(ClaudeState::Done) => return Mood::Happy,
        None => {}
    }
    if music {
        Mood::Music
    } else if away {
        Mood::Sleep
    } else if !(6..21).contains(&hour) {
        Mood::Evening
    } else if hour < 11 {
        Mood::Morning
    } else {
        Mood::Idle
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Season {
    None,
    Halloween,
    Christmas,
    NewYear,
}

impl Season {
    /// Nom passé à l'UI (`Mascot.season`), vide hors saison.
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Halloween => "halloween",
            Self::Christmas => "christmas",
            Self::NewYear => "newyear",
        }
    }
}

/// Saison du jour : Halloween en octobre (jusqu'au 1er novembre), Noël du
/// 1er au 26 décembre, Nouvel An du 30 décembre au 2 janvier.
pub fn season(month: u32, day: u32) -> Season {
    match (month, day) {
        (10, _) | (11, 1) => Season::Halloween,
        (12, 1..=26) => Season::Christmas,
        (12, 30..) | (1, 1..=2) => Season::NewYear,
        _ => Season::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_first_then_music_then_sleep_then_time() {
        assert_eq!(mood(Some(ClaudeState::Waiting), true, true, 12), Mood::Wait);
        assert_eq!(
            mood(Some(ClaudeState::Working), true, false, 12),
            Mood::Work
        );
        assert_eq!(mood(Some(ClaudeState::Done), false, false, 12), Mood::Happy);
        assert_eq!(mood(None, true, true, 23), Mood::Music);
        assert_eq!(mood(None, false, true, 12), Mood::Sleep);
        assert_eq!(mood(None, false, false, 22), Mood::Evening);
        assert_eq!(mood(None, false, false, 3), Mood::Evening);
        assert_eq!(mood(None, false, false, 8), Mood::Morning);
        assert_eq!(mood(None, false, false, 15), Mood::Idle);
        assert!(Mood::Music.is_lively() && !Mood::Sleep.is_lively());
    }

    #[test]
    fn seasons() {
        assert_eq!(season(10, 10), Season::Halloween);
        assert_eq!(season(11, 1), Season::Halloween);
        assert_eq!(season(11, 2), Season::None);
        assert_eq!(season(12, 24), Season::Christmas);
        assert_eq!(season(12, 28), Season::None);
        assert_eq!(season(12, 31), Season::NewYear);
        assert_eq!(season(1, 2), Season::NewYear);
        assert_eq!(season(7, 14), Season::None);
    }

    /// Les sections des modèles de config.toml valent les valeurs par défaut.
    #[test]
    fn default_templates_match_defaults() {
        for template in [bw_config::DEFAULT_TOML, bw_config::DEFAULT_TOML_EN] {
            let c = bw_config::Config::from_toml_str(template).unwrap();
            assert_eq!(
                MascotConfig::from_table(c.modules.get("mascot")).unwrap(),
                MascotConfig::default()
            );
            assert_eq!(
                crate::gestures::GesturesConfig::from_table(c.modules.get("gestures")).unwrap(),
                crate::gestures::GesturesConfig::default()
            );
            assert_eq!(
                bw_presence::PresentationConfig::from_table(c.modules.get(bw_presence::MODULE_ID))
                    .unwrap(),
                bw_presence::PresentationConfig::default()
            );
        }
    }

    #[test]
    fn config_validation() {
        let c = MascotConfig::from_table(None).unwrap();
        assert!(c.enabled && !c.always_animated && c.music && c.seasonal);
        assert_eq!(c.accessory, "sprout");
        assert!(c.body_color().is_none());
        let t: toml::Table = toml::from_str(
            "accessory = \"glasses\"
color = \"#FFB3C7\"",
        )
        .unwrap();
        let c = MascotConfig::from_table(Some(&t)).unwrap();
        assert_eq!(c.accessory, "glasses");
        assert!(c.body_color().is_some());
        for bad in [
            "sleep_after_minutes = 0",
            "danse = true",
            "accessory = \"chapeau\"",
            "color = \"rose\"",
        ] {
            let t: toml::Table = toml::from_str(bad).unwrap();
            assert!(MascotConfig::from_table(Some(&t)).is_err(), "{bad}");
        }
    }
}

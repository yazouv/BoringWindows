use std::time::Duration;

use bw_i18n::tr;
use serde::Deserialize;

/// Section `[modules.calendar]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CalendarConfig {
    pub enabled: bool,
    /// Calendriers à suivre (`[[modules.calendar.sources]]`).
    pub sources: Vec<Source>,
    /// Intervalle de rafraîchissement (minutes).
    pub refresh_minutes: u32,
    /// Rappel avant le début d'une réunion (minutes).
    pub remind_minutes: u32,
    /// Afficher les événements « toute la journée ».
    pub show_all_day: bool,
    /// Horizon de l'agenda (heures).
    pub lookahead_hours: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Nom affiché (facultatif).
    #[serde(default)]
    pub name: String,
    /// Lien ICS privé (https:// ou webcal://) ou chemin d'un fichier .ics.
    pub url: String,
}

impl Default for CalendarConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sources: Vec::new(),
            refresh_minutes: 10,
            remind_minutes: 5,
            show_all_day: true,
            lookahead_hours: 24,
        }
    }
}

impl CalendarConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            (2..=1440).contains(&config.refresh_minutes),
            tr!(
                "modules.calendar.refresh_minutes must be between 2 and 1440",
                "modules.calendar.refresh_minutes doit être entre 2 et 1440"
            )
        );
        anyhow::ensure!(
            config.remind_minutes <= 120,
            tr!(
                "modules.calendar.remind_minutes must be ≤ 120",
                "modules.calendar.remind_minutes doit être ≤ 120"
            )
        );
        anyhow::ensure!(
            (1..=168).contains(&config.lookahead_hours),
            tr!(
                "modules.calendar.lookahead_hours must be between 1 and 168",
                "modules.calendar.lookahead_hours doit être entre 1 et 168"
            )
        );
        for s in &config.sources {
            anyhow::ensure!(
                !s.url.trim().is_empty(),
                tr!(
                    "modules.calendar.sources: empty url",
                    "modules.calendar.sources : url vide"
                )
            );
        }
        Ok(config)
    }

    pub fn refresh(&self) -> Duration {
        Duration::from_secs(u64::from(self.refresh_minutes) * 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sources() {
        assert_eq!(
            CalendarConfig::from_table(None).unwrap(),
            CalendarConfig::default()
        );
        let t: toml::Table = toml::from_str(
            "remind_minutes = 10\n[[sources]]\nname = \"Pro\"\nurl = \"webcal://x/y.ics\"\n[[sources]]\nurl = \"C:/a.ics\"",
        )
        .unwrap();
        let c = CalendarConfig::from_table(Some(&t)).unwrap();
        assert_eq!(c.sources.len(), 2);
        assert_eq!(c.sources[1].name, "");
        assert_eq!(c.remind_minutes, 10);

        let bad: toml::Table = toml::from_str("refresh_minutes = 1").unwrap();
        assert!(CalendarConfig::from_table(Some(&bad)).is_err());
        let empty: toml::Table = toml::from_str("[[sources]]\nurl = \" \"").unwrap();
        assert!(CalendarConfig::from_table(Some(&empty)).is_err());
    }
}

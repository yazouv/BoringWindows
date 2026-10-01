use std::time::Duration;

use bw_i18n::tr;
use serde::Deserialize;

/// Section `[modules.timer]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TimerConfig {
    /// Désactivé par défaut : le minuteur n'apparaît que si on le veut.
    pub enabled: bool,
    /// Durées proposées dans l'île, en minutes.
    pub presets: Vec<u32>,
    /// Son à la fin du minuteur.
    pub sound: bool,
    /// Combien de temps l'île signale la fin (secondes).
    pub done_secs: u32,
}

impl Default for TimerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            presets: vec![5, 15, 25],
            sound: true,
            done_secs: 20,
        }
    }
}

impl TimerConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            !config.presets.is_empty() && config.presets.len() <= 5,
            tr!(
                "modules.timer.presets: 1 to 5 durations",
                "modules.timer.presets : 1 à 5 durées"
            )
        );
        anyhow::ensure!(
            config.presets.iter().all(|m| (1..=600).contains(m)),
            tr!(
                "modules.timer.presets: each duration between 1 and 600 minutes",
                "modules.timer.presets : chaque durée entre 1 et 600 minutes"
            )
        );
        anyhow::ensure!(
            (1..=600).contains(&config.done_secs),
            tr!(
                "modules.timer.done_secs must be between 1 and 600",
                "modules.timer.done_secs doit être entre 1 et 600"
            )
        );
        Ok(config)
    }

    pub fn done_for(&self) -> Duration {
        Duration::from_secs(self.done_secs.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_validation() {
        let c = TimerConfig::from_table(None).unwrap();
        assert!(!c.enabled);
        assert_eq!(c.presets, [5, 15, 25]);
        let t: toml::Table = toml::from_str("enabled = true\npresets = [10, 50]").unwrap();
        assert_eq!(TimerConfig::from_table(Some(&t)).unwrap().presets, [10, 50]);
        for bad in [
            "presets = []",
            "presets = [0]",
            "presets = [601]",
            "presets = [1,2,3,4,5,6]",
            "done_secs = 0",
            "preset = [5]",
        ] {
            let t: toml::Table = toml::from_str(bad).unwrap();
            assert!(TimerConfig::from_table(Some(&t)).is_err(), "{bad}");
        }
    }
}

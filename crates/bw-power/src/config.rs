use std::time::Duration;

use bw_i18n::tr;
use serde::Deserialize;

/// Section `[modules.battery]` ou `[modules.bluetooth]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PowerConfig {
    pub enabled: bool,
    /// Durée d'affichage d'un changement (secondes).
    pub show_secs: u32,
    /// Seuil de batterie faible (%).
    pub low_percent: u8,
}

impl Default for PowerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            show_secs: 4,
            low_percent: 20,
        }
    }
}

impl PowerConfig {
    pub fn from_table(id: &str, table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            (1..=10).contains(&config.show_secs),
            tr!(
                "modules.{}.show_secs must be between 1 and 10",
                "modules.{}.show_secs doit être entre 1 et 10",
                id
            )
        );
        anyhow::ensure!(
            (5..=50).contains(&config.low_percent),
            tr!(
                "modules.{}.low_percent must be between 5 and 50",
                "modules.{}.low_percent doit être entre 5 et 50",
                id
            )
        );
        Ok(config)
    }

    pub fn show_for(&self) -> Duration {
        Duration::from_secs(self.show_secs.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_validation() {
        let c = PowerConfig::from_table("battery", None).unwrap();
        assert!(c.enabled);
        assert_eq!(c.show_for(), Duration::from_secs(4));
        assert_eq!(c.low_percent, 20);
        for bad in [
            "show_secs = 0",
            "low_percent = 60",
            "low_percent = 2",
            "seuil = 2",
        ] {
            let t: toml::Table = toml::from_str(bad).unwrap();
            assert!(
                PowerConfig::from_table("battery", Some(&t)).is_err(),
                "{bad}"
            );
        }
    }
}

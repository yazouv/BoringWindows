use std::time::Duration;

use bw_i18n::tr;
use serde::Deserialize;

/// Section `[modules.system]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SystemConfig {
    pub enabled: bool,
    /// Rafraîchissement pendant que l'île est ouverte (secondes).
    pub refresh_secs: u32,
    /// Alerte quand le processeur reste au-dessus de ce seuil (%) ; 0 : jamais.
    pub cpu_alert_percent: u8,
    /// Alerte quand la mémoire reste au-dessus de ce seuil (%) ; 0 : jamais.
    pub ram_alert_percent: u8,
    /// Durée au-dessus du seuil avant l'alerte (secondes).
    pub alert_after_secs: u32,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            refresh_secs: 2,
            cpu_alert_percent: 0,
            ram_alert_percent: 0,
            alert_after_secs: 30,
        }
    }
}

impl SystemConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            (1..=10).contains(&config.refresh_secs),
            tr!(
                "modules.system.refresh_secs must be between 1 and 10",
                "modules.system.refresh_secs doit être entre 1 et 10"
            )
        );
        for (key, value) in [
            ("cpu_alert_percent", config.cpu_alert_percent),
            ("ram_alert_percent", config.ram_alert_percent),
        ] {
            anyhow::ensure!(
                value == 0 || (50..=100).contains(&value),
                tr!(
                    "modules.system.{}: 0 (off) or between 50 and 100",
                    "modules.system.{} : 0 (désactivé) ou entre 50 et 100",
                    key
                )
            );
        }
        anyhow::ensure!(
            (5..=600).contains(&config.alert_after_secs),
            tr!(
                "modules.system.alert_after_secs must be between 5 and 600",
                "modules.system.alert_after_secs doit être entre 5 et 600"
            )
        );
        Ok(config)
    }

    pub fn refresh(&self) -> Duration {
        Duration::from_secs(self.refresh_secs.into())
    }

    pub fn alert_after(&self) -> Duration {
        Duration::from_secs(self.alert_after_secs.into())
    }

    /// Une alerte au moins est active : il faut mesurer même île fermée.
    pub fn alerts(&self) -> bool {
        self.cpu_alert_percent > 0 || self.ram_alert_percent > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> anyhow::Result<SystemConfig> {
        let t: toml::Table = toml::from_str(text).unwrap();
        SystemConfig::from_table(Some(&t))
    }

    #[test]
    fn defaults_and_validation() {
        let c = SystemConfig::from_table(None).unwrap();
        assert!(c.enabled);
        assert!(!c.alerts());
        assert_eq!(c.refresh(), Duration::from_secs(2));
        assert!(parse("cpu_alert_percent = 90").unwrap().alerts());
        for bad in [
            "refresh_secs = 0",
            "refresh_secs = 60",
            "cpu_alert_percent = 30",
            "ram_alert_percent = 101",
            "alert_after_secs = 1",
            "cpu = true",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}

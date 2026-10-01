use std::time::Duration;

use bw_i18n::tr;
use serde::Deserialize;

/// Section `[modules.visualizer]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VizConfig {
    /// Éteint par défaut : seule fonction qui consomme en continu.
    pub enabled: bool,
    /// Nombre de barres.
    pub bands: usize,
    /// Images par seconde pendant que l'île est ouverte et que ça joue.
    pub fps: u32,
}

impl Default for VizConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bands: 12,
            fps: 30,
        }
    }
}

impl VizConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            (4..=32).contains(&config.bands),
            tr!(
                "modules.visualizer.bands must be between 4 and 32",
                "modules.visualizer.bands doit être entre 4 et 32"
            )
        );
        anyhow::ensure!(
            (10..=60).contains(&config.fps),
            tr!(
                "modules.visualizer.fps must be between 10 and 60",
                "modules.visualizer.fps doit être entre 10 et 60"
            )
        );
        Ok(config)
    }

    pub fn frame(&self) -> Duration {
        Duration::from_millis(1000 / u64::from(self.fps))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_validation() {
        let c = VizConfig::from_table(None).unwrap();
        assert!(!c.enabled);
        assert_eq!(c.frame(), Duration::from_millis(33));
        for bad in ["bands = 3", "bands = 33", "fps = 9", "fps = 61", "band = 8"] {
            let t: toml::Table = toml::from_str(bad).unwrap();
            assert!(VizConfig::from_table(Some(&t)).is_err(), "{bad}");
        }
    }
}

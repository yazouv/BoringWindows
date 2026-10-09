use std::time::Duration;

use bw_i18n::tr;
use serde::Deserialize;

/// Section `[modules.volume]` (ou `[modules.brightness]`) de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VolumeConfig {
    /// Désactivé par défaut : Windows affiche déjà son propre indicateur.
    pub enabled: bool,
    /// Durée d'affichage après un changement (secondes).
    pub show_secs: u32,
}

impl Default for VolumeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            show_secs: 2,
        }
    }
}

impl VolumeConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        Self::from_module_table(crate::MODULE_ID, table)
    }

    /// Même section pour un autre module (`brightness`), messages compris.
    pub fn from_module_table(id: &str, table: Option<&toml::Table>) -> anyhow::Result<Self> {
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
        let c = VolumeConfig::from_table(None).unwrap();
        assert!(!c.enabled);
        assert_eq!(c.show_for(), Duration::from_secs(2));
        for bad in ["show_secs = 0", "show_secs = 11", "show = 2"] {
            let t: toml::Table = toml::from_str(bad).unwrap();
            assert!(VolumeConfig::from_table(Some(&t)).is_err(), "{bad}");
        }
    }
}

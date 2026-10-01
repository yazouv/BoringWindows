use bw_i18n::tr;
use serde::Deserialize;

/// Section `[modules.plugins]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PluginsConfig {
    /// Désactivé par défaut : on n'exécute du code tiers que si on le demande.
    pub enabled: bool,
    /// Plugins à lancer (noms de dossiers) ; vide = tous ceux du dossier `plugins/`.
    pub only: Vec<String>,
}

impl PluginsConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            config.only.iter().all(|n| crate::manifest::valid_name(n)),
            tr!(
                "modules.plugins.only: folder names (letters, digits, - and _)",
                "modules.plugins.only : noms de dossiers (lettres, chiffres, - et _)"
            )
        );
        Ok(config)
    }

    pub fn allows(&self, folder: &str) -> bool {
        self.only.is_empty() || self.only.iter().any(|n| n == folder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_filter() {
        let c = PluginsConfig::from_table(None).unwrap();
        assert!(!c.enabled && c.allows("n'importe-quoi"));
        let t: toml::Table = toml::from_str("only = [\"a\", \"b-2\"]").unwrap();
        let c = PluginsConfig::from_table(Some(&t)).unwrap();
        assert!(c.allows("a") && !c.allows("c"));
        let bad: toml::Table = toml::from_str("only = [\"../x\"]").unwrap();
        assert!(PluginsConfig::from_table(Some(&bad)).is_err());
    }
}

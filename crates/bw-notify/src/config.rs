use std::time::Duration;

use bw_i18n::tr;
use serde::Deserialize;

/// Section `[modules.notifications]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NotifyConfig {
    pub enabled: bool,
    /// Durée d'affichage d'une nouvelle notification dans la pilule (secondes).
    pub show_secs: u32,
    /// Montrer l'expéditeur et le message ; sinon, seulement le nom de l'appli.
    pub show_content: bool,
    /// Applications à ignorer (morceau du nom, sans tenir compte de la casse).
    pub ignore: Vec<String>,
    /// Un clic rejoue la notification depuis le centre de notifications, pour
    /// arriver au bon endroit (salon Discord, onglet…) ; sinon, il ouvre
    /// seulement l'application.
    pub open_original: bool,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            show_secs: 5,
            show_content: true,
            ignore: Vec::new(),
            open_original: true,
        }
    }
}

impl NotifyConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            (1..=30).contains(&config.show_secs),
            tr!(
                "modules.notifications.show_secs must be between 1 and 30",
                "modules.notifications.show_secs doit être entre 1 et 30"
            )
        );
        Ok(config)
    }

    pub fn show_for(&self) -> Duration {
        Duration::from_secs(self.show_secs.into())
    }

    /// L'application `app` est-elle dans la liste `ignore` ?
    pub fn ignores(&self, app: &str) -> bool {
        let app = app.to_lowercase();
        self.ignore
            .iter()
            .map(|i| i.trim().to_lowercase())
            .any(|i| !i.is_empty() && app.contains(&i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_validation() {
        let c = NotifyConfig::from_table(None).unwrap();
        assert!(c.enabled && c.show_content && c.open_original);
        assert_eq!(c.show_for(), Duration::from_secs(5));
        for bad in ["show_secs = 0", "show_secs = 31", "sound = true"] {
            let t: toml::Table = toml::from_str(bad).unwrap();
            assert!(NotifyConfig::from_table(Some(&t)).is_err(), "{bad}");
        }
    }

    #[test]
    fn ignore_matches_part_of_the_name() {
        let t: toml::Table = toml::from_str(r#"ignore = ["docker", " ", "Intel"]"#).unwrap();
        let c = NotifyConfig::from_table(Some(&t)).unwrap();
        assert!(c.ignores("Docker Desktop"));
        assert!(c.ignores("Intel Driver & Support Assistant"));
        assert!(!c.ignores("Discord"));
    }
}

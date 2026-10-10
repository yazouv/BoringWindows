use serde::Deserialize;

/// Section `[modules.presentation]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PresentationConfig {
    /// Détecter appels et partages d'écran : annonces tues, point « en direct ».
    pub enabled: bool,
    /// L'île n'apparaît jamais dans les partages d'écran, enregistrements et
    /// captures (y compris les tiennes).
    pub hide_from_capture: bool,
    /// Applications dont l'usage du micro veut dire « en appel » (morceau de
    /// nom). La capture d'écran compte quelle que soit l'application.
    pub call_apps: Vec<String>,
}

impl Default for PresentationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hide_from_capture: true,
            call_apps: [
                "teams", "discord", "zoom", "slack", "webex", "skype", "chrome", "msedge",
                "firefox", "brave", "opera", "obs",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}

impl PresentationConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        Ok(match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_unknown_keys() {
        let c = PresentationConfig::from_table(None).unwrap();
        assert!(c.enabled && c.hide_from_capture);
        assert!(c.call_apps.iter().any(|a| a == "discord"));
        let t: toml::Table = toml::from_str("cacher = true").unwrap();
        assert!(PresentationConfig::from_table(Some(&t)).is_err());
    }
}

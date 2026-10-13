use std::time::Duration;

use bw_i18n::tr;
use serde::Deserialize;

/// Section `[modules.github]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GithubConfig {
    pub enabled: bool,
    /// Jeton d'accès : `secret:<id>` (coffre de l'OS, ce que font les réglages)
    /// ou le jeton lui-même. Vide : la session du GitHub CLI, si `use_gh_cli`.
    pub token: String,
    /// Sans jeton, demander le sien à `gh auth token` (GitHub CLI).
    pub use_gh_cli: bool,
    /// Intervalle entre deux relevés (secondes).
    pub refresh_secs: u32,
    /// Annoncer le résultat de la CI de tes pull requests.
    pub announce_ci: bool,
    /// Annoncer une revue demandée.
    pub announce_reviews: bool,
    /// Durée d'affichage d'une annonce (secondes).
    pub show_secs: u32,
}

impl Default for GithubConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            token: String::new(),
            use_gh_cli: true,
            refresh_secs: 120,
            announce_ci: true,
            announce_reviews: true,
            show_secs: 6,
        }
    }
}

impl GithubConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            (30..=3600).contains(&config.refresh_secs),
            tr!(
                "modules.github.refresh_secs must be between 30 and 3600",
                "modules.github.refresh_secs doit être entre 30 et 3600"
            )
        );
        anyhow::ensure!(
            (2..=30).contains(&config.show_secs),
            tr!(
                "modules.github.show_secs must be between 2 and 30",
                "modules.github.show_secs doit être entre 2 et 30"
            )
        );
        Ok(config)
    }

    pub fn refresh(&self) -> Duration {
        Duration::from_secs(self.refresh_secs.into())
    }

    pub fn show_for(&self) -> Duration {
        Duration::from_secs(self.show_secs.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> anyhow::Result<GithubConfig> {
        let t: toml::Table = toml::from_str(text).unwrap();
        GithubConfig::from_table(Some(&t))
    }

    #[test]
    fn defaults_and_validation() {
        let c = GithubConfig::from_table(None).unwrap();
        assert!(!c.enabled && c.use_gh_cli && c.announce_ci && c.announce_reviews);
        assert_eq!(c.refresh(), Duration::from_secs(120));
        assert_eq!(
            parse("token = \"secret:github-1\"").unwrap().token,
            "secret:github-1"
        );
        for bad in [
            "refresh_secs = 10",
            "refresh_secs = 99999",
            "show_secs = 1",
            "use_gh_cli = \"oui\"",
            "organisation = \"x\"",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}

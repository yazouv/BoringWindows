use std::time::Duration;

use serde::Deserialize;

/// Section `[modules.claude]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClaudeConfig {
    pub enabled: bool,
    /// Répondre aux demandes de permission depuis l'île.
    pub permissions: bool,
    pub permission_wait_secs: u32,
    pub done_secs: u32,
    /// Son système quand une session se met à t'attendre (question, permission…).
    pub sound: bool,
    /// Rejoue le son toutes les N secondes tant qu'une session attend (0 = jamais).
    pub remind_secs: u32,
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            permissions: true,
            permission_wait_secs: 60,
            done_secs: 8,
            sound: true,
            remind_secs: 30,
        }
    }
}

impl ClaudeConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            (5..=280).contains(&config.permission_wait_secs),
            bw_i18n::tr!(
                "modules.claude.permission_wait_secs must be between 5 and 280",
                "modules.claude.permission_wait_secs doit être entre 5 et 280"
            )
        );
        anyhow::ensure!(
            config.done_secs <= 600,
            bw_i18n::tr!(
                "modules.claude.done_secs must be ≤ 600",
                "modules.claude.done_secs doit être ≤ 600"
            )
        );
        anyhow::ensure!(
            config.remind_secs == 0 || (5..=3600).contains(&config.remind_secs),
            bw_i18n::tr!(
                "modules.claude.remind_secs must be 0 (off) or between 5 and 3600",
                "modules.claude.remind_secs doit être 0 (désactivé) ou entre 5 et 3600"
            )
        );
        Ok(config)
    }

    pub fn permission_wait(&self) -> Duration {
        Duration::from_secs(self.permission_wait_secs.into())
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
        assert_eq!(
            ClaudeConfig::from_table(None).unwrap(),
            ClaudeConfig::default()
        );
        let t: toml::Table = toml::from_str("permissions = false\ndone_secs = 3").unwrap();
        let c = ClaudeConfig::from_table(Some(&t)).unwrap();
        assert!(!c.permissions);
        assert_eq!(c.done_secs, 3);
        assert_eq!(c.permission_wait_secs, 60);

        let bad: toml::Table = toml::from_str("permission_wait_secs = 1000").unwrap();
        assert!(ClaudeConfig::from_table(Some(&bad)).is_err());
        let typo: toml::Table = toml::from_str("permision = true").unwrap();
        assert!(ClaudeConfig::from_table(Some(&typo)).is_err());
    }
}

use serde::Deserialize;

/// Section `[modules.media]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MediaConfig {
    pub enabled: bool,
    /// Teinter l'île avec la couleur de la pochette.
    pub accent_from_artwork: bool,
    /// Sources à ignorer (morceau de nom, ex. "chrome", "msedge").
    pub ignore: Vec<String>,
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            accent_from_artwork: true,
            ignore: Vec::new(),
        }
    }
}

impl MediaConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        Ok(match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        })
    }

    pub fn is_ignored(&self, source_id: &str) -> bool {
        let id = source_id.to_ascii_lowercase();
        self.ignore
            .iter()
            .any(|i| !i.is_empty() && id.contains(&i.to_ascii_lowercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_ignore() {
        assert_eq!(
            MediaConfig::from_table(None).unwrap(),
            MediaConfig::default()
        );
        let t: toml::Table = toml::from_str("ignore = [\"MSEdge\", \"\"]").unwrap();
        let c = MediaConfig::from_table(Some(&t)).unwrap();
        assert!(c.is_ignored("msedge"));
        assert!(!c.is_ignored("Spotify.exe"));
        let typo: toml::Table = toml::from_str("ignor = []").unwrap();
        assert!(MediaConfig::from_table(Some(&typo)).is_err());
    }
}

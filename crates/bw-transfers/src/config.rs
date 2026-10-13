use std::path::PathBuf;
use std::time::Duration;

use bw_i18n::tr;
use serde::Deserialize;

/// Section `[modules.transfers]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TransfersConfig {
    pub enabled: bool,
    /// Suivre les copies, déplacements et suppressions de l'Explorateur
    /// (Windows).
    pub copies: bool,
    /// Suivre les téléchargements des navigateurs (fichiers partiels du
    /// dossier de téléchargement).
    pub downloads: bool,
    /// Dossier des téléchargements ; vide : celui du système.
    pub downloads_folder: String,
    /// Annoncer la fin d'une copie ou d'un téléchargement.
    pub announce_done: bool,
    /// Durée d'affichage d'une annonce (secondes).
    pub show_secs: u32,
}

impl Default for TransfersConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            copies: true,
            downloads: true,
            downloads_folder: String::new(),
            announce_done: true,
            show_secs: 5,
        }
    }
}

impl TransfersConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            (2..=30).contains(&config.show_secs),
            tr!(
                "modules.transfers.show_secs must be between 2 and 30",
                "modules.transfers.show_secs doit être entre 2 et 30"
            )
        );
        Ok(config)
    }

    pub fn show_for(&self) -> Duration {
        Duration::from_secs(self.show_secs.into())
    }

    /// Dossier surveillé pour les téléchargements.
    pub fn downloads_dir(&self) -> Option<PathBuf> {
        let folder = self.downloads_folder.trim();
        if folder.is_empty() {
            dirs::download_dir()
        } else {
            Some(PathBuf::from(folder))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> anyhow::Result<TransfersConfig> {
        let t: toml::Table = toml::from_str(text).unwrap();
        TransfersConfig::from_table(Some(&t))
    }

    #[test]
    fn defaults_and_validation() {
        let c = TransfersConfig::from_table(None).unwrap();
        assert!(c.enabled && c.copies && c.downloads && c.announce_done);
        assert_eq!(c.show_for(), Duration::from_secs(5));
        let custom = parse("downloads_folder = \"D:/Telechargements\"").unwrap();
        assert_eq!(
            custom.downloads_dir(),
            Some(PathBuf::from("D:/Telechargements"))
        );
        // Dossier vide : celui du système, s'il existe.
        assert_eq!(c.downloads_dir(), dirs::download_dir());
        for bad in [
            "show_secs = 1",
            "show_secs = 99",
            "copies = 3",
            "speed = true",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}

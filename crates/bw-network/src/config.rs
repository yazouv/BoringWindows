use std::time::Duration;

use bw_i18n::tr;
use serde::Deserialize;

/// Quand le débit s'affiche dans la pilule repliée.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompactSpeed {
    /// Jamais : le débit n'est visible que île ouverte.
    #[default]
    Off,
    /// Seulement quand ça circule (téléchargement, envoi…).
    Active,
    /// Tout le temps.
    Always,
}

/// Unité des débits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    /// Octets par seconde (Mo/s), comme un navigateur ou un gestionnaire de téléchargements.
    #[default]
    Bytes,
    /// Bits par seconde (Mb/s), comme un fournisseur d'accès.
    Bits,
}

/// Section `[modules.network]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NetworkConfig {
    pub enabled: bool,
    /// Débit dans la pilule repliée : `off`, `active` ou `always`.
    pub compact_speed: CompactSpeed,
    /// Unité des débits : `bytes` ou `bits`.
    pub unit: Unit,
    /// Rafraîchissement du débit (secondes).
    pub refresh_secs: u32,
    /// Débit (Ko/s) à partir duquel le mode `active` affiche.
    pub active_kb_per_sec: u32,
    /// Annoncer la perte et le retour de la connexion.
    pub announce_connection: bool,
    /// Annoncer la connexion et la déconnexion d'un VPN.
    pub announce_vpn: bool,
    /// Annoncer un changement de réseau Wi-Fi.
    pub announce_wifi: bool,
    /// Durée d'affichage d'une annonce (secondes).
    pub show_secs: u32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            compact_speed: CompactSpeed::Off,
            unit: Unit::Bytes,
            refresh_secs: 1,
            active_kb_per_sec: 50,
            announce_connection: true,
            announce_vpn: true,
            announce_wifi: false,
            show_secs: 5,
        }
    }
}

impl NetworkConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            (1..=10).contains(&config.refresh_secs),
            tr!(
                "modules.network.refresh_secs must be between 1 and 10",
                "modules.network.refresh_secs doit être entre 1 et 10"
            )
        );
        anyhow::ensure!(
            (1..=1_000_000).contains(&config.active_kb_per_sec),
            tr!(
                "modules.network.active_kb_per_sec must be between 1 and 1000000",
                "modules.network.active_kb_per_sec doit être entre 1 et 1000000"
            )
        );
        anyhow::ensure!(
            (2..=30).contains(&config.show_secs),
            tr!(
                "modules.network.show_secs must be between 2 and 30",
                "modules.network.show_secs doit être entre 2 et 30"
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

    /// Seuil du mode `active`, en octets par seconde.
    pub fn active_threshold(&self) -> u64 {
        u64::from(self.active_kb_per_sec) * 1000
    }

    /// Au moins une annonce de l'état de la connexion est demandée.
    pub fn announces(&self) -> bool {
        self.announce_connection || self.announce_vpn || self.announce_wifi
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> anyhow::Result<NetworkConfig> {
        let t: toml::Table = toml::from_str(text).unwrap();
        NetworkConfig::from_table(Some(&t))
    }

    #[test]
    fn defaults_and_validation() {
        let c = NetworkConfig::from_table(None).unwrap();
        assert!(c.enabled);
        assert_eq!(c.compact_speed, CompactSpeed::Off);
        assert_eq!(c.active_threshold(), 50_000);
        assert_eq!(
            parse("compact_speed = \"active\"\nunit = \"bits\"")
                .unwrap()
                .compact_speed,
            CompactSpeed::Active
        );
        for bad in [
            "refresh_secs = 0",
            "refresh_secs = 60",
            "active_kb_per_sec = 0",
            "show_secs = 1",
            "compact_speed = \"sometimes\"",
            "unit = \"nibbles\"",
            "speed = true",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}

use std::time::Duration;

use bw_i18n::tr;
use serde::Deserialize;

/// Section `[modules.weather]` de config.toml.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WeatherConfig {
    /// Désactivé par défaut : il faut une ville, et la météo passe par Internet.
    pub enabled: bool,
    /// Ville cherchée chez open-meteo (« Paris », « Lyon, France »…).
    pub city: String,
    /// Coordonnées exactes : passent devant `city`.
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub units: Units,
    /// Rafraîchissement (minutes).
    pub refresh_minutes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Units {
    Celsius,
    Fahrenheit,
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            city: String::new(),
            latitude: None,
            longitude: None,
            units: Units::Celsius,
            refresh_minutes: 30,
        }
    }
}

/// Où chercher la météo.
#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    City(String),
    Coords(f64, f64),
}

impl WeatherConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            (10..=180).contains(&config.refresh_minutes),
            tr!(
                "modules.weather.refresh_minutes must be between 10 and 180",
                "modules.weather.refresh_minutes doit être entre 10 et 180"
            )
        );
        anyhow::ensure!(
            config.latitude.is_some() == config.longitude.is_some(),
            tr!(
                "modules.weather: latitude and longitude go together",
                "modules.weather : latitude et longitude vont ensemble"
            )
        );
        if let (Some(lat), Some(lon)) = (config.latitude, config.longitude) {
            anyhow::ensure!(
                (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon),
                tr!(
                    "modules.weather: latitude between -90 and 90, longitude between -180 and 180",
                    "modules.weather : latitude entre -90 et 90, longitude entre -180 et 180"
                )
            );
        }
        Ok(config)
    }

    /// Lieu configuré ; `None` : ni ville ni coordonnées.
    pub fn place(&self) -> Option<Place> {
        match (self.latitude, self.longitude) {
            (Some(lat), Some(lon)) => Some(Place::Coords(lat, lon)),
            _ => {
                let city = self.city.trim();
                (!city.is_empty()).then(|| Place::City(city.to_owned()))
            }
        }
    }

    pub fn refresh(&self) -> Duration {
        Duration::from_secs(u64::from(self.refresh_minutes) * 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> anyhow::Result<WeatherConfig> {
        let t: toml::Table = toml::from_str(text).unwrap();
        WeatherConfig::from_table(Some(&t))
    }

    #[test]
    fn defaults_and_validation() {
        let c = WeatherConfig::from_table(None).unwrap();
        assert!(!c.enabled);
        assert_eq!(c.place(), None);
        assert_eq!(c.refresh(), Duration::from_secs(30 * 60));
        for bad in [
            "refresh_minutes = 5",
            "latitude = 48.8",
            "latitude = 91.0\nlongitude = 2.0",
            "units = \"kelvin\"",
            "ville = \"Paris\"",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn coordinates_win_over_city() {
        let c = parse("city = \"  Lyon \"").unwrap();
        assert_eq!(c.place(), Some(Place::City("Lyon".into())));
        let c = parse("city = \"Lyon\"\nlatitude = 48.85\nlongitude = 2.35").unwrap();
        assert_eq!(c.place(), Some(Place::Coords(48.85, 2.35)));
    }
}

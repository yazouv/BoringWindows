//! Météo du moment, affichée à côté de la date dans l'île ouverte.
//!
//! Les données viennent d'open-meteo.com : gratuit, sans clé ni compte. La
//! ville est cherchée une fois (géocodage), puis la météo est relue toutes
//! les `refresh_minutes`.

mod config;
mod module;

use serde::Deserialize;

pub use config::{Place, Units, WeatherConfig};
pub use module::{MODULE_ID, WeatherModule};

/// État du ciel, regroupé depuis les codes météo WMO.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sky {
    Clear,
    PartlyCloudy,
    Cloudy,
    Fog,
    Drizzle,
    Rain,
    Snow,
    Storm,
}

impl Sky {
    /// Codes WMO utilisés par open-meteo (`weather_code`).
    pub fn from_wmo(code: u32) -> Self {
        match code {
            0 => Self::Clear,
            1 | 2 => Self::PartlyCloudy,
            45 | 48 => Self::Fog,
            51..=57 => Self::Drizzle,
            61..=67 | 80..=82 => Self::Rain,
            71..=77 | 85 | 86 => Self::Snow,
            95..=99 => Self::Storm,
            _ => Self::Cloudy,
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Clear => bw_i18n::tr!("Clear", "Dégagé"),
            Self::PartlyCloudy => bw_i18n::tr!("Partly cloudy", "Éclaircies"),
            Self::Cloudy => bw_i18n::tr!("Cloudy", "Couvert"),
            Self::Fog => bw_i18n::tr!("Fog", "Brouillard"),
            Self::Drizzle => bw_i18n::tr!("Drizzle", "Bruine"),
            Self::Rain => bw_i18n::tr!("Rain", "Pluie"),
            Self::Snow => bw_i18n::tr!("Snow", "Neige"),
            Self::Storm => bw_i18n::tr!("Thunderstorm", "Orage"),
        }
    }

    /// Nom de l'icône dessinée par l'île.
    pub fn icon(self, day: bool) -> &'static str {
        match (self, day) {
            (Self::Clear, true) => "sun",
            (Self::Clear, false) => "moon",
            (Self::PartlyCloudy, true) => "cloud-sun",
            (Self::PartlyCloudy, false) => "cloud-moon",
            (Self::Cloudy, _) => "cloud",
            (Self::Fog, _) => "fog",
            (Self::Drizzle, _) => "drizzle",
            (Self::Rain, _) => "rain",
            (Self::Snow, _) => "snow",
            (Self::Storm, _) => "storm",
        }
    }
}

/// Ce que publie le module.
#[derive(Debug, Clone, PartialEq)]
pub struct WeatherSnapshot {
    /// Températures arrondies, dans l'unité choisie.
    pub temperature: i32,
    pub min: i32,
    pub max: i32,
    pub sky: Sky,
    pub day: bool,
    /// Nom du lieu (celui trouvé par le géocodage), vide pour des coordonnées.
    pub place: String,
}

impl WeatherSnapshot {
    /// « 14° ».
    pub fn temperature_text(&self) -> String {
        format!("{}°", self.temperature)
    }

    /// « Paris · Éclaircies · 9° / 16° ».
    pub fn detail(&self) -> String {
        let mut parts = Vec::new();
        if !self.place.is_empty() {
            parts.push(self.place.clone());
        }
        parts.push(self.sky.label());
        parts.push(format!("{}° / {}°", self.min, self.max));
        parts.join(" · ")
    }
}

/// Lieu trouvé par le géocodage.
#[derive(Debug, Clone, PartialEq)]
pub struct Located {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
}

/// URL de recherche d'une ville.
pub fn geocoding_url(city: &str, lang: bw_i18n::Lang) -> String {
    let lang = match lang {
        bw_i18n::Lang::Fr => "fr",
        bw_i18n::Lang::En => "en",
    };
    format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language={lang}&format=json",
        encode(city)
    )
}

/// Premier résultat du géocodage ; `None` si la ville est inconnue.
pub fn parse_geocoding(body: &[u8]) -> anyhow::Result<Option<Located>> {
    #[derive(Deserialize)]
    struct Response {
        #[serde(default)]
        results: Vec<Hit>,
    }
    #[derive(Deserialize)]
    struct Hit {
        name: String,
        latitude: f64,
        longitude: f64,
    }
    let response: Response = serde_json::from_slice(body)?;
    Ok(response.results.into_iter().next().map(|h| Located {
        name: h.name,
        latitude: h.latitude,
        longitude: h.longitude,
    }))
}

/// URL des prévisions : conditions actuelles et extrêmes du jour.
pub fn forecast_url(latitude: f64, longitude: f64, units: Units) -> String {
    let unit = match units {
        Units::Celsius => "celsius",
        Units::Fahrenheit => "fahrenheit",
    };
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={latitude:.4}&longitude={longitude:.4}\
         &current=temperature_2m,weather_code,is_day\
         &daily=temperature_2m_max,temperature_2m_min\
         &timezone=auto&forecast_days=1&temperature_unit={unit}"
    )
}

pub fn parse_forecast(body: &[u8], place: &str) -> anyhow::Result<WeatherSnapshot> {
    #[derive(Deserialize)]
    struct Response {
        current: Current,
        daily: Daily,
    }
    #[derive(Deserialize)]
    struct Current {
        temperature_2m: f64,
        weather_code: u32,
        is_day: u8,
    }
    #[derive(Deserialize)]
    struct Daily {
        temperature_2m_max: Vec<Option<f64>>,
        temperature_2m_min: Vec<Option<f64>>,
    }
    let r: Response = serde_json::from_slice(body)?;
    let now = r.current.temperature_2m;
    let first = |v: &[Option<f64>]| v.first().copied().flatten().unwrap_or(now);
    Ok(WeatherSnapshot {
        temperature: now.round() as i32,
        min: first(&r.daily.temperature_2m_min).round() as i32,
        max: first(&r.daily.temperature_2m_max).round() as i32,
        sky: Sky::from_wmo(r.current.weather_code),
        day: r.current.is_day != 0,
        place: place.to_owned(),
    })
}

/// Encodage d'un paramètre d'URL (RFC 3986, octets UTF-8).
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wmo_codes() {
        assert_eq!(Sky::from_wmo(0), Sky::Clear);
        assert_eq!(Sky::from_wmo(2), Sky::PartlyCloudy);
        assert_eq!(Sky::from_wmo(3), Sky::Cloudy);
        assert_eq!(Sky::from_wmo(48), Sky::Fog);
        assert_eq!(Sky::from_wmo(55), Sky::Drizzle);
        assert_eq!(Sky::from_wmo(81), Sky::Rain);
        assert_eq!(Sky::from_wmo(86), Sky::Snow);
        assert_eq!(Sky::from_wmo(99), Sky::Storm);
        assert_eq!(Sky::Clear.icon(false), "moon");
    }

    #[test]
    fn urls() {
        assert_eq!(
            geocoding_url("Saint-Étienne, FR", bw_i18n::Lang::Fr),
            "https://geocoding-api.open-meteo.com/v1/search?name=Saint-%C3%89tienne%2C%20FR&count=1&language=fr&format=json"
        );
        let url = forecast_url(48.8566, 2.3522, Units::Fahrenheit);
        assert!(url.contains("latitude=48.8566&longitude=2.3522&current="));
        assert!(url.ends_with("temperature_unit=fahrenheit"));
    }

    #[test]
    fn parses_responses() {
        let geo = br#"{"results":[{"id":1,"name":"Paris","latitude":48.85,"longitude":2.35,"country":"France"}]}"#;
        assert_eq!(
            parse_geocoding(geo).unwrap(),
            Some(Located {
                name: "Paris".into(),
                latitude: 48.85,
                longitude: 2.35
            })
        );
        assert_eq!(
            parse_geocoding(br#"{"generationtime_ms":0.1}"#).unwrap(),
            None
        );

        let body = br#"{"current":{"time":"2026-10-10T14:00","temperature_2m":13.6,"weather_code":2,"is_day":1},
            "daily":{"time":["2026-10-10"],"temperature_2m_max":[16.4],"temperature_2m_min":[8.6]}}"#;
        bw_i18n::set(bw_i18n::Lang::Fr);
        let w = parse_forecast(body, "Paris").unwrap();
        assert_eq!((w.temperature, w.min, w.max), (14, 9, 16));
        assert_eq!(w.sky, Sky::PartlyCloudy);
        assert_eq!(w.temperature_text(), "14°");
        assert_eq!(w.detail(), "Paris · Éclaircies · 9° / 16°");
        assert!(parse_forecast(b"{}", "").is_err());
    }
}

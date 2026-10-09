//! Module « weather » : cherche le lieu une fois, puis relit la météo à
//! intervalle fixe (plus vite après un échec : réseau absent au réveil…).

use std::time::Duration;

use bw_core::{Module, ModuleCtx};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::config::{Place, WeatherConfig};
use crate::{Located, WeatherSnapshot};

pub const MODULE_ID: &str = "weather";
/// Délai maximal d'une requête.
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
/// Nouvel essai après un échec.
const RETRY_AFTER: Duration = Duration::from_secs(120);
/// open-meteo compresse par défaut en « deflate », que le client HTTP de
/// Windows ne sait pas décoder (ERROR_INTERNET_DECODING_FAILED) : réponse
/// non compressée (quelques centaines d'octets).
const NO_COMPRESSION: bw_net::Headers = &[("Accept-Encoding", "identity")];

pub struct WeatherModule {
    config: WeatherConfig,
    refresh: Option<UnboundedSender<()>>,
}

impl WeatherModule {
    pub fn new(config: WeatherConfig) -> Self {
        Self {
            config,
            refresh: None,
        }
    }
}

impl Module for WeatherModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        let Some(place) = self.config.place() else {
            log::info!("météo : aucune ville configurée ([modules.weather] city)");
            return Ok(());
        };
        let (tx, mut rx) = unbounded_channel();
        self.refresh = Some(tx);
        let config = self.config.clone();
        let task_ctx = ctx.clone();

        ctx.spawn(async move {
            let mut located: Option<Located> = None;
            loop {
                let wait = match fetch(&config, &place, &mut located).await {
                    Ok(snapshot) => {
                        task_ctx.set_state(snapshot);
                        config.refresh()
                    }
                    Err(e) => {
                        log::warn!("météo : {e:#}");
                        RETRY_AFTER
                    }
                };
                tokio::select! {
                    () = tokio::time::sleep(wait) => {}
                    msg = rx.recv() => if msg.is_none() { break },
                }
            }
        });
        Ok(())
    }

    /// Action : `refresh` (relire tout de suite).
    fn on_action(&mut self, action: &str) {
        if action == "refresh"
            && let Some(tx) = &self.refresh
        {
            let _ = tx.send(());
        }
    }
}

async fn fetch(
    config: &WeatherConfig,
    place: &Place,
    located: &mut Option<Located>,
) -> anyhow::Result<WeatherSnapshot> {
    let at = match located {
        Some(at) => at.clone(),
        None => located.insert(locate(place).await?).clone(),
    };
    let body = get(crate::forecast_url(at.latitude, at.longitude, config.units)).await?;
    crate::parse_forecast(&body, &at.name)
}

/// Coordonnées du lieu : celles de la config, ou le géocodage de la ville.
async fn locate(place: &Place) -> anyhow::Result<Located> {
    match place {
        Place::Coords(latitude, longitude) => Ok(Located {
            name: String::new(),
            latitude: *latitude,
            longitude: *longitude,
        }),
        Place::City(city) => {
            let body = get(crate::geocoding_url(city, bw_i18n::lang())).await?;
            let found = crate::parse_geocoding(&body)?
                .ok_or_else(|| anyhow::anyhow!("ville « {city} » introuvable"))?;
            log::info!(
                "météo : {city} → {} ({:.2}, {:.2})",
                found.name,
                found.latitude,
                found.longitude
            );
            Ok(found)
        }
    }
}

/// Requête bloquante hors du runtime, avec délai maximal.
async fn get(url: String) -> anyhow::Result<Vec<u8>> {
    tokio::time::timeout(
        FETCH_TIMEOUT,
        tokio::task::spawn_blocking(move || bw_net::get(&url, NO_COMPRESSION)),
    )
    .await
    .map_err(|_| anyhow::anyhow!("délai dépassé"))??
}

#[cfg(test)]
mod tests {
    /// Réseau requis : `cargo test -p bw-weather -- --ignored`.
    #[test]
    #[ignore]
    fn fetches_real_weather() {
        let body = bw_net::get(
            &crate::geocoding_url("Lyon", bw_i18n::Lang::Fr),
            super::NO_COMPRESSION,
        )
        .unwrap();
        let at = crate::parse_geocoding(&body)
            .unwrap()
            .expect("Lyon introuvable");
        let units = crate::Units::Celsius;
        let body = bw_net::get(
            &crate::forecast_url(at.latitude, at.longitude, units),
            super::NO_COMPRESSION,
        )
        .unwrap();
        let w = crate::parse_forecast(&body, &at.name).unwrap();
        println!("{} {}", w.temperature_text(), w.detail());
    }
}

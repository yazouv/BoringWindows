//! Module « calendar » : télécharge les calendriers, puis ne se réveille
//! qu'aux instants où l'agenda change (rappel, début, fin…).

use std::time::Duration;

use bw_core::{Module, ModuleCtx};
use chrono::Utc;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::agenda::{CalendarSnapshot, agenda};
use crate::config::CalendarConfig;
use crate::fetch::fetch;
use crate::ics::{Event, events_between};

pub const MODULE_ID: &str = "calendar";
/// Délai maximal d'un téléchargement.
const FETCH_TIMEOUT: Duration = Duration::from_secs(45);
/// Nouvel essai plus rapide après un échec (réseau absent au réveil…).
const RETRY_AFTER: Duration = Duration::from_secs(60);

pub struct CalendarModule {
    config: CalendarConfig,
    refresh: Option<UnboundedSender<()>>,
}

impl CalendarModule {
    pub fn new(config: CalendarConfig) -> Self {
        Self {
            config,
            refresh: None,
        }
    }
}

impl Module for CalendarModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        if self.config.sources.is_empty() {
            log::info!("agenda : aucune source configurée ([[modules.calendar.sources]])");
            return Ok(());
        }
        let (tx, mut rx) = unbounded_channel();
        self.refresh = Some(tx);
        let config = self.config.clone();
        let task_ctx = ctx.clone();

        ctx.spawn(async move {
            let mut sources: Vec<Vec<Event>> = vec![Vec::new(); config.sources.len()];
            let mut error: Option<String> = None;
            let mut next_fetch = tokio::time::Instant::now();
            let mut last: Option<CalendarSnapshot> = None;

            loop {
                if tokio::time::Instant::now() >= next_fetch {
                    error = refresh_all(&config, &mut sources).await;
                    next_fetch = tokio::time::Instant::now()
                        + if error.is_some() {
                            RETRY_AFTER.min(config.refresh())
                        } else {
                            config.refresh()
                        };
                }

                let events: Vec<Event> = sources.iter().flatten().cloned().collect();
                let (mut snapshot, next_change) = agenda(&events, Utc::now(), &config);
                snapshot.error.clone_from(&error);
                if last.as_ref() != Some(&snapshot) {
                    task_ctx.set_attention(snapshot.attention, snapshot.summary.clone());
                    task_ctx.set_state(snapshot.clone());
                    last = Some(snapshot);
                }

                let wake = next_change
                    .and_then(|t| (t - Utc::now()).to_std().ok())
                    .map(|d| tokio::time::Instant::now() + d)
                    .map_or(next_fetch, |t| t.min(next_fetch));
                tokio::select! {
                    () = tokio::time::sleep_until(wake) => {}
                    msg = rx.recv() => match msg {
                        Some(()) => next_fetch = tokio::time::Instant::now(),
                        None => break,
                    },
                }
            }
        });
        Ok(())
    }

    /// Action : `refresh` (retélécharger tout de suite).
    fn on_action(&mut self, action: &str) {
        if action == "refresh"
            && let Some(tx) = &self.refresh
        {
            let _ = tx.send(());
        }
    }
}

/// Retélécharge chaque source ; en cas d'échec, garde sa version précédente.
async fn refresh_all(config: &CalendarConfig, sources: &mut [Vec<Event>]) -> Option<String> {
    let now = Utc::now();
    let from = now - chrono::Duration::days(1);
    let to = now + chrono::Duration::hours(i64::from(config.lookahead_hours) + 24);
    let mut error = None;

    for (i, source) in config.sources.iter().enumerate() {
        let url = source.url.clone();
        let label = if source.name.is_empty() {
            bw_i18n::tr!("calendar {}", "calendrier {}", i + 1)
        } else {
            source.name.clone()
        };
        let result = tokio::time::timeout(
            FETCH_TIMEOUT,
            tokio::task::spawn_blocking(move || fetch(&url)),
        )
        .await;
        match result {
            Ok(Ok(Ok(text))) => {
                let events = events_between(&text, from, to);
                log::info!("agenda : {label} — {} événement(s) à venir", events.len());
                sources[i] = events;
            }
            Ok(Ok(Err(e))) => {
                log::warn!("agenda : {label} : {e:#}");
                error = Some(bw_i18n::tr!(
                    "{label}: download failed",
                    "{label} : téléchargement impossible"
                ));
            }
            Ok(Err(_)) | Err(_) => {
                log::warn!("agenda : {label} : délai dépassé");
                error = Some(bw_i18n::tr!(
                    "{label}: timed out",
                    "{label} : délai dépassé"
                ));
            }
        }
    }
    error
}

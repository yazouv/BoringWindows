//! Module « network » : l'état de la connexion arrive par événements ; le
//! débit se mesure seulement quand il est visible (île ouverte, ou pilule
//! repliée réglée pour l'afficher).

use std::time::{Duration, Instant};

use bw_core::{Attention, Module, ModuleCtx};
use sysinfo::Networks;
use tokio::sync::{mpsc, watch};

use crate::{
    CompactSpeed, Link, NetworkConfig, NetworkSnapshot, Notice, Ticker, aggregate, compact_text,
    link_notice,
};

pub const MODULE_ID: &str = "network";

pub struct NetworkModule {
    config: NetworkConfig,
    open: watch::Sender<bool>,
    /// Réveille le thread d'écoute ; `Wake::Stop` à l'arrêt du module.
    #[cfg(windows)]
    wake: Option<std::sync::mpsc::Sender<crate::win_link::Wake>>,
}

impl NetworkModule {
    pub fn new(config: NetworkConfig) -> Self {
        Self {
            config,
            open: watch::Sender::new(false),
            #[cfg(windows)]
            wake: None,
        }
    }
}

#[cfg(windows)]
impl Drop for NetworkModule {
    fn drop(&mut self) {
        if let Some(wake) = self.wake.take() {
            let _ = wake.send(crate::win_link::Wake::Stop);
        }
    }
}

impl Module for NetworkModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        let (link_tx, link_rx) = mpsc::unbounded_channel::<Link>();
        #[cfg(windows)]
        {
            self.wake = Some(crate::win_link::spawn(link_tx));
        }
        // Hors Windows, pas d'état de connexion : le canal se ferme aussitôt.
        #[cfg(not(windows))]
        drop(link_tx);

        let state = State::new(self.config.clone(), ctx.clone());
        let open = self.open.subscribe();
        ctx.spawn(state.run(link_rx, open));
        Ok(())
    }

    /// Actions : `open` / `close` (l'île s'ouvre ou se referme).
    fn on_action(&mut self, action: &str) {
        match action {
            "open" => self.open.send_replace(true),
            "close" => self.open.send_replace(false),
            _ => return,
        };
    }
}

/// Ce que la pilule montre : l'annonce l'emporte sur le débit.
struct State {
    config: NetworkConfig,
    ctx: ModuleCtx,
    networks: Networks,
    link: Option<Link>,
    ticker: Ticker,
    speed: (u64, u64),
    /// Dernier relevé : sans relevé récent, le suivant ne sert que de base.
    sampled_at: Option<Instant>,
    notice: Option<(Notice, Instant)>,
    ticker_text: Option<String>,
    /// Dernière attention envoyée, pour ne rien répéter.
    claimed: (Attention, Option<String>),
}

impl State {
    fn new(config: NetworkConfig, ctx: ModuleCtx) -> Self {
        let ticker = Ticker::new(config.compact_speed, config.active_threshold());
        Self {
            config,
            ctx,
            networks: Networks::new_with_refreshed_list(),
            link: None,
            ticker,
            speed: (0, 0),
            sampled_at: None,
            notice: None,
            ticker_text: None,
            claimed: (Attention::None, None),
        }
    }

    async fn run(
        mut self,
        mut links: mpsc::UnboundedReceiver<Link>,
        mut open: watch::Receiver<bool>,
    ) {
        let mut links_open = true;
        loop {
            let is_open = *open.borrow_and_update();
            let sampling = is_open || self.config.compact_speed != CompactSpeed::Off;
            let period = self.config.refresh();
            let notice_end = self.notice.as_ref().map(|(_, end)| *end);
            tokio::select! {
                now = links.recv(), if links_open => match now {
                    Some(now) => self.on_link(now, is_open),
                    None => links_open = false,
                },
                changed = open.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    if *open.borrow() {
                        self.publish();
                    }
                }
                () = tokio::time::sleep(period), if sampling => self.sample(is_open),
                () = sleep_until(notice_end), if notice_end.is_some() => {
                    self.notice = None;
                    self.claim();
                }
            }
            if !sampling {
                // Plus rien à mesurer : le prochain relevé repart d'une base.
                self.sampled_at = None;
                self.ticker_text = None;
                self.claim();
            }
        }
    }

    fn on_link(&mut self, now: Link, is_open: bool) {
        if let Some(prev) = &self.link
            && let Some(notice) = link_notice(prev, &now, &self.config)
        {
            log::info!("réseau : {}", notice.text);
            self.notice = Some((notice, Instant::now() + self.config.show_for()));
        }
        self.link = Some(now);
        self.claim();
        if is_open {
            self.publish();
        }
    }

    fn sample(&mut self, is_open: bool) {
        let period = self.config.refresh();
        let vpn = self.link.as_ref().is_some_and(|l| l.vpn);
        self.networks.refresh(true);
        let first = self
            .sampled_at
            .is_none_or(|t| t.elapsed() > period * 3 + Duration::from_secs(1));
        let elapsed = self.sampled_at.map(|t| t.elapsed());
        self.sampled_at = Some(Instant::now());
        if first {
            // Octets cumulés depuis un relevé trop ancien : pas de débit fiable.
            return;
        }
        let samples: Vec<(&str, u64, u64)> = self
            .networks
            .iter()
            .map(|(name, data)| (name.as_str(), data.received(), data.transmitted()))
            .collect();
        let (down, up) = aggregate(&samples, vpn);
        // Octets par seconde, d'après la durée réellement écoulée.
        let secs = elapsed.map_or(1.0, |e| e.as_secs_f64()).max(0.1);
        self.speed = ((down as f64 / secs) as u64, (up as f64 / secs) as u64);

        self.ticker_text = self
            .ticker
            .feed(self.speed.0, self.speed.1)
            .then(|| compact_text(self.speed.0, self.speed.1, self.config.unit));
        self.claim();
        if is_open {
            self.publish();
        }
    }

    fn publish(&self) {
        self.ctx.set_state(NetworkSnapshot {
            down: self.speed.0,
            up: self.speed.1,
            link: self.link.clone(),
            unit: self.config.unit,
        });
    }

    /// Annonce, sinon débit, sinon rien.
    fn claim(&mut self) {
        let wanted = match (&self.notice, &self.ticker_text) {
            (Some((notice, _)), _) => (notice.attention, Some(notice.text.clone())),
            (None, Some(text)) => (Attention::Low, Some(text.clone())),
            (None, None) => (Attention::None, None),
        };
        if wanted != self.claimed {
            self.ctx.set_attention(wanted.0, wanted.1.clone());
            self.claimed = wanted;
        }
    }
}

async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep(at.saturating_duration_since(Instant::now())).await,
        None => std::future::pending().await,
    }
}

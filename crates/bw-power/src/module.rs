use bw_core::{Attention, Module, ModuleCtx};
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::Instant;

use crate::Notice;
use crate::config::PowerConfig;

pub const BATTERY_ID: &str = "battery";
pub const BLUETOOTH_ID: &str = "bluetooth";

#[derive(Clone, Copy)]
enum Source {
    Battery,
    Bluetooth,
}

/// Batterie du PC ou appareils Bluetooth : chaque changement notable
/// s'affiche un instant dans la pilule, avec une jauge.
pub struct PowerModule {
    source: Source,
    config: PowerConfig,
    /// Réveille le thread d'écoute ; `Wake::Stop` à l'arrêt du module.
    #[cfg(windows)]
    wake: Option<std::sync::mpsc::Sender<crate::Wake>>,
}

impl PowerModule {
    pub fn battery(config: PowerConfig) -> Self {
        Self::with_source(Source::Battery, config)
    }

    pub fn bluetooth(config: PowerConfig) -> Self {
        Self::with_source(Source::Bluetooth, config)
    }

    fn with_source(source: Source, config: PowerConfig) -> Self {
        Self {
            source,
            config,
            #[cfg(windows)]
            wake: None,
        }
    }

    pub const fn is_supported() -> bool {
        cfg!(windows)
    }
}

impl Module for PowerModule {
    fn id(&self) -> &'static str {
        match self.source {
            Source::Battery => BATTERY_ID,
            Source::Bluetooth => BLUETOOTH_ID,
        }
    }

    #[cfg(windows)]
    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        let (tx, rx) = unbounded_channel::<Notice>();
        let low = self.config.low_percent;
        match self.source {
            Source::Battery => {
                let (btx, mut brx) = unbounded_channel::<crate::Battery>();
                self.wake = Some(crate::win_battery::spawn(btx));
                ctx.clone().spawn(async move {
                    // Le premier état sert de référence : rien à annoncer.
                    let Some(mut prev) = brx.recv().await else {
                        return;
                    };
                    log::info!("batterie : {prev:?}");
                    while let Some(now) = brx.recv().await {
                        log::debug!("batterie : {now:?}");
                        if let Some(notice) = crate::battery_notice(prev, now, low)
                            && tx.send(notice).is_err()
                        {
                            break;
                        }
                        prev = now;
                    }
                });
            }
            Source::Bluetooth => {
                let (dtx, mut drx) = unbounded_channel::<Vec<crate::Device>>();
                self.wake = Some(crate::win_bluetooth::spawn(dtx));
                ctx.clone().spawn(async move {
                    let Some(mut prev) = drx.recv().await else {
                        return;
                    };
                    log::info!("bluetooth : {} appareil(s) appairé(s)", prev.len());
                    while let Some(now) = drx.recv().await {
                        log::debug!("bluetooth : {now:?}");
                        if let Some(notice) = crate::bluetooth_notice(&prev, &now, low)
                            && tx.send(notice).is_err()
                        {
                            break;
                        }
                        prev = now;
                    }
                });
            }
        }
        show(ctx, rx, self.config.show_for());
        Ok(())
    }

    #[cfg(not(windows))]
    fn start(&mut self, _ctx: ModuleCtx) -> anyhow::Result<()> {
        let _ = (&self.config, unbounded_channel::<()>, show);
        Ok(())
    }
}

/// Affiche chaque annonce pendant `show_for`, puis rend la pilule.
fn show(
    ctx: ModuleCtx,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<Notice>,
    show_for: std::time::Duration,
) {
    ctx.clone().spawn(async move {
        let mut hide_at: Option<Instant> = None;
        loop {
            let hide = async {
                match hide_at {
                    Some(at) => tokio::time::sleep_until(at).await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                notice = rx.recv() => {
                    let Some(notice) = notice else { break };
                    ctx.set_state(notice.gauge);
                    ctx.set_attention(notice.attention, Some(notice.text));
                    hide_at = Some(Instant::now() + show_for);
                }
                () = hide => {
                    ctx.set_attention(Attention::None, None);
                    hide_at = None;
                }
            }
        }
    });
}

#[cfg(windows)]
impl Drop for PowerModule {
    fn drop(&mut self) {
        // Les gestionnaires d'événements gardent eux aussi un émetteur : le
        // canal ne se ferme pas tout seul.
        if let Some(wake) = &self.wake {
            let _ = wake.send(crate::Wake::Stop);
        }
    }
}

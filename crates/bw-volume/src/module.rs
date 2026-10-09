use bw_core::{Attention, Module, ModuleCtx};
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::Instant;

use crate::config::VolumeConfig;

pub const MODULE_ID: &str = "volume";
pub const BRIGHTNESS_ID: &str = "brightness";

/// Ce qu'écoute le module : le volume (WASAPI) ou la luminosité (WMI).
#[derive(Clone, Copy)]
enum Source {
    Volume,
    Brightness,
}

/// Affiche un instant chaque changement de volume ou de luminosité.
pub struct VolumeModule {
    source: Source,
    config: VolumeConfig,
    /// Fermé quand le module s'arrête : le thread d'écoute se désinscrit.
    #[cfg(windows)]
    stop: Option<std::sync::mpsc::Sender<()>>,
}

impl VolumeModule {
    pub fn new(config: VolumeConfig) -> Self {
        Self::with_source(Source::Volume, config)
    }

    /// Luminosité de l'écran intégré (portables, tablettes).
    pub fn brightness(config: VolumeConfig) -> Self {
        Self::with_source(Source::Brightness, config)
    }

    fn with_source(source: Source, config: VolumeConfig) -> Self {
        Self {
            source,
            config,
            #[cfg(windows)]
            stop: None,
        }
    }

    pub const fn is_supported() -> bool {
        cfg!(windows)
    }
}

impl Module for VolumeModule {
    fn id(&self) -> &'static str {
        match self.source {
            Source::Volume => MODULE_ID,
            Source::Brightness => BRIGHTNESS_ID,
        }
    }

    #[cfg(windows)]
    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        let (tx, mut rx) = unbounded_channel::<String>();
        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        self.stop = Some(stop_tx);
        match self.source {
            Source::Volume => {
                let (vtx, mut vrx) = unbounded_channel::<(f32, bool)>();
                crate::wasapi::spawn(vtx, stop_rx);
                ctx.clone().spawn(async move {
                    while let Some((level, muted)) = vrx.recv().await {
                        log::debug!("volume : {level:.3} muet={muted}");
                        if tx.send(crate::label(level, muted)).is_err() {
                            break;
                        }
                    }
                });
            }
            Source::Brightness => {
                let (btx, mut brx) = unbounded_channel::<u8>();
                crate::wmi::spawn(btx, stop_rx);
                ctx.clone().spawn(async move {
                    while let Some(level) = brx.recv().await {
                        log::debug!("luminosité : {level}");
                        if tx.send(crate::brightness_label(level)).is_err() {
                            break;
                        }
                    }
                });
            }
        }

        let show_for = self.config.show_for();
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
                    text = rx.recv() => {
                        let Some(text) = text else { break };
                        ctx.set_attention(Attention::High, Some(text));
                        hide_at = Some(Instant::now() + show_for);
                    }
                    () = hide => {
                        ctx.set_attention(Attention::None, None);
                        hide_at = None;
                    }
                }
            }
        });
        Ok(())
    }

    #[cfg(not(windows))]
    fn start(&mut self, _ctx: ModuleCtx) -> anyhow::Result<()> {
        let _ = (
            &self.config,
            unbounded_channel::<()>,
            Attention::None,
            Instant::now,
        );
        Ok(())
    }
}

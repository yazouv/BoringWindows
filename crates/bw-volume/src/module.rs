use bw_core::{Attention, Module, ModuleCtx};
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::Instant;

use crate::config::VolumeConfig;

pub const MODULE_ID: &str = "volume";

pub struct VolumeModule {
    config: VolumeConfig,
    /// Fermé quand le module s'arrête : le thread audio se désinscrit.
    #[cfg(windows)]
    stop: Option<std::sync::mpsc::Sender<()>>,
}

impl VolumeModule {
    pub fn new(config: VolumeConfig) -> Self {
        Self {
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
        MODULE_ID
    }

    #[cfg(windows)]
    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        let (tx, mut rx) = unbounded_channel::<(f32, bool)>();
        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        self.stop = Some(stop_tx);
        crate::wasapi::spawn(tx, stop_rx);

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
                    event = rx.recv() => {
                        let Some((level, muted)) = event else { break };
                        log::debug!("volume : {level:.3} muet={muted}");
                        ctx.set_attention(Attention::High, Some(crate::label(level, muted)));
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
        let _ = (&self.config, unbounded_channel::<()>, Attention::None, Instant::now);
        Ok(())
    }
}

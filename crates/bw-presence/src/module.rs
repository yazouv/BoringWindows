use bw_core::{Module, ModuleCtx};

use crate::LiveSnapshot;
use crate::config::PresentationConfig;

pub const MODULE_ID: &str = "presentation";

/// Publie l'application en appel ou en partage d'écran (`LiveSnapshot`).
pub struct PresenceModule {
    config: PresentationConfig,
    /// Arrêt du thread d'écoute (événement Windows).
    #[cfg(windows)]
    stop: Option<crate::win::Stop>,
}

impl PresenceModule {
    pub fn new(config: PresentationConfig) -> Self {
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

impl Module for PresenceModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    #[cfg(windows)]
    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Option<String>>();
        self.stop = Some(crate::win::spawn(self.config.call_apps.clone(), tx)?);
        ctx.clone().spawn(async move {
            while let Some(app) = rx.recv().await {
                match &app {
                    Some(app) => log::info!("présentation : en direct ({app})"),
                    None => log::info!("présentation : terminé"),
                }
                ctx.set_state(LiveSnapshot {
                    app: app.unwrap_or_default(),
                });
            }
        });
        Ok(())
    }

    #[cfg(not(windows))]
    fn start(&mut self, _ctx: ModuleCtx) -> anyhow::Result<()> {
        let _ = (&self.config, LiveSnapshot::default());
        Ok(())
    }
}

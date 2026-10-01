use bw_core::{Module, ModuleCtx};

use crate::config::VizConfig;

pub const MODULE_ID: &str = "visualizer";

/// Niveaux (0 à 1) de chaque barre.
#[derive(Debug, Clone, PartialEq)]
pub struct VizSnapshot {
    pub bands: Vec<f32>,
}

pub struct VizModule {
    config: VizConfig,
    ctx: Option<ModuleCtx>,
    /// Arrêt de la capture en cours.
    #[cfg(windows)]
    running: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl VizModule {
    pub fn new(config: VizConfig) -> Self {
        Self {
            config,
            ctx: None,
            #[cfg(windows)]
            running: None,
        }
    }

    pub const fn is_supported() -> bool {
        cfg!(windows)
    }
}

impl Module for VizModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        self.ctx = Some(ctx);
        Ok(())
    }

    /// Actions : `start` (île ouverte et musique en cours), `stop`.
    #[cfg(windows)]
    fn on_action(&mut self, action: &str) {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        match action {
            "start" if self.running.is_none() => {
                let Some(ctx) = self.ctx.clone() else { return };
                let flag = Arc::new(AtomicBool::new(true));
                self.running = Some(flag.clone());
                crate::capture::spawn(ctx, self.config.clone(), flag);
            }
            "stop" => {
                if let Some(flag) = self.running.take() {
                    flag.store(false, Ordering::Relaxed);
                }
            }
            _ => {}
        }
    }

    #[cfg(not(windows))]
    fn on_action(&mut self, _action: &str) {
        let _ = (&self.config, &self.ctx);
    }
}

impl Drop for VizModule {
    fn drop(&mut self) {
        #[cfg(windows)]
        if let Some(flag) = self.running.take() {
            flag.store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

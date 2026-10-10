//! Module « media » : relaie l'état du lecteur vers l'UI et ses commandes
//! vers le lecteur.

use std::time::Duration;

use bw_core::{Module, ModuleCtx};
use tokio::sync::mpsc::unbounded_channel;

use crate::config::MediaConfig;
use crate::snapshot::MediaSnapshot;

pub const MODULE_ID: &str = "media";

/// Commandes envoyées au thread qui parle au système.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
pub(crate) enum Command {
    Refresh,
    TogglePlayPause,
    Next,
    Previous,
    Seek(Duration),
    /// Passer à la source suivante (si plusieurs lecteurs sont ouverts).
    CycleSource,
    Stop,
}

impl Command {
    /// Actions de l'UI : `toggle`, `next`, `prev`, `seek:<ms>`, `source`.
    fn parse(action: &str) -> Option<Self> {
        Some(match action {
            "toggle" => Self::TogglePlayPause,
            "next" => Self::Next,
            "prev" => Self::Previous,
            "source" => Self::CycleSource,
            _ => Self::Seek(Duration::from_millis(
                action.strip_prefix("seek:")?.parse().ok()?,
            )),
        })
    }
}

pub struct MediaModule {
    config: MediaConfig,
    commands: Option<std::sync::mpsc::Sender<Command>>,
}

impl MediaModule {
    pub fn new(config: MediaConfig) -> Self {
        Self {
            config,
            commands: None,
        }
    }

    /// Une source de contrôles média existe sur ce système.
    pub fn is_supported() -> bool {
        cfg!(any(windows, target_os = "macos"))
    }
}

impl Module for MediaModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        let (tx, mut rx) = unbounded_channel::<MediaSnapshot>();

        #[cfg(windows)]
        {
            self.commands = Some(crate::gsmtc::spawn(self.config.clone(), move |s| {
                let _ = tx.send(s);
            })?);
        }
        #[cfg(target_os = "macos")]
        {
            self.commands = Some(crate::macos::spawn(self.config.clone(), move |s| {
                let _ = tx.send(s);
            })?);
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = (&self.config, tx);
            log::info!("musique : pas de contrôles média sur ce système");
        }

        let task_ctx = ctx.clone();
        ctx.spawn(async move {
            let mut last: Option<MediaSnapshot> = None;
            while let Some(snapshot) = rx.recv().await {
                if last
                    .as_ref()
                    .is_some_and(|l| l.same_except_timeline(&snapshot))
                {
                    continue;
                }
                let (level, summary) = snapshot.attention();
                task_ctx.set_attention(level, summary);
                task_ctx.set_state(snapshot.clone());
                last = Some(snapshot);
            }
        });
        Ok(())
    }

    fn on_action(&mut self, action: &str) {
        match (Command::parse(action), &self.commands) {
            (Some(cmd), Some(tx)) => {
                let _ = tx.send(cmd);
            }
            (None, _) => log::warn!("musique : action inconnue {action:?}"),
            _ => {}
        }
    }
}

impl Drop for MediaModule {
    fn drop(&mut self) {
        // Les gestionnaires d'événements du système gardent une copie de
        // l'émetteur : il faut un ordre d'arrêt explicite.
        if let Some(tx) = &self.commands {
            let _ = tx.send(Command::Stop);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_actions() {
        assert_eq!(Command::parse("toggle"), Some(Command::TogglePlayPause));
        assert_eq!(Command::parse("prev"), Some(Command::Previous));
        assert_eq!(
            Command::parse("seek:61500"),
            Some(Command::Seek(Duration::from_millis(61_500)))
        );
        assert_eq!(Command::parse("seek:x"), None);
        assert_eq!(Command::parse("boom"), None);
    }
}

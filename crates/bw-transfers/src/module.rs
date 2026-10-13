//! Module « transfers » : copies de l'Explorateur (événements Windows) et
//! téléchargements des navigateurs (dossier surveillé, taille relevée chaque
//! seconde tant qu'un fichier partiel existe).

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use bw_core::{Attention, Module, ModuleCtx};
use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{Debouncer, new_debouncer};
use tokio::sync::mpsc;

use crate::{
    CopyEvent, DONE_PERCENT, DownloadTracker, Partial, TransferProgress, TransfersConfig,
    copy_done, copy_text, download_done, partial_target,
};

pub const MODULE_ID: &str = "transfers";

/// Relevé de la taille des téléchargements en cours.
const SAMPLE: Duration = Duration::from_secs(1);
/// Un relevé plus récent que ça suffit : le dossier bouge à chaque écriture.
const MIN_SCAN_GAP: Duration = Duration::from_millis(500);

pub struct TransfersModule {
    config: TransfersConfig,
    /// Surveillance du dossier des téléchargements.
    watcher: Option<Debouncer<RecommendedWatcher>>,
    #[cfg(windows)]
    copies: Option<crate::win_copy::Watcher>,
}

impl TransfersModule {
    pub fn new(config: TransfersConfig) -> Self {
        Self {
            config,
            watcher: None,
            #[cfg(windows)]
            copies: None,
        }
    }
}

impl Module for TransfersModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        let (copy_tx, copy_rx) = mpsc::unbounded_channel::<CopyEvent>();
        #[cfg(windows)]
        if self.config.copies {
            self.copies = crate::win_copy::Watcher::spawn(copy_tx);
        }
        // Hors Windows, ou copies désactivées : le canal se ferme aussitôt.
        #[cfg(not(windows))]
        drop(copy_tx);

        let (wake_tx, wake_rx) = mpsc::unbounded_channel::<()>();
        let dir = self
            .config
            .downloads
            .then(|| self.config.downloads_dir())
            .flatten()
            .filter(|d| d.is_dir());
        if let Some(dir) = &dir {
            let result = new_debouncer(
                Duration::from_millis(300),
                move |_: notify_debouncer_mini::DebounceEventResult| {
                    let _ = wake_tx.send(());
                },
            )
            .and_then(|mut debouncer| {
                debouncer
                    .watcher()
                    .watch(dir, RecursiveMode::NonRecursive)?;
                Ok(debouncer)
            });
            match result {
                Ok(debouncer) => {
                    log::info!(
                        "transferts : téléchargements surveillés dans {}",
                        dir.display()
                    );
                    self.watcher = Some(debouncer);
                }
                Err(e) => log::warn!("transferts : dossier de téléchargement non surveillé : {e}"),
            }
        } else if self.config.downloads {
            log::info!("transferts : pas de dossier de téléchargement à surveiller");
        }

        let state = State::new(self.config.clone(), ctx.clone(), dir);
        ctx.spawn(state.run(copy_rx, wake_rx));
        Ok(())
    }
}

/// Une opération de l'Explorateur en cours.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Copy {
    percent: Option<u8>,
    label: Option<String>,
}

struct State {
    config: TransfersConfig,
    ctx: ModuleCtx,
    dir: Option<std::path::PathBuf>,
    copies: HashMap<isize, Copy>,
    /// Fenêtre de copie la plus récemment mise à jour : celle qu'on affiche.
    current: Option<isize>,
    tracker: DownloadTracker,
    scanned_at: Option<Instant>,
    notice: Option<(String, Instant)>,
    /// Dernières valeurs envoyées, pour ne rien répéter.
    progress: TransferProgress,
    claimed: (Attention, Option<String>),
}

impl State {
    fn new(config: TransfersConfig, ctx: ModuleCtx, dir: Option<std::path::PathBuf>) -> Self {
        Self {
            config,
            ctx,
            dir,
            copies: HashMap::new(),
            current: None,
            tracker: DownloadTracker::new(),
            scanned_at: None,
            notice: None,
            progress: TransferProgress::IDLE,
            claimed: (Attention::None, None),
        }
    }

    async fn run(
        mut self,
        mut copy_rx: mpsc::UnboundedReceiver<CopyEvent>,
        mut wake_rx: mpsc::UnboundedReceiver<()>,
    ) {
        let (mut copy_open, mut wake_open) = (true, true);
        // Un téléchargement déjà en cours au lancement.
        self.scan();
        loop {
            let notice_end = self.notice.as_ref().map(|(_, end)| *end);
            let downloading = self.tracker.active();
            tokio::select! {
                event = copy_rx.recv(), if copy_open => match event {
                    Some(event) => self.on_copy(event),
                    None => copy_open = false,
                },
                woken = wake_rx.recv(), if wake_open => match woken {
                    Some(()) => {
                        if self.scanned_at.is_none_or(|t| t.elapsed() >= MIN_SCAN_GAP) {
                            self.scan();
                        }
                    }
                    None => wake_open = false,
                },
                () = tokio::time::sleep(SAMPLE), if downloading => self.scan(),
                () = sleep_until(notice_end), if notice_end.is_some() => {
                    self.notice = None;
                    self.publish();
                }
            }
            // Sans plus rien à suivre, on s'arrête là jusqu'au prochain événement.
            if !copy_open && !wake_open && !downloading && self.notice.is_none() {
                break;
            }
        }
    }

    fn on_copy(&mut self, event: CopyEvent) {
        match event {
            CopyEvent::Update {
                window,
                percent,
                label,
            } => {
                self.copies.insert(window, Copy { percent, label });
                self.current = Some(window);
            }
            CopyEvent::Gone { window } => {
                let finished = self
                    .copies
                    .remove(&window)
                    .is_some_and(|c| c.percent.is_some_and(|p| p >= DONE_PERCENT));
                if self.current == Some(window) {
                    self.current = self.copies.keys().next().copied();
                }
                if finished {
                    self.announce(copy_done());
                }
            }
        }
        self.publish();
    }

    /// Relève les fichiers partiels du dossier des téléchargements.
    fn scan(&mut self) {
        let Some(dir) = self.dir.clone() else { return };
        let elapsed = self.scanned_at.map_or(1.0, |t| t.elapsed().as_secs_f64());
        self.scanned_at = Some(Instant::now());
        let partials = list_partials(&dir);
        let done = self
            .tracker
            .update(&partials, elapsed, |path: &Path| path.is_file());
        if let Some(first) = done.first() {
            let text = if done.len() == 1 {
                download_done(first)
            } else {
                let count = done.len();
                bw_i18n::tr!(
                    "{count} downloads finished",
                    "{count} téléchargements terminés"
                )
            };
            self.announce(text);
        }
        self.publish();
    }

    fn announce(&mut self, text: String) {
        if self.config.announce_done {
            log::info!("transferts : {text}");
            self.notice = Some((text, Instant::now() + self.config.show_for()));
        }
    }

    /// Ce qu'on montre : la copie d'abord, sinon les téléchargements.
    fn publish(&mut self) {
        let copy = self.current.and_then(|w| self.copies.get(&w));
        let (progress, text) = match (copy, self.tracker.summary()) {
            (Some(c), _) => (
                TransferProgress {
                    active: true,
                    fraction: c.percent.map(|p| f32::from(p) / 100.0),
                },
                Some(copy_text(c.percent, c.label.as_deref())),
            ),
            (None, Some(summary)) => (
                TransferProgress {
                    active: true,
                    fraction: None,
                },
                Some(summary),
            ),
            (None, None) => (TransferProgress::IDLE, None),
        };
        if progress != self.progress {
            self.progress = progress;
            self.ctx.set_state(progress);
        }
        let wanted = match (&self.notice, text) {
            (Some((notice, _)), _) => (Attention::High, Some(notice.clone())),
            (None, Some(text)) => (Attention::Low, Some(text)),
            (None, None) => (Attention::None, None),
        };
        if wanted != self.claimed {
            self.ctx.set_attention(wanted.0, wanted.1.clone());
            self.claimed = wanted;
        }
    }
}

/// Les fichiers partiels des navigateurs (`.crdownload`, `.part`…) du dossier.
fn list_partials(dir: &Path) -> Vec<Partial> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            partial_target(name.to_str()?)?;
            // Pas `entry.metadata()` : sous Windows, la taille issue de la liste du
            // dossier reste périmée tant que le navigateur garde le fichier ouvert
            // en écriture. Une lecture par le fichier lui-même est à jour.
            let path = entry.path();
            let meta = std::fs::metadata(&path)
                .ok()
                .filter(std::fs::Metadata::is_file)?;
            Some(Partial {
                path,
                size: meta.len(),
            })
        })
        .collect()
}

async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep(at.saturating_duration_since(Instant::now())).await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn partial_size_is_current_while_the_browser_holds_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut partial = std::fs::File::create(dir.path().join("video.mp4.crdownload")).unwrap();
        partial.write_all(&[0u8; 5000]).unwrap();
        partial.flush().unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"autre fichier").unwrap();

        // Le fichier est encore ouvert en écriture, comme chez un navigateur.
        let found = list_partials(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].size, 5000);

        partial.write_all(&[0u8; 3000]).unwrap();
        partial.flush().unwrap();
        assert_eq!(list_partials(dir.path())[0].size, 8000);
    }
}

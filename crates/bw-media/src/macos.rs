//! macOS : Spotify et Musique, lus et pilotés en AppleScript.
//!
//! Les deux lecteurs envoient une notification distribuée à chaque
//! changement (morceau, lecture, pause) : un fil les écoute sur sa propre
//! boucle d'événements et demande alors un rafraîchissement au fil de
//! travail. Pas de polling ; seule la lecture d'un état passe par osascript.

use std::process::Command as Process;
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2_app_kit::NSRunningApplication;
use objc2_core_foundation::{CFRunLoop, CFRunLoopRunResult, kCFRunLoopDefaultMode};
use objc2_foundation::{NSDistributedNotificationCenter, NSNotification, NSString};

use crate::applescript::{Control, Player, Track, parse_raw_data, parse_track};
use crate::artwork::{Artwork, MAX_ENCODED};
use crate::config::MediaConfig;
use crate::module::Command;
use crate::snapshot::{MediaSnapshot, NowPlaying, display_name};

/// Tentatives de lecture d'une pochette (elle arrive parfois après le titre).
const ARTWORK_ATTEMPTS: u8 = 3;

/// Boucle d'événements du fil des notifications, pour l'arrêter depuis le
/// fil de travail.
struct RunLoopHandle(objc2_core_foundation::CFRetained<CFRunLoop>);
// SAFETY: CFRunLoopStop et CFRunLoopWakeUp peuvent être appelés depuis
// n'importe quel fil (documentation de Core Foundation).
unsafe impl Send for RunLoopHandle {}

pub(crate) fn spawn<F>(config: MediaConfig, out: F) -> anyhow::Result<Sender<Command>>
where
    F: Fn(MediaSnapshot) + Send + 'static,
{
    let (tx, rx) = channel();
    let stopped = Arc::new(AtomicBool::new(false));
    let (loop_tx, loop_rx) = channel::<RunLoopHandle>();

    let notify_tx = tx.clone();
    let notify_stopped = stopped.clone();
    let listener = std::thread::Builder::new()
        .name("bw-media-notify".into())
        .spawn(move || listen(&notify_tx, &notify_stopped, &loop_tx))?;

    std::thread::Builder::new()
        .name("bw-media".into())
        .spawn(move || {
            let mut worker = Worker {
                config,
                out,
                pinned: None,
                artwork: None,
            };
            worker.update();
            while let Ok(first) = rx.recv() {
                let batch: Vec<Command> = std::iter::once(first).chain(rx.try_iter()).collect();
                if batch.contains(&Command::Stop) {
                    break;
                }
                for command in batch {
                    worker.execute(command);
                }
                worker.update();
            }
            stopped.store(true, Ordering::SeqCst);
            if let Ok(run_loop) = loop_rx.recv() {
                run_loop.0.stop();
            }
            listener.thread().unpark();
        })?;
    Ok(tx)
}

/// Écoute les notifications des lecteurs jusqu'à l'arrêt.
fn listen(tx: &Sender<Command>, stopped: &AtomicBool, loop_tx: &Sender<RunLoopHandle>) {
    let center = NSDistributedNotificationCenter::defaultCenter();
    let observers: Vec<_> = Player::ALL
        .iter()
        .map(|player| {
            let tx = tx.clone();
            let block = RcBlock::new(move |_: NonNull<NSNotification>| {
                let _ = tx.send(Command::Refresh);
            });
            let name = NSString::from_str(player.notification());
            // SAFETY: le bloc ne capture qu'un émetteur de canal ; il reste
            // valable tant que l'observateur existe (copié par le centre).
            unsafe {
                center.addObserverForName_object_queue_usingBlock(Some(&name), None, None, &block)
            }
        })
        .collect();

    if let Some(run_loop) = CFRunLoop::current() {
        let _ = loop_tx.send(RunLoopHandle(run_loop));
    }
    // Les notifications arrivent par la boucle d'événements de ce fil. Si
    // elle n'a aucune source (livraison ailleurs), elle rend la main tout de
    // suite : on attend alors simplement l'arrêt. Le délai borne l'attente
    // si l'ordre d'arrêt arrive juste avant d'entrer dans la boucle.
    // SAFETY: constante de Core Foundation, toujours définie.
    let mode = unsafe { kCFRunLoopDefaultMode };
    while !stopped.load(Ordering::SeqCst) {
        if CFRunLoop::run_in_mode(mode, 60.0, false) == CFRunLoopRunResult::Finished {
            std::thread::park_timeout(Duration::from_secs(60));
        }
    }
    for observer in observers {
        // SAFETY: observateur obtenu de ce même centre.
        unsafe { center.removeObserver(observer.as_ref()) };
    }
}

struct ArtworkCache {
    key: (Player, String),
    artwork: Option<Artwork>,
    attempts: u8,
}

struct Worker<F> {
    config: MediaConfig,
    out: F,
    /// Lecteur choisi à la main (« changer de source »).
    pinned: Option<Player>,
    artwork: Option<ArtworkCache>,
}

impl<F: Fn(MediaSnapshot)> Worker<F> {
    /// Lecteurs ouverts, non ignorés, avec un morceau chargé.
    fn tracks(&self) -> Vec<(Player, Track)> {
        Player::ALL
            .into_iter()
            .filter(|p| !self.config.is_ignored(p.bundle_id()) && is_running(*p))
            .filter_map(|p| Some((p, parse_track(&osascript(&p.query_script())?)?)))
            .collect()
    }

    /// Source manuelle si elle joue encore un morceau, sinon celle qui joue,
    /// sinon la première.
    fn choose(&mut self, tracks: &[(Player, Track)]) -> Option<usize> {
        if let Some(pin) = self.pinned {
            match tracks.iter().position(|(p, _)| *p == pin) {
                Some(i) => return Some(i),
                None => self.pinned = None,
            }
        }
        tracks
            .iter()
            .position(|(_, t)| t.playing)
            .or((!tracks.is_empty()).then_some(0))
    }

    fn execute(&mut self, command: Command) {
        let control = match command {
            Command::TogglePlayPause => Control::PlayPause,
            Command::Next => Control::Next,
            Command::Previous => Control::Previous,
            Command::Seek(at) => Control::Seek(at),
            Command::CycleSource => {
                let tracks = self.tracks();
                if let Some(i) = self.choose(&tracks) {
                    self.pinned = tracks.get((i + 1) % tracks.len()).map(|(p, _)| *p);
                }
                return;
            }
            Command::Refresh | Command::Stop => return,
        };
        let tracks = self.tracks();
        if let Some(i) = self.choose(&tracks) {
            let _ = osascript(&tracks[i].0.command_script(control));
        }
    }

    fn update(&mut self) {
        let tracks = self.tracks();
        let chosen = self.choose(&tracks);
        let now_playing = chosen.map(|i| {
            let (player, track) = &tracks[i];
            let artwork = self.artwork_for(*player, track);
            let id = player.bundle_id();
            NowPlaying {
                source_id: id.to_owned(),
                source: display_name(id),
                title: track.title.clone(),
                artist: track.artist.clone(),
                album: track.album.clone(),
                playing: track.playing,
                position: track.position,
                position_at: Instant::now(),
                duration: track.duration,
                can_previous: true,
                can_next: true,
                can_play_pause: true,
                can_seek: true,
                artwork,
            }
        });
        (self.out)(MediaSnapshot {
            now_playing,
            source_count: tracks.len(),
        });
    }

    fn artwork_for(&mut self, player: Player, track: &Track) -> Option<Artwork> {
        let key = (player, format!("{}\u{1e}{}", track.id, track.title));
        let cache = match &mut self.artwork {
            Some(c) if c.key == key => c,
            _ => self.artwork.insert(ArtworkCache {
                key,
                artwork: None,
                attempts: 0,
            }),
        };
        if cache.artwork.is_none() && cache.attempts < ARTWORK_ATTEMPTS {
            cache.attempts += 1;
            cache.artwork = artwork_bytes(player, track).and_then(|b| Artwork::decode(&b));
        }
        cache.artwork.clone()
    }
}

fn artwork_bytes(player: Player, track: &Track) -> Option<Vec<u8>> {
    if let Some(script) = player.artwork_script() {
        return parse_raw_data(&osascript(&script)?);
    }
    // Pochettes de Spotify : uniquement depuis son serveur d'images.
    let url = &track.artwork_url;
    if !url.starts_with("https://i.scdn.co/") {
        return None;
    }
    match bw_net::get(url, &[]) {
        Ok(bytes) if bytes.len() <= MAX_ENCODED => Some(bytes),
        Ok(_) => None,
        Err(e) => {
            log::debug!("musique : pochette Spotify : {e:#}");
            None
        }
    }
}

/// Le lecteur est ouvert ? (Sans cela, un script le lancerait.)
fn is_running(player: Player) -> bool {
    let id = NSString::from_str(player.bundle_id());
    NSRunningApplication::runningApplicationsWithBundleIdentifier(&id).count() > 0
}

/// Lance un AppleScript et renvoie sa sortie.
fn osascript(script: &str) -> Option<String> {
    let output = Process::new("/usr/bin/osascript")
        .args(["-e", script])
        .output()
        .map_err(|e| log::warn!("musique : osascript : {e}"))
        .ok()?;
    if !output.status.success() {
        log::debug!(
            "musique : osascript : {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

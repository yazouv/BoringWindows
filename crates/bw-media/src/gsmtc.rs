//! Lecture et pilotage des contrôles média de Windows (GSMTC).
//!
//! Tous les appels WinRT se font sur un thread dédié. Les événements du
//! système (changement de morceau, lecture/pause, nouvelle source…) ne font
//! que lui demander un rafraîchissement : pas de polling, pas de réentrance.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    MediaPropertiesChangedEventArgs, PlaybackInfoChangedEventArgs, SessionsChangedEventArgs,
    TimelinePropertiesChangedEventArgs,
};
use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};
use windows::core::RuntimeType;

use crate::artwork::{Artwork, MAX_ENCODED};
use crate::config::MediaConfig;
use crate::module::Command;
use crate::snapshot::{MediaSnapshot, NowPlaying, display_name};

/// Unités WinRT : 100 ns.
const TICKS_PER_SEC: i64 = 10_000_000;
/// Écart entre l'époque Windows (1601) et Unix (1970), en secondes.
const EPOCH_GAP_SECS: i64 = 11_644_473_600;
/// Tentatives de lecture d'une pochette arrivée en retard.
const ARTWORK_ATTEMPTS: u8 = 3;

pub(crate) fn spawn<F>(config: MediaConfig, out: F) -> anyhow::Result<Sender<Command>>
where
    F: Fn(MediaSnapshot) + Send + 'static,
{
    let (tx, rx) = channel();
    let worker_tx = tx.clone();
    std::thread::Builder::new()
        .name("bw-media".into())
        .spawn(move || {
            if let Err(e) = run(config, &rx, worker_tx, out) {
                log::error!("musique : contrôles média indisponibles : {e}");
            }
        })?;
    Ok(tx)
}

fn on_change<S: RuntimeType + 'static, A: RuntimeType + 'static>(
    tx: &Sender<Command>,
) -> TypedEventHandler<S, A> {
    let tx = tx.clone();
    TypedEventHandler::new(move |_, _| {
        let _ = tx.send(Command::Refresh);
        Ok(())
    })
}

struct Subscription {
    id: String,
    session: Session,
    tokens: [i64; 3],
}

struct ArtworkCache {
    key: (String, String, String, String),
    artwork: Option<Artwork>,
    attempts: u8,
}

struct Worker<F> {
    manager: Manager,
    config: MediaConfig,
    tx: Sender<Command>,
    subscriptions: Vec<Subscription>,
    /// Source choisie à la main (« changer de source »).
    pinned: Option<String>,
    artwork: Option<ArtworkCache>,
    out: F,
}

fn run<F: Fn(MediaSnapshot)>(
    config: MediaConfig,
    rx: &Receiver<Command>,
    tx: Sender<Command>,
    out: F,
) -> windows::core::Result<()> {
    let manager = Manager::RequestAsync()?.join()?;
    let t_sessions =
        manager.SessionsChanged(&on_change::<Manager, SessionsChangedEventArgs>(&tx))?;
    let t_current = manager
        .CurrentSessionChanged(&on_change::<Manager, CurrentSessionChangedEventArgs>(&tx))?;

    let mut worker = Worker {
        manager,
        config,
        tx,
        subscriptions: Vec::new(),
        pinned: None,
        artwork: None,
        out,
    };
    worker.update();

    while let Ok(first) = rx.recv() {
        // Regrouper les rafales d'événements en un seul rafraîchissement.
        let batch: Vec<Command> = std::iter::once(first).chain(rx.try_iter()).collect();
        if batch.contains(&Command::Stop) {
            break;
        }
        for command in batch {
            worker.execute(command);
        }
        worker.update();
    }

    worker.unsubscribe_all();
    let _ = worker.manager.RemoveSessionsChanged(t_sessions);
    let _ = worker.manager.RemoveCurrentSessionChanged(t_current);
    Ok(())
}

impl<F: Fn(MediaSnapshot)> Worker<F> {
    fn sessions(&self) -> Vec<Session> {
        let Ok(list) = self.manager.GetSessions() else {
            return Vec::new();
        };
        list.into_iter()
            .filter(|s| !self.config.is_ignored(&source_id(s)))
            .collect()
    }

    /// Source manuelle si elle existe encore, sinon celle qui joue, sinon
    /// celle que Windows considère comme courante.
    fn choose(&mut self, sessions: &[Session]) -> Option<Session> {
        if let Some(pin) = &self.pinned {
            match sessions.iter().find(|s| &source_id(s) == pin) {
                Some(s) => return Some(s.clone()),
                None => self.pinned = None,
            }
        }
        if let Some(s) = sessions.iter().find(|s| is_playing(s)) {
            return Some(s.clone());
        }
        if let Ok(current) = self.manager.GetCurrentSession()
            && !self.config.is_ignored(&source_id(&current))
        {
            return Some(current);
        }
        sessions.first().cloned()
    }

    fn execute(&mut self, command: Command) {
        let sessions = self.sessions();
        let Some(session) = self.choose(&sessions) else {
            return;
        };
        // Les opérations sont lancées sans attendre : le résultat revient par
        // les événements du lecteur.
        let _ = match command {
            Command::TogglePlayPause => session.TryTogglePlayPauseAsync().map(drop),
            Command::Next => session.TrySkipNextAsync().map(drop),
            Command::Previous => session.TrySkipPreviousAsync().map(drop),
            Command::Seek(position) => {
                let start = session
                    .GetTimelineProperties()
                    .and_then(|t| t.StartTime())
                    .map_or(0, |t| t.Duration);
                let ticks = (position.as_nanos() / 100) as i64;
                session
                    .TryChangePlaybackPositionAsync(start + ticks)
                    .map(drop)
            }
            Command::CycleSource => {
                let current = source_id(&session);
                let pos = sessions.iter().position(|s| source_id(s) == current);
                let next = pos.map_or(0, |p| (p + 1) % sessions.len().max(1));
                self.pinned = sessions.get(next).map(source_id);
                Ok(())
            }
            Command::Refresh | Command::Stop => Ok(()),
        };
    }

    fn update(&mut self) {
        let sessions = self.sessions();
        self.resubscribe(&sessions);
        let now_playing = self
            .choose(&sessions)
            .and_then(|s| self.now_playing(&s).ok());
        (self.out)(MediaSnapshot {
            now_playing,
            source_count: sessions.len(),
        });
    }

    fn now_playing(&mut self, s: &Session) -> windows::core::Result<NowPlaying> {
        let id = source_id(s);
        let props = s.TryGetMediaPropertiesAsync()?.join()?;
        let title = props.Title()?.to_string();
        let artist = props.Artist()?.to_string();
        let album = props.AlbumTitle()?.to_string();

        let info = s.GetPlaybackInfo()?;
        let playing = info.PlaybackStatus()? == Status::Playing;
        let controls = info.Controls()?;

        let timeline = s.GetTimelineProperties()?;
        let start = timeline.StartTime()?.Duration;
        let end = timeline.EndTime()?.Duration;
        let position = (timeline.Position()?.Duration - start).max(0);
        let updated = timeline.LastUpdatedTime()?.UniversalTime;

        // Le lecteur ne met pas toujours la position à jour : on la date
        // pour la faire avancer côté UI.
        let now = Instant::now();
        let since_update = elapsed_since_filetime(updated)
            .filter(|_| playing)
            .unwrap_or(Duration::ZERO);
        let position_at = now.checked_sub(since_update).unwrap_or(now);

        let key = (id.clone(), title.clone(), artist.clone(), album.clone());
        let artwork = self.artwork_for(key, props.Thumbnail().ok());

        Ok(NowPlaying {
            source: display_name(&id),
            source_id: id,
            title,
            artist,
            album,
            playing,
            position: ticks(position),
            position_at,
            duration: (end > start).then(|| ticks(end - start)),
            can_previous: controls.IsPreviousEnabled().unwrap_or(false),
            can_next: controls.IsNextEnabled().unwrap_or(false),
            can_play_pause: controls.IsPlayPauseToggleEnabled().unwrap_or(false)
                || controls.IsPlayEnabled().unwrap_or(false)
                || controls.IsPauseEnabled().unwrap_or(false),
            can_seek: controls.IsPlaybackPositionEnabled().unwrap_or(false),
            artwork,
        })
    }

    /// Pochette du morceau, lue une fois par morceau (avec quelques
    /// tentatives : elle arrive souvent juste après le titre).
    fn artwork_for(
        &mut self,
        key: (String, String, String, String),
        thumbnail: Option<IRandomAccessStreamReference>,
    ) -> Option<Artwork> {
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
            cache.artwork = thumbnail
                .and_then(|t| read_stream(&t))
                .and_then(|bytes| Artwork::decode(&bytes));
        }
        cache.artwork.clone()
    }

    fn resubscribe(&mut self, sessions: &[Session]) {
        let ids: Vec<String> = sessions.iter().map(source_id).collect();
        if self.subscriptions.iter().map(|s| &s.id).eq(ids.iter()) {
            return;
        }
        self.unsubscribe_all();
        for (session, id) in sessions.iter().zip(ids) {
            let tx = &self.tx;
            let tokens = (|| -> windows::core::Result<[i64; 3]> {
                Ok([
                    session.MediaPropertiesChanged(&on_change::<
                        Session,
                        MediaPropertiesChangedEventArgs,
                    >(tx))?,
                    session
                        .PlaybackInfoChanged(
                            &on_change::<Session, PlaybackInfoChangedEventArgs>(tx),
                        )?,
                    session.TimelinePropertiesChanged(&on_change::<
                        Session,
                        TimelinePropertiesChangedEventArgs,
                    >(tx))?,
                ])
            })();
            match tokens {
                Ok(tokens) => self.subscriptions.push(Subscription {
                    id,
                    session: session.clone(),
                    tokens,
                }),
                Err(e) => log::warn!("musique : abonnement à {id} impossible : {e}"),
            }
        }
    }

    fn unsubscribe_all(&mut self) {
        for sub in self.subscriptions.drain(..) {
            let [media, playback, timeline] = sub.tokens;
            let _ = sub.session.RemoveMediaPropertiesChanged(media);
            let _ = sub.session.RemovePlaybackInfoChanged(playback);
            let _ = sub.session.RemoveTimelinePropertiesChanged(timeline);
        }
    }
}

fn source_id(s: &Session) -> String {
    s.SourceAppUserModelId()
        .map(|h| h.to_string())
        .unwrap_or_default()
}

fn is_playing(s: &Session) -> bool {
    s.GetPlaybackInfo()
        .and_then(|i| i.PlaybackStatus())
        .is_ok_and(|st| st == Status::Playing)
}

fn ticks(t: i64) -> Duration {
    Duration::from_nanos(u64::try_from(t).unwrap_or(0).saturating_mul(100))
}

/// Temps écoulé depuis une date WinRT (ticks depuis 1601), si elle est plausible.
fn elapsed_since_filetime(filetime: i64) -> Option<Duration> {
    if filetime <= 0 {
        return None;
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    let now_ticks = (now.as_secs() as i64 + EPOCH_GAP_SECS) * TICKS_PER_SEC
        + i64::from(now.subsec_nanos()) / 100;
    let elapsed = ticks(now_ticks - filetime);
    (elapsed < Duration::from_secs(24 * 3600)).then_some(elapsed)
}

fn read_stream(reference: &IRandomAccessStreamReference) -> Option<Vec<u8>> {
    let stream = reference.OpenReadAsync().ok()?.join().ok()?;
    let size = usize::try_from(stream.Size().ok()?).ok()?;
    if size == 0 || size > MAX_ENCODED {
        return None;
    }
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    reader.LoadAsync(size as u32).ok()?.join().ok()?;
    let mut bytes = vec![0u8; size];
    reader.ReadBytes(&mut bytes).ok()?;
    Some(bytes)
}

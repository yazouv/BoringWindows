use std::time::{Duration, Instant};

use bw_core::Attention;

use crate::artwork::Artwork;

/// Morceau affiché.
#[derive(Debug, Clone, PartialEq)]
pub struct NowPlaying {
    /// Identifiant de l'application source (AUMID sous Windows).
    pub source_id: String,
    /// Nom lisible : « Spotify », « Apple Music », « Chrome »…
    pub source: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    /// Position à l'instant `position_at`.
    pub position: Duration,
    pub position_at: Instant,
    pub duration: Option<Duration>,
    pub can_previous: bool,
    pub can_next: bool,
    pub can_play_pause: bool,
    pub can_seek: bool,
    pub artwork: Option<Artwork>,
}

impl NowPlaying {
    /// Position estimée maintenant (elle avance seule pendant la lecture).
    pub fn position_now(&self, now: Instant) -> Duration {
        let pos = if self.playing {
            self.position + now.saturating_duration_since(self.position_at)
        } else {
            self.position
        };
        self.duration.map_or(pos, |d| pos.min(d))
    }

    /// Clé du morceau : change quand on passe au suivant.
    pub fn track_key(&self) -> (&str, &str, &str, &str) {
        (&self.source_id, &self.title, &self.artist, &self.album)
    }
}

/// État publié vers l'UI.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MediaSnapshot {
    pub now_playing: Option<NowPlaying>,
    /// Nombre de sources disponibles (pour proposer d'en changer).
    pub source_count: usize,
}

impl MediaSnapshot {
    /// Musique en lecture : attention basse, la pilule montre le morceau.
    pub fn attention(&self) -> (Attention, Option<String>) {
        match &self.now_playing {
            Some(np) if np.playing && !np.title.is_empty() => {
                let summary = if np.artist.is_empty() {
                    format!("♪ {}", np.title)
                } else {
                    format!("♪ {} — {}", np.title, np.artist)
                };
                (Attention::Low, Some(summary))
            }
            _ => (Attention::None, None),
        }
    }

    /// Égalité en ignorant la progression normale de la lecture : évite de
    /// réveiller l'UI à chaque mise à jour de position envoyée par le lecteur.
    pub fn same_except_timeline(&self, other: &Self) -> bool {
        match (&self.now_playing, &other.now_playing) {
            (Some(a), Some(b)) => {
                let now = Instant::now();
                let drift = a.position_now(now).abs_diff(b.position_now(now));
                let a2 = NowPlaying {
                    position: Duration::ZERO,
                    position_at: now,
                    ..a.clone()
                };
                let b2 = NowPlaying {
                    position: Duration::ZERO,
                    position_at: now,
                    ..b.clone()
                };
                a2 == b2
                    && drift < Duration::from_millis(1500)
                    && self.source_count == other.source_count
            }
            (a, b) => a.is_none() && b.is_none() && self.source_count == other.source_count,
        }
    }
}

const KNOWN: [(&str, &str); 14] = [
    ("spotify", "Spotify"),
    ("applemusic", "Apple Music"),
    ("itunes", "iTunes"),
    ("deezer", "Deezer"),
    ("tidal", "Tidal"),
    ("chrome", "Chrome"),
    ("msedge", "Edge"),
    ("firefox", "Firefox"),
    // AUMID historique de Firefox.
    ("308046b0af4a39cb", "Firefox"),
    ("opera", "Opera"),
    ("brave", "Brave"),
    ("vlc", "VLC"),
    ("foobar2000", "foobar2000"),
    ("musicbee", "MusicBee"),
];

/// Morceau de nom à mettre dans `ignore` pour écarter cette source.
pub fn ignore_token(source_id: &str) -> String {
    let lower = source_id.to_ascii_lowercase();
    if lower.contains("zunemusic") || lower.contains("microsoft.media.player") {
        return "zunemusic".into();
    }
    if let Some((key, _)) = KNOWN.iter().find(|(k, _)| lower.contains(k)) {
        return (*key).to_owned();
    }
    display_name(source_id).to_ascii_lowercase()
}

/// Nom lisible d'une application à partir de son identifiant.
pub fn display_name(source_id: &str) -> String {
    let lower = source_id.to_ascii_lowercase();
    // Lecteur de Windows : seul nom à traduire.
    if lower.contains("zunemusic") || lower.contains("microsoft.media.player") {
        return bw_i18n::tr!("Media Player", "Lecteur multimédia");
    }
    if let Some((_, name)) = KNOWN.iter().find(|(k, _)| lower.contains(k)) {
        return (*name).to_owned();
    }
    // « Editeur.App_hash!App » ou « app.exe » → « App ».
    let base = source_id.split('!').next().unwrap_or(source_id);
    let base = base.split('_').next().unwrap_or(base);
    let base = base.strip_suffix(".exe").unwrap_or(base);
    let base = base.rsplit(['.', '\\', '/']).next().unwrap_or(base);
    if base.is_empty() {
        "Média".into()
    } else {
        base.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignore_tokens_match_their_source() {
        let cfg = crate::MediaConfig {
            ignore: vec![ignore_token("MSEdge"), ignore_token("Foo.Bar_abc!App")],
            ..Default::default()
        };
        assert_eq!(ignore_token("Spotify.exe"), "spotify");
        assert!(cfg.is_ignored("MSEdge"));
        assert!(cfg.is_ignored("Foo.Bar_abc!App"));
        assert!(!cfg.is_ignored("Spotify.exe"));
    }

    fn np(playing: bool) -> NowPlaying {
        NowPlaying {
            source_id: "Spotify.exe".into(),
            source: "Spotify".into(),
            title: "Digital Love".into(),
            artist: "Daft Punk".into(),
            album: "Discovery".into(),
            playing,
            position: Duration::from_secs(10),
            position_at: Instant::now(),
            duration: Some(Duration::from_secs(301)),
            can_previous: true,
            can_next: true,
            can_play_pause: true,
            can_seek: true,
            artwork: None,
        }
    }

    #[test]
    fn names() {
        assert_eq!(display_name("Spotify.exe"), "Spotify");
        assert_eq!(
            display_name("AppleInc.AppleMusicWin_nzyj5cx40ttqa!App"),
            "Apple Music"
        );
        assert_eq!(display_name("MSEdge"), "Edge");
        assert_eq!(display_name("308046B0AF4A39CB"), "Firefox");
        assert_eq!(display_name("Contoso.Player_abc123!App"), "Player");
        assert_eq!(display_name("C:\\Apps\\tunes.exe"), "tunes");
        assert_eq!(display_name(""), "Média");
    }

    #[test]
    fn position_advances_only_while_playing_and_is_capped() {
        let mut p = np(true);
        let t = p.position_at + Duration::from_secs(5);
        assert_eq!(p.position_now(t), Duration::from_secs(15));
        assert_eq!(
            p.position_now(t + Duration::from_secs(1000)),
            Duration::from_secs(301)
        );
        p.playing = false;
        assert_eq!(p.position_now(t), Duration::from_secs(10));
    }

    #[test]
    fn attention_only_while_playing() {
        let mut s = MediaSnapshot {
            now_playing: Some(np(true)),
            source_count: 1,
        };
        assert_eq!(
            s.attention(),
            (Attention::Low, Some("♪ Digital Love — Daft Punk".into()))
        );
        s.now_playing = Some(np(false));
        assert_eq!(s.attention(), (Attention::None, None));
        assert_eq!(
            MediaSnapshot::default().attention(),
            (Attention::None, None)
        );
    }

    #[test]
    fn timeline_noise_is_ignored_but_seeks_are_not() {
        let a = MediaSnapshot {
            now_playing: Some(np(true)),
            source_count: 1,
        };
        let mut b = a.clone();
        // Même position projetée, mesurée à un autre instant.
        if let Some(p) = b.now_playing.as_mut() {
            p.position_at -= Duration::from_secs(3);
            p.position -= Duration::from_secs(3);
        }
        assert!(a.same_except_timeline(&b));

        let mut seek = a.clone();
        if let Some(p) = seek.now_playing.as_mut() {
            p.position = Duration::from_secs(120);
        }
        assert!(!a.same_except_timeline(&seek));

        let mut paused = a.clone();
        if let Some(p) = paused.now_playing.as_mut() {
            p.playing = false;
        }
        assert!(!a.same_except_timeline(&paused));
    }
}

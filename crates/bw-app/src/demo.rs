//! Module de démonstration (`[modules.demo] enabled = true`) : fait tourner
//! les niveaux d'attention et simule un lecteur, pour tester l'île sans vrais
//! modules (et hors Windows).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bw_core::{Attention, Module, ModuleCtx};
use bw_media::{Artwork, MediaSnapshot, NowPlaying};

const STEPS: [(Attention, Option<&str>); 3] = [
    (Attention::High, Some("Réunion dans 5 min")),
    (Attention::Urgent, Some("Claude attend ta réponse")),
    (Attention::None, None),
];

/// Titre, artiste, durée (s), couleurs de la pochette.
struct Track(&'static str, &'static str, u64, [u8; 3], [u8; 3]);

const TRACKS: [Track; 2] = [
    Track(
        "Digital Love",
        "Daft Punk",
        301,
        [255, 120, 40],
        [120, 30, 160],
    ),
    Track("Midnight City", "M83", 244, [40, 140, 255], [20, 20, 80]),
];

#[derive(Default)]
pub struct Demo {
    player: Arc<Mutex<Player>>,
    ctx: Option<ModuleCtx>,
}

#[derive(Default)]
struct Player {
    track: usize,
    playing: bool,
    position: Duration,
    position_at: Option<Instant>,
}

impl Player {
    fn position_now(&self) -> Duration {
        match (self.playing, self.position_at) {
            (true, Some(at)) => self.position + at.elapsed(),
            _ => self.position,
        }
    }

    fn set_position(&mut self, position: Duration) {
        self.position = position;
        self.position_at = Some(Instant::now());
    }

    fn snapshot(&self) -> MediaSnapshot {
        let Track(title, artist, secs, top, bottom) = TRACKS[self.track];
        MediaSnapshot {
            now_playing: Some(NowPlaying {
                source_id: "demo".into(),
                source: "Démo".into(),
                title: title.into(),
                artist: artist.into(),
                album: String::new(),
                playing: self.playing,
                position: self.position,
                position_at: self.position_at.unwrap_or_else(Instant::now),
                duration: Some(Duration::from_secs(secs)),
                can_previous: true,
                can_next: true,
                can_play_pause: true,
                can_seek: true,
                artwork: Some(gradient(top, bottom)),
            }),
            source_count: 1,
        }
    }
}

/// Pochette générée : dégradé diagonal.
fn gradient(a: [u8; 3], b: [u8; 3]) -> Artwork {
    const N: u32 = 96;
    let mut rgba = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let t = (x + y) as f32 / (2 * (N - 1)) as f32;
            for c in 0..3 {
                rgba.push((f32::from(a[c]) * (1.0 - t) + f32::from(b[c]) * t) as u8);
            }
            rgba.push(255);
        }
    }
    Artwork::from_rgba(N, N, rgba)
}

impl Demo {
    fn publish(&self) {
        let (Some(ctx), Ok(player)) = (&self.ctx, self.player.lock()) else {
            return;
        };
        let snapshot = player.snapshot();
        // L'attention « musique » passe par le module demo lui-même.
        let (level, summary) = snapshot.attention();
        ctx.set_attention(level, summary);
        ctx.set_state(snapshot);
    }
}

impl Module for Demo {
    fn id(&self) -> &'static str {
        "demo"
    }

    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        self.ctx = Some(ctx.clone());
        if let Ok(mut p) = self.player.lock() {
            p.playing = true;
            p.set_position(Duration::from_secs(42));
        }
        self.publish();

        // Après un tour de musique, les autres niveaux d'attention défilent.
        let task_ctx = ctx.clone();
        ctx.spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(6));
            tick.tick().await;
            for (level, summary) in STEPS.iter().cycle() {
                tick.tick().await;
                if *level != Attention::None {
                    task_ctx.set_attention(*level, summary.map(str::to_owned));
                }
            }
        });
        Ok(())
    }

    fn on_action(&mut self, action: &str) {
        if let Ok(mut p) = self.player.lock() {
            let position = p.position_now();
            match action {
                "toggle" => {
                    p.playing = !p.playing;
                    p.set_position(position);
                }
                "next" | "prev" => {
                    p.track = (p.track + 1) % TRACKS.len();
                    p.set_position(Duration::ZERO);
                }
                _ => match action.strip_prefix("seek:").and_then(|ms| ms.parse().ok()) {
                    Some(ms) => p.set_position(Duration::from_millis(ms)),
                    None => return,
                },
            }
        }
        self.publish();
    }
}

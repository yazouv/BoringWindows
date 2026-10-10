//! Gestes sur l'île, sans l'ouvrir : molette pour le volume, glissement
//! horizontal (pavé tactile, Maj + molette, ou glisser à la souris) pour
//! changer de morceau, appui long pour « ne pas déranger ».
//!
//! `Wheel` est pur : l'instant de chaque événement est passé en paramètre.

use std::time::{Duration, Instant};

use bw_i18n::tr;
use serde::Deserialize;

/// Section `[modules.gestures]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GesturesConfig {
    pub enabled: bool,
    /// Pas de volume par cran de molette (%).
    pub volume_step: u32,
}

impl Default for GesturesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            volume_step: 2,
        }
    }
}

impl GesturesConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            (1..=10).contains(&config.volume_step),
            tr!(
                "modules.gestures.volume_step must be between 1 and 10",
                "modules.gestures.volume_step doit être entre 1 et 10"
            )
        );
        Ok(config)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    VolumeUp,
    VolumeDown,
    Next,
    Previous,
}

/// Distance (pixels logiques) d'un cran de molette, telle que Slint la donne.
pub const NOTCH: f32 = 60.0;
/// Glissement horizontal qui change de morceau.
const SWIPE: f32 = 120.0;
/// Après un changement de morceau, le reste du geste est ignoré.
const SWIPE_COOLDOWN: Duration = Duration::from_millis(700);
/// Un geste s'arrête après ce silence.
const GESTURE_GAP: Duration = Duration::from_millis(250);

/// Cumule les défilements : une molette donne un cran par événement, un pavé
/// tactile une pluie de petits déplacements.
#[derive(Debug, Default)]
pub struct Wheel {
    dx: f32,
    dy: f32,
    last: Option<Instant>,
    cooldown_until: Option<Instant>,
}

impl Wheel {
    /// `dy` > 0 : vers le haut (monter le volume) ; `dx` > 0 : vers la droite.
    pub fn feed(&mut self, dx: f32, dy: f32, now: Instant) -> Vec<Gesture> {
        if self
            .last
            .is_none_or(|t| now.duration_since(t) > GESTURE_GAP)
        {
            self.dx = 0.0;
            self.dy = 0.0;
        }
        self.last = Some(now);

        let mut out = Vec::new();
        // Un geste surtout horizontal ne touche pas au volume, et inversement.
        if dx.abs() > dy.abs() {
            if self.cooldown_until.is_some_and(|t| now < t) {
                return out;
            }
            self.dx += dx;
            if self.dx.abs() >= SWIPE {
                out.push(if self.dx > 0.0 {
                    Gesture::Previous
                } else {
                    Gesture::Next
                });
                self.dx = 0.0;
                self.cooldown_until = Some(now + SWIPE_COOLDOWN);
            }
        } else {
            self.dy += dy;
            while self.dy.abs() >= NOTCH {
                let up = self.dy > 0.0;
                out.push(if up {
                    Gesture::VolumeUp
                } else {
                    Gesture::VolumeDown
                });
                self.dy -= NOTCH.copysign(self.dy);
            }
        }
        out
    }
}

/// Glisser-déposer horizontal à la souris sur l'île (`dx` : déplacement
/// entre l'appui et le relâchement).
pub fn drag(dx: f32) -> Option<Gesture> {
    if dx >= SWIPE / 2.0 {
        Some(Gesture::Previous)
    } else if dx <= -SWIPE / 2.0 {
        Some(Gesture::Next)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_wheel_gives_one_step_per_notch() {
        let mut w = Wheel::default();
        let t = Instant::now();
        assert_eq!(w.feed(0.0, NOTCH, t), [Gesture::VolumeUp]);
        assert_eq!(w.feed(0.0, -NOTCH, t), [Gesture::VolumeDown]);
        assert_eq!(
            w.feed(0.0, 2.0 * NOTCH, t),
            [Gesture::VolumeUp, Gesture::VolumeUp]
        );
    }

    #[test]
    fn touchpad_scroll_accumulates_and_resets_after_a_pause() {
        let mut w = Wheel::default();
        let t = Instant::now();
        for i in 0..5 {
            assert!(
                w.feed(0.0, 10.0, t + Duration::from_millis(i * 10))
                    .is_empty()
            );
        }
        assert_eq!(
            w.feed(0.0, 10.0, t + Duration::from_millis(50)),
            [Gesture::VolumeUp]
        );
        // Après une pause, le cumul repart de zéro.
        let later = t + Duration::from_secs(1);
        assert!(w.feed(0.0, 50.0, later).is_empty());
    }

    #[test]
    fn horizontal_swipe_changes_track_once() {
        let mut w = Wheel::default();
        let t = Instant::now();
        let mut got = Vec::new();
        for i in 0..20 {
            got.extend(w.feed(-20.0, 2.0, t + Duration::from_millis(i * 10)));
        }
        assert_eq!(got, [Gesture::Next]);
        assert_eq!(drag(100.0), Some(Gesture::Previous));
        assert_eq!(drag(-100.0), Some(Gesture::Next));
        assert_eq!(drag(20.0), None);
    }

    #[test]
    fn config_validation() {
        assert!(GesturesConfig::from_table(None).unwrap().enabled);
        let t: toml::Table = toml::from_str("volume_step = 0").unwrap();
        assert!(GesturesConfig::from_table(Some(&t)).is_err());
    }
}

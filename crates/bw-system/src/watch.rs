//! Logique pure des alertes : une valeur qui reste au-dessus d'un seuil
//! pendant un certain temps déclenche une alerte, une seule fois, jusqu'à ce
//! qu'elle redescende nettement.

use std::time::{Duration, Instant};

/// Écart sous le seuil pour réarmer l'alerte (points de pourcentage) : une
/// valeur qui oscille autour du seuil ne déclenche pas d'alerte en boucle.
const HYSTERESIS: f32 = 10.0;

#[derive(Debug, Clone)]
pub struct Watch {
    /// Seuil en % ; 0 : jamais d'alerte.
    threshold: f32,
    sustain: Duration,
    over_since: Option<Instant>,
    fired: bool,
}

impl Watch {
    pub fn new(threshold: u8, sustain: Duration) -> Self {
        Self {
            threshold: f32::from(threshold),
            sustain,
            over_since: None,
            fired: false,
        }
    }

    /// Nouvelle mesure (en %) ; `true` : c'est le moment d'alerter.
    pub fn feed(&mut self, value: f32, now: Instant) -> bool {
        if self.threshold <= 0.0 {
            return false;
        }
        if value < self.threshold {
            self.over_since = None;
            if value < self.threshold - HYSTERESIS {
                self.fired = false;
            }
            return false;
        }
        let since = *self.over_since.get_or_insert(now);
        if !self.fired && now.duration_since(since) >= self.sustain {
            self.fired = true;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alerts_once_after_sustained_load() {
        let t0 = Instant::now();
        let s = |secs| t0 + Duration::from_secs(secs);
        let mut w = Watch::new(90, Duration::from_secs(30));
        assert!(!w.feed(95.0, s(0)));
        assert!(!w.feed(96.0, s(20)));
        assert!(w.feed(97.0, s(30)));
        // Toujours chargé : pas de nouvelle alerte.
        assert!(!w.feed(99.0, s(60)));
        // Redescend un peu sous le seuil puis remonte : pas réarmée.
        assert!(!w.feed(85.0, s(65)));
        assert!(!w.feed(95.0, s(70)));
        assert!(!w.feed(95.0, s(110)));
        // Redescend nettement : réarmée, il faut de nouveau 30 s.
        assert!(!w.feed(50.0, s(120)));
        assert!(!w.feed(95.0, s(125)));
        assert!(!w.feed(95.0, s(150)));
        assert!(w.feed(95.0, s(155)));
    }

    #[test]
    fn short_spikes_and_disabled_watch() {
        let t0 = Instant::now();
        let s = |secs| t0 + Duration::from_secs(secs);
        let mut w = Watch::new(90, Duration::from_secs(30));
        assert!(!w.feed(100.0, s(0)));
        assert!(!w.feed(20.0, s(10)));
        assert!(!w.feed(100.0, s(31)));
        assert!(!w.feed(100.0, s(40)));

        let mut off = Watch::new(0, Duration::from_secs(5));
        assert!(!off.feed(100.0, s(0)));
        assert!(!off.feed(100.0, s(100)));
    }
}

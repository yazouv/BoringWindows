//! Machine à états du minuteur : pure (l'heure est passée en paramètre), donc
//! testable sans attendre.

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Running,
    Paused,
    /// Terminé : l'île le signale pendant `done_for`.
    Done,
}

/// État publié vers l'île.
#[derive(Debug, Clone)]
pub struct TimerSnapshot {
    pub phase: Phase,
    pub total: Duration,
    /// Temps restant à l'instant `at` (l'île en déduit la suite).
    pub remaining: Duration,
    pub at: Instant,
    pub presets: Vec<u32>,
}

impl TimerSnapshot {
    pub fn remaining_now(&self, now: Instant) -> Duration {
        if self.phase == Phase::Running {
            self.remaining.saturating_sub(now.saturating_duration_since(self.at))
        } else {
            self.remaining
        }
    }

    pub fn progress_now(&self, now: Instant) -> f32 {
        if self.total.is_zero() {
            return 0.0;
        }
        1.0 - (self.remaining_now(now).as_secs_f32() / self.total.as_secs_f32()).clamp(0.0, 1.0)
    }
}

pub(crate) struct Machine {
    phase: Phase,
    total: Duration,
    /// Reste, valable hors `Running`.
    remaining: Duration,
    ends_at: Option<Instant>,
    done_until: Option<Instant>,
    done_for: Duration,
    presets: Vec<u32>,
}

impl Machine {
    pub fn new(presets: Vec<u32>, done_for: Duration) -> Self {
        Self {
            phase: Phase::Idle,
            total: Duration::ZERO,
            remaining: Duration::ZERO,
            ends_at: None,
            done_until: None,
            done_for,
            presets,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn start(&mut self, minutes: u32, now: Instant) {
        self.total = Duration::from_secs(u64::from(minutes) * 60);
        self.remaining = self.total;
        self.ends_at = Some(now + self.total);
        self.done_until = None;
        self.phase = Phase::Running;
    }

    pub fn pause(&mut self, now: Instant) {
        if self.phase == Phase::Running {
            self.remaining = self.remaining(now);
            self.ends_at = None;
            self.phase = Phase::Paused;
        }
    }

    pub fn resume(&mut self, now: Instant) {
        if self.phase == Phase::Paused {
            self.ends_at = Some(now + self.remaining);
            self.phase = Phase::Running;
        }
    }

    pub fn reset(&mut self) {
        self.phase = Phase::Idle;
        self.total = Duration::ZERO;
        self.remaining = Duration::ZERO;
        self.ends_at = None;
        self.done_until = None;
    }

    pub fn remaining(&self, now: Instant) -> Duration {
        match (self.phase, self.ends_at) {
            (Phase::Running, Some(end)) => end.saturating_duration_since(now),
            _ => self.remaining,
        }
    }

    /// Applique les échéances dépassées ; vrai si la phase a changé.
    pub fn tick(&mut self, now: Instant) -> bool {
        match self.phase {
            Phase::Running if self.ends_at.is_some_and(|e| now >= e) => {
                self.phase = Phase::Done;
                self.remaining = Duration::ZERO;
                self.ends_at = None;
                self.done_until = Some(now + self.done_for);
                true
            }
            Phase::Done if self.done_until.is_some_and(|d| now >= d) => {
                self.reset();
                true
            }
            _ => false,
        }
    }

    /// Minutes affichées dans la pilule (arrondi au-dessus).
    pub fn minutes_left(&self, now: Instant) -> u64 {
        self.remaining(now).as_secs().div_ceil(60)
    }

    /// Prochain instant où l'état publié change.
    pub fn next_wake(&self, now: Instant) -> Option<Instant> {
        match self.phase {
            Phase::Running => {
                let left = self.remaining(now);
                let m = self.minutes_left(now);
                let to_next_minute = left.saturating_sub(Duration::from_secs((m - 1) * 60));
                Some(now + to_next_minute.max(Duration::from_millis(50)))
            }
            Phase::Done => self.done_until,
            _ => None,
        }
    }

    pub fn snapshot(&self, now: Instant) -> TimerSnapshot {
        TimerSnapshot {
            phase: self.phase,
            total: self.total,
            remaining: self.remaining(now),
            at: now,
            presets: self.presets.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine() -> (Machine, Instant) {
        (
            Machine::new(vec![5], Duration::from_secs(20)),
            Instant::now(),
        )
    }

    fn secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    #[test]
    fn runs_to_done_then_back_to_idle() {
        let (mut m, t0) = machine();
        m.start(2, t0);
        assert_eq!(m.phase(), Phase::Running);
        assert_eq!(m.remaining(t0 + secs(30)), secs(90));
        assert!(!m.tick(t0 + secs(119)));
        assert!(m.tick(t0 + secs(120)));
        assert_eq!(m.phase(), Phase::Done);
        assert_eq!(m.next_wake(t0 + secs(121)), Some(t0 + secs(140)));
        assert!(!m.tick(t0 + secs(139)));
        assert!(m.tick(t0 + secs(140)));
        assert_eq!(m.phase(), Phase::Idle);
        assert_eq!(m.next_wake(t0 + secs(141)), None);
    }

    #[test]
    fn pause_freezes_and_resume_continues() {
        let (mut m, t0) = machine();
        m.start(10, t0);
        m.pause(t0 + secs(60));
        assert_eq!(m.phase(), Phase::Paused);
        assert_eq!(m.remaining(t0 + secs(500)), secs(540));
        assert!(!m.tick(t0 + secs(5000)));
        m.resume(t0 + secs(600));
        assert_eq!(m.remaining(t0 + secs(660)), secs(480));
        m.reset();
        assert_eq!(m.phase(), Phase::Idle);
    }

    #[test]
    fn wakes_at_each_minute_boundary() {
        let (mut m, t0) = machine();
        m.start(3, t0);
        assert_eq!(m.minutes_left(t0), 3);
        // 180 s restantes : la pilule passe de « 3 min » à « 2 min » à 120 s.
        assert_eq!(m.next_wake(t0), Some(t0 + secs(60)));
        assert_eq!(m.minutes_left(t0 + secs(60)), 2);
        assert_eq!(m.minutes_left(t0 + secs(61)), 2);
        // Dernière minute : on attend la fin.
        assert_eq!(m.next_wake(t0 + secs(130)), Some(t0 + secs(180)));
    }

    #[test]
    fn snapshot_extrapolates_while_running() {
        let (mut m, t0) = machine();
        m.start(1, t0);
        let s = m.snapshot(t0);
        assert_eq!(s.remaining_now(t0 + secs(15)), secs(45));
        assert!((s.progress_now(t0 + secs(30)) - 0.5).abs() < 1e-3);
        m.pause(t0 + secs(30));
        let p = m.snapshot(t0 + secs(30));
        assert_eq!(p.remaining_now(t0 + secs(300)), secs(30));
    }
}

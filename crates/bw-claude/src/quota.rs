//! Alerte de consommation : la fenêtre en cours dépasse `alert_percent` de la
//! limite estimée, puis elle se renouvelle. Logique pure : le temps est passé
//! en paramètre.

use bw_core::Attention;
use bw_i18n::tr;
use chrono::{DateTime, Utc};

use crate::transcripts::UsageSummary;

/// Annonce à faire dans la pilule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaNotice {
    pub attention: Attention,
    pub text: String,
}

/// Retient la fenêtre pour laquelle on a déjà prévenu : une alerte par fenêtre.
#[derive(Debug, Default)]
pub struct QuotaWatch {
    /// Fin de la fenêtre déjà signalée.
    alerted: Option<DateTime<Utc>>,
}

impl QuotaWatch {
    /// Après une lecture des transcripts. `None` : rien à annoncer.
    pub fn update(
        &mut self,
        usage: &UsageSummary,
        limit_tokens: u64,
        alert_percent: u8,
        now: DateTime<Utc>,
    ) -> Option<QuotaNotice> {
        if alert_percent == 0 || limit_tokens == 0 {
            self.alerted = None;
            return None;
        }
        let end = usage.window_end.filter(|end| *end > now)?;
        let percent = percent(usage.tokens, limit_tokens);
        if percent < u64::from(alert_percent) || self.alerted == Some(end) {
            return None;
        }
        self.alerted = Some(end);
        let reset = until(end, now);
        Some(QuotaNotice {
            attention: if percent >= 100 {
                Attention::Urgent
            } else {
                Attention::High
            },
            text: tr!(
                "Claude: {percent} % of the window used · resets in {reset}",
                "Claude : {percent} % de la fenêtre utilisés · reset dans {reset}"
            ),
        })
    }

    /// Fin de la fenêtre signalée, s'il y en a une : le moment de `renewed`.
    pub fn next_deadline(&self) -> Option<DateTime<Utc>> {
        self.alerted
    }

    /// La fenêtre signalée est passée : la consommation repart de zéro.
    pub fn renewed(&mut self, now: DateTime<Utc>) -> Option<QuotaNotice> {
        if self.alerted.is_none_or(|end| end > now) {
            return None;
        }
        self.alerted = None;
        Some(QuotaNotice {
            attention: Attention::High,
            text: tr!(
                "Claude: new usage window",
                "Claude : nouvelle fenêtre de consommation"
            ),
        })
    }
}

fn percent(tokens: u64, limit: u64) -> u64 {
    (u128::from(tokens) * 100 / u128::from(limit.max(1))) as u64
}

/// « 1 h 20 », « 35 min », « 1 min ».
fn until(end: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let minutes = (end - now).num_minutes().max(1);
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m:02}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    fn usage(tokens: u64, end: DateTime<Utc>) -> UsageSummary {
        UsageSummary {
            window_end: Some(end),
            tokens,
            messages: 1,
        }
    }

    fn now() -> DateTime<Utc> {
        // Les textes vérifiés ici sont en français, comme dans le reste du crate.
        bw_i18n::set(bw_i18n::Lang::Fr);
        Utc.with_ymd_and_hms(2026, 10, 15, 12, 0, 0).unwrap()
    }

    #[test]
    fn alerts_once_per_window() {
        let now = now();
        let end = now + Duration::minutes(80);
        let mut w = QuotaWatch::default();
        assert_eq!(w.update(&usage(700, end), 1000, 80, now), None);
        let notice = w.update(&usage(820, end), 1000, 80, now).unwrap();
        assert_eq!(notice.attention, Attention::High);
        assert_eq!(
            notice.text,
            "Claude : 82 % de la fenêtre utilisés · reset dans 1 h 20"
        );
        // Déjà prévenu pour cette fenêtre.
        assert_eq!(w.update(&usage(900, end), 1000, 80, now), None);
        assert_eq!(w.next_deadline(), Some(end));
    }

    #[test]
    fn over_the_limit_is_urgent() {
        let now = now();
        let mut w = QuotaWatch::default();
        let notice = w
            .update(&usage(1500, now + Duration::minutes(5)), 1000, 80, now)
            .unwrap();
        assert_eq!(notice.attention, Attention::Urgent);
        assert!(notice.text.contains("150 %"));
        assert!(notice.text.ends_with("5 min"));
    }

    #[test]
    fn renewal_is_announced_after_an_alert_only() {
        let now = now();
        let end = now + Duration::hours(2);
        let mut w = QuotaWatch::default();
        assert_eq!(w.renewed(end), None);
        w.update(&usage(900, end), 1000, 80, now).unwrap();
        assert_eq!(w.renewed(end - Duration::seconds(1)), None);
        let notice = w.renewed(end).unwrap();
        assert_eq!(notice.text, "Claude : nouvelle fenêtre de consommation");
        assert_eq!(w.next_deadline(), None);
        // La fenêtre suivante peut de nouveau alerter.
        let next = end + Duration::hours(5);
        assert!(w.update(&usage(950, next), 1000, 80, end).is_some());
    }

    #[test]
    fn off_without_limit_or_threshold_or_window() {
        let now = now();
        let end = now + Duration::hours(1);
        let mut w = QuotaWatch::default();
        assert_eq!(w.update(&usage(5000, end), 0, 80, now), None);
        assert_eq!(w.update(&usage(5000, end), 1000, 0, now), None);
        let idle = UsageSummary {
            window_end: None,
            tokens: 0,
            messages: 0,
        };
        assert_eq!(w.update(&idle, 1000, 80, now), None);
        // Fenêtre déjà finie (transcripts relus en retard).
        assert_eq!(w.update(&usage(900, now), 1000, 80, now), None);
    }
}

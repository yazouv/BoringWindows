//! Ce que l'île affiche de l'agenda à un instant donné, et quand le
//! recalculer. Logique pure : le temps est passé en paramètre.

use chrono::{DateTime, Duration, Local, Utc};

use bw_core::Attention;

use crate::config::CalendarConfig;
use crate::ics::Event;

/// Une réunion reste « à rejoindre » ce temps après son début.
const JOIN_GRACE: Duration = Duration::minutes(10);
/// En deçà, on affiche un compte à rebours (« dans 12 min »).
const COUNTDOWN: Duration = Duration::minutes(60);
const MAX_ITEMS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgendaItem {
    pub title: String,
    /// « 09:30 », « demain 09:30 », « toute la journée ».
    pub time: String,
    /// « dans 12 min », « en cours »…
    pub relative: Option<String>,
    pub join_url: Option<String>,
    /// Imminent ou en cours : mis en valeur.
    pub soon: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CalendarSnapshot {
    pub items: Vec<AgendaItem>,
    pub attention: Attention,
    pub summary: Option<String>,
    /// Dernière erreur de téléchargement, s'il y en a une.
    pub error: Option<String>,
}

/// Agenda à l'instant `now`, et prochain instant où il changera.
pub fn agenda(
    events: &[Event],
    now: DateTime<Utc>,
    config: &CalendarConfig,
) -> (CalendarSnapshot, Option<DateTime<Utc>>) {
    let horizon = now + Duration::hours(config.lookahead_hours.into());
    let remind = Duration::minutes(config.remind_minutes.into());

    let mut visible: Vec<&Event> = events
        .iter()
        .filter(|e| e.end > now && e.start < horizon)
        .filter(|e| config.show_all_day || !e.all_day)
        .collect();
    // Réunions d'abord, journées entières ensuite.
    visible.sort_by_key(|e| (e.all_day, e.start));

    let items = visible
        .iter()
        .take(MAX_ITEMS)
        .map(|e| item(e, now, remind))
        .collect();

    let next_meeting = visible.iter().find(|e| !e.all_day);
    let (attention, summary) = match next_meeting {
        Some(e) if e.start <= now && now < e.start + JOIN_GRACE => {
            (Attention::High, Some(format!("{} · a commencé", e.title)))
        }
        Some(e) if e.start > now && e.start - now <= remind => (
            Attention::High,
            Some(format!("{} {}", e.title, countdown(e.start - now))),
        ),
        Some(e) if e.start > now && e.start - now <= COUNTDOWN => (
            Attention::Low,
            Some(format!("{} à {}", e.title, clock(e.start))),
        ),
        _ => (Attention::None, None),
    };

    (
        CalendarSnapshot {
            items,
            attention,
            summary,
            error: None,
        },
        next_change(&visible, now, remind),
    )
}

fn item(e: &Event, now: DateTime<Utc>, remind: Duration) -> AgendaItem {
    let ongoing = e.start <= now;
    let until = e.start - now;
    let relative = if e.all_day {
        None
    } else if ongoing {
        Some("en cours".into())
    } else if until <= COUNTDOWN {
        Some(countdown(until))
    } else {
        None
    };
    AgendaItem {
        title: e.title.clone(),
        time: if e.all_day {
            "toute la journée".into()
        } else if e.start.with_timezone(&Local).date_naive()
            != now.with_timezone(&Local).date_naive()
        {
            format!("demain {}", clock(e.start))
        } else {
            clock(e.start)
        },
        relative,
        join_url: e.join_url.clone(),
        soon: !e.all_day && (ongoing || until <= remind),
    }
}

fn clock(t: DateTime<Utc>) -> String {
    t.with_timezone(&Local).format("%H:%M").to_string()
}

/// « dans 12 min », « dans 1 h 05 », « maintenant ».
fn countdown(d: Duration) -> String {
    // Minute entamée : à 4 min 10 s, on annonce « dans 5 min ».
    let minutes = (d.num_seconds() + 59) / 60;
    match minutes {
        ..=0 => "maintenant".into(),
        1..=59 => format!("dans {minutes} min"),
        _ => format!("dans {} h {:02}", minutes / 60, minutes % 60),
    }
}

/// Prochain instant où l'affichage change : seuils autour des réunions,
/// et chaque minute tant qu'un compte à rebours est visible.
fn next_change(events: &[&Event], now: DateTime<Utc>, remind: Duration) -> Option<DateTime<Utc>> {
    let mut candidates = Vec::new();
    for e in events {
        candidates.push(e.end);
        if !e.all_day {
            candidates.extend([
                e.start - COUNTDOWN,
                e.start - remind,
                e.start,
                e.start + JOIN_GRACE,
            ]);
            if e.start > now && e.start - now <= COUNTDOWN {
                // Le compte à rebours change à chaque minute (alignée sur le début).
                let secs = (e.start - now).num_seconds() % 60;
                let step = if secs == 0 { 60 } else { secs };
                candidates.push(now + Duration::seconds(step));
            }
        } else {
            candidates.push(e.start);
        }
    }
    candidates.into_iter().filter(|t| *t > now).min()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(min: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-01T08:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
            + Duration::minutes(min)
    }

    fn meeting(title: &str, start: i64, len: i64) -> Event {
        Event {
            uid: title.into(),
            title: title.into(),
            start: at(start),
            end: at(start + len),
            all_day: false,
            location: None,
            join_url: Some("https://meet.google.com/x".into()),
        }
    }

    fn config() -> CalendarConfig {
        CalendarConfig::default()
    }

    #[test]
    fn far_meeting_is_listed_without_attention() {
        let (s, next) = agenda(&[meeting("Revue", 120, 30)], at(0), &config());
        assert_eq!(s.attention, Attention::None);
        assert_eq!(s.items[0].relative, None);
        // Prochain changement : quand le compte à rebours commence.
        assert_eq!(next, Some(at(60)));
    }

    #[test]
    fn countdown_then_reminder_then_started() {
        let events = [meeting("Daily", 30, 15)];
        let (s, _) = agenda(&events, at(0), &config());
        assert_eq!(s.attention, Attention::Low);
        assert_eq!(s.items[0].relative.as_deref(), Some("dans 30 min"));

        let (s, next) = agenda(&events, at(26), &config());
        assert_eq!(s.attention, Attention::High);
        assert_eq!(s.summary.as_deref(), Some("Daily dans 4 min"));
        assert!(s.items[0].soon);
        assert_eq!(next, Some(at(27)));

        let (s, _) = agenda(&events, at(32), &config());
        assert_eq!(s.summary.as_deref(), Some("Daily · a commencé"));
        assert_eq!(s.items[0].relative.as_deref(), Some("en cours"));

        // Après le délai pour rejoindre : encore listée, plus d'alerte.
        let (s, next) = agenda(&events, at(41), &config());
        assert_eq!(s.attention, Attention::None);
        assert_eq!(s.items.len(), 1);
        assert_eq!(next, Some(at(45)));

        let (s, _) = agenda(&events, at(46), &config());
        assert!(s.items.is_empty());
    }

    #[test]
    fn partial_minutes_round_up() {
        // 4 min 10 s avant : dans la fenêtre de rappel, annoncé « dans 5 min ».
        let mut ev = meeting("X", 4, 10);
        ev.start += Duration::seconds(10);
        let (s, next) = agenda(&[ev], at(0), &config());
        assert_eq!(s.summary.as_deref(), Some("X dans 5 min"));
        assert_eq!(next, Some(at(0) + Duration::seconds(10)));
    }

    #[test]
    fn all_day_events_never_alert_and_come_last() {
        let mut day = meeting("Congé", -60, 24 * 60);
        day.all_day = true;
        let events = [day, meeting("Point", 3, 15)];
        let (s, _) = agenda(&events, at(0), &config());
        assert_eq!(s.items[0].title, "Point");
        assert_eq!(s.items[1].time, "toute la journée");
        assert_eq!(s.attention, Attention::High);

        let hidden = CalendarConfig {
            show_all_day: false,
            ..config()
        };
        let (s, _) = agenda(&events, at(0), &hidden);
        assert_eq!(s.items.len(), 1);
    }

    #[test]
    fn countdown_text() {
        assert_eq!(countdown(Duration::seconds(0)), "maintenant");
        assert_eq!(countdown(Duration::seconds(30)), "dans 1 min");
        assert_eq!(countdown(Duration::minutes(65)), "dans 1 h 05");
    }
}

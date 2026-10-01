//! Lecture des calendriers iCalendar (RFC 5545) : juste ce qu'il faut pour
//! un agenda — événements, récurrences, exceptions, fuseaux horaires.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use rrule::RRuleSet;

use crate::join::find_join_url;
use crate::tz;

/// Occurrence d'un événement, prête à afficher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub uid: String,
    pub title: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub all_day: bool,
    pub location: Option<String>,
    pub join_url: Option<String>,
}

/// Date d'un champ ICS : jour entier ou instant précis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum When {
    Date(NaiveDate),
    Instant(DateTime<Utc>),
}

impl When {
    fn utc(self) -> DateTime<Utc> {
        match self {
            // Jour entier : minuit heure locale.
            Self::Date(d) => local_to_utc(d.and_hms_opt(0, 0, 0).unwrap_or_default()),
            Self::Instant(t) => t,
        }
    }
}

#[derive(Debug, Default)]
struct RawEvent {
    uid: String,
    summary: String,
    start: Option<When>,
    /// Ligne DTSTART d'origine, pour construire la règle de récurrence.
    dtstart_rrule: Option<String>,
    end: Option<When>,
    duration: Option<Duration>,
    rrule: Vec<String>,
    exdates: Vec<DateTime<Utc>>,
    recurrence_id: Option<DateTime<Utc>>,
    cancelled: bool,
    location: Option<String>,
    texts: Vec<String>,
}

/// Toutes les occurrences qui chevauchent `[from, to)`, triées par début.
pub fn events_between(ics: &str, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<Event> {
    let raws = parse(ics);

    // Exceptions (RECURRENCE-ID) : elles remplacent une occurrence de la série.
    let mut overridden: HashMap<&str, HashSet<DateTime<Utc>>> = HashMap::new();
    for r in raws.iter() {
        if let Some(rid) = r.recurrence_id {
            overridden.entry(&r.uid).or_default().insert(rid);
        }
    }

    let mut out = Vec::new();
    for r in &raws {
        if r.cancelled {
            continue;
        }
        let Some(start) = r.start else { continue };
        let all_day = matches!(start, When::Date(_));
        let length = match (r.end, r.duration) {
            (Some(end), _) => end.utc() - start.utc(),
            (None, Some(d)) => d,
            (None, None) if all_day => Duration::days(1),
            (None, None) => Duration::zero(),
        }
        .max(Duration::zero());

        let starts: Vec<DateTime<Utc>> = if r.rrule.is_empty() || r.recurrence_id.is_some() {
            vec![start.utc()]
        } else {
            expand(r, from - length, to)
                .into_iter()
                .filter(|s| {
                    !overridden
                        .get(r.uid.as_str())
                        .is_some_and(|set| set.contains(s))
                })
                .collect()
        };

        let join_url = find_join_url(
            r.location
                .iter()
                .map(String::as_str)
                .chain(r.texts.iter().map(String::as_str)),
        );
        for s in starts {
            let end = s + length;
            // Chevauche la fenêtre (un événement sans durée compte à son début).
            if s < to && (end > from || (length.is_zero() && s >= from)) {
                out.push(Event {
                    uid: r.uid.clone(),
                    title: if r.summary.is_empty() {
                        bw_i18n::tr!("(untitled)", "(sans titre)")
                    } else {
                        r.summary.clone()
                    },
                    start: s,
                    end,
                    all_day,
                    location: r.location.clone(),
                    join_url: join_url.clone(),
                });
            }
        }
    }
    out.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.title.cmp(&b.title)));
    out
}

/// Débuts des occurrences d'une série entre `from` et `to`.
fn expand(r: &RawEvent, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<DateTime<Utc>> {
    let Some(dtstart) = &r.dtstart_rrule else {
        return Vec::new();
    };
    let mut text = dtstart.clone();
    for rule in &r.rrule {
        text.push_str("\nRRULE:");
        text.push_str(rule);
    }
    let set: RRuleSet = match text.parse() {
        Ok(s) => s,
        Err(e) => {
            log::debug!("agenda : récurrence ignorée ({e}) pour {}", r.summary);
            return Vec::new();
        }
    };
    let excluded: HashSet<DateTime<Utc>> = r.exdates.iter().copied().collect();
    let utc = rrule::Tz::UTC;
    set.after(from.with_timezone(&utc) - Duration::seconds(1))
        .before(to.with_timezone(&utc))
        .all(500)
        .dates
        .into_iter()
        .map(|d| d.with_timezone(&Utc))
        .filter(|d| !excluded.contains(d))
        .collect()
}

// ---------------------------------------------------------------------------
// Lecture ligne à ligne

struct Property<'a> {
    name: String,
    params: Vec<(String, String)>,
    value: &'a str,
}

impl Property<'_> {
    fn param(&self, key: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }
}

/// Lignes « dépliées » : une ligne commençant par un espace prolonge la précédente.
fn unfold(ics: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in ics.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        match (line.strip_prefix([' ', '\t']), lines.last_mut()) {
            (Some(rest), Some(last)) => last.push_str(rest),
            _ => lines.push(line.to_owned()),
        }
    }
    lines
}

fn property(line: &str) -> Option<Property<'_>> {
    // Le « : » qui sépare nom/paramètres et valeur n'est pas entre guillemets.
    let mut in_quotes = false;
    let colon = line.char_indices().find_map(|(i, c)| {
        match c {
            '"' => in_quotes = !in_quotes,
            ':' if !in_quotes => return Some(i),
            _ => {}
        }
        None
    })?;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    let mut parts = head.split(';');
    let name = parts.next()?.to_ascii_uppercase();
    let params = parts
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.to_ascii_uppercase(), v.trim_matches('"').to_owned()))
        .collect();
    Some(Property {
        name,
        params,
        value,
    })
}

fn parse(ics: &str) -> Vec<RawEvent> {
    let mut events = Vec::new();
    let mut current: Option<RawEvent> = None;
    // Profondeur des sous-blocs (VALARM…) à ignorer dans un VEVENT.
    let mut nested = 0usize;

    for line in unfold(ics) {
        let Some(p) = property(&line) else { continue };
        match (p.name.as_str(), p.value) {
            ("BEGIN", "VEVENT") if current.is_none() => current = Some(RawEvent::default()),
            ("BEGIN", _) if current.is_some() => nested += 1,
            ("END", "VEVENT") if nested == 0 => events.extend(current.take()),
            ("END", _) if nested > 0 => nested -= 1,
            _ => {
                if let Some(e) = current.as_mut().filter(|_| nested == 0) {
                    apply(e, &p);
                }
            }
        }
    }
    events
}

fn apply(e: &mut RawEvent, p: &Property<'_>) {
    match p.name.as_str() {
        "UID" => e.uid = p.value.to_owned(),
        "SUMMARY" => e.summary = unescape(p.value),
        "DTSTART" => {
            e.start = parse_when(p);
            e.dtstart_rrule = e.start.map(|w| rrule_dtstart(p, w));
        }
        "DTEND" => e.end = parse_when(p),
        "DURATION" => e.duration = parse_duration(p.value),
        "RRULE" => e.rrule.push(p.value.to_owned()),
        "EXDATE" => {
            for v in p.value.split(',') {
                let single = Property {
                    name: p.name.clone(),
                    params: p.params.clone(),
                    value: v,
                };
                if let Some(w) = parse_when(&single) {
                    e.exdates.push(w.utc());
                }
            }
        }
        "RECURRENCE-ID" => e.recurrence_id = parse_when(p).map(When::utc),
        "STATUS" => e.cancelled = p.value.eq_ignore_ascii_case("CANCELLED"),
        "LOCATION" => e.location = Some(unescape(p.value)).filter(|s| !s.is_empty()),
        "DESCRIPTION"
        | "URL"
        | "X-GOOGLE-CONFERENCE"
        | "X-MICROSOFT-SKYPETEAMSMEETINGURL"
        | "X-MICROSOFT-ONLINEMEETINGCONFLINK"
        | "CONFERENCE" => {
            e.texts.push(unescape(p.value));
        }
        _ => {}
    }
}

fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n' | 'N') => out.push('\n'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn parse_when(p: &Property<'_>) -> Option<When> {
    let v = p.value.trim();
    if p.param("VALUE")
        .is_some_and(|t| t.eq_ignore_ascii_case("DATE"))
        || v.len() == 8
    {
        return NaiveDate::parse_from_str(v, "%Y%m%d").ok().map(When::Date);
    }
    if let Some(utc) = v.strip_suffix('Z') {
        let naive = NaiveDateTime::parse_from_str(utc, "%Y%m%dT%H%M%S").ok()?;
        return Some(When::Instant(Utc.from_utc_datetime(&naive)));
    }
    let naive = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok()?;
    let instant = match p.param("TZID").and_then(tz::resolve) {
        Some(zone) => zone
            .from_local_datetime(&naive)
            .earliest()
            .map(|t| t.with_timezone(&Utc))?,
        // Heure « flottante » ou fuseau inconnu : heure locale.
        None => local_to_utc(naive),
    };
    Some(When::Instant(instant))
}

/// DTSTART normalisé pour `rrule` : fuseau IANA connu, sinon UTC.
fn rrule_dtstart(p: &Property<'_>, when: When) -> String {
    let zone = p.param("TZID").and_then(tz::resolve);
    match (when, zone) {
        (When::Instant(_), Some(zone)) => {
            format!("DTSTART;TZID={}:{}", zone.name(), p.value.trim())
        }
        (When::Instant(t), None) => format!("DTSTART:{}", t.format("%Y%m%dT%H%M%SZ")),
        (When::Date(d), _) => format!(
            "DTSTART:{}",
            local_to_utc(d.and_hms_opt(0, 0, 0).unwrap_or_default()).format("%Y%m%dT%H%M%SZ")
        ),
    }
}

fn local_to_utc(naive: NaiveDateTime) -> DateTime<Utc> {
    Local
        .from_local_datetime(&naive)
        .earliest()
        .map_or_else(|| Utc.from_utc_datetime(&naive), |t| t.with_timezone(&Utc))
}

/// « PT1H30M », « P1D », « -PT15M », « P1W ».
fn parse_duration(v: &str) -> Option<Duration> {
    let (negative, v) = match v.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, v.strip_prefix('+').unwrap_or(v)),
    };
    let v = v.strip_prefix('P')?;
    let mut total = Duration::zero();
    let mut number = String::new();
    for c in v.chars() {
        match c {
            '0'..='9' => number.push(c),
            'T' => {}
            unit => {
                let n: i64 = number.parse().ok()?;
                number.clear();
                total += match unit {
                    'W' => Duration::weeks(n),
                    'D' => Duration::days(n),
                    'H' => Duration::hours(n),
                    'M' => Duration::minutes(n),
                    'S' => Duration::seconds(n),
                    _ => return None,
                };
            }
        }
    }
    Some(if negative { -total } else { total })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    fn cal(events: &str) -> String {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{events}END:VCALENDAR\r\n")
    }

    #[test]
    fn simple_event_with_teams_link_and_alarm() {
        let ics = cal(concat!(
            "BEGIN:VEVENT\r\n",
            "UID:1\r\n",
            "SUMMARY:Point d\\, équipe\r\n",
            "DTSTART:20261001T090000Z\r\n",
            "DTEND:20261001T093000Z\r\n",
            "DESCRIPTION:Rejoindre : https://teams.microsoft.com/l/meetup-j\r\n",
            " oin/abc\\nMerci\r\n",
            "BEGIN:VALARM\r\nSUMMARY:pas moi\r\nEND:VALARM\r\n",
            "END:VEVENT\r\n"
        ));
        let ev = events_between(
            &ics,
            utc("2026-10-01T00:00:00Z"),
            utc("2026-10-02T00:00:00Z"),
        );
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].title, "Point d, équipe");
        assert_eq!(ev[0].end - ev[0].start, Duration::minutes(30));
        assert_eq!(
            ev[0].join_url.as_deref(),
            Some("https://teams.microsoft.com/l/meetup-join/abc")
        );
    }

    #[test]
    fn windows_timezone_from_outlook() {
        let ics = cal(concat!(
            "BEGIN:VEVENT\r\nUID:2\r\nSUMMARY:Outlook\r\n",
            "DTSTART;TZID=Romance Standard Time:20261001T100000\r\n",
            "DURATION:PT1H\r\nEND:VEVENT\r\n"
        ));
        let ev = events_between(
            &ics,
            utc("2026-10-01T00:00:00Z"),
            utc("2026-10-02T00:00:00Z"),
        );
        // Paris en octobre = UTC+2.
        assert_eq!(ev[0].start, utc("2026-10-01T08:00:00Z"));
        assert_eq!(ev[0].end, utc("2026-10-01T09:00:00Z"));
    }

    #[test]
    fn weekly_series_with_exdate_override_and_dst() {
        let ics = cal(concat!(
            "BEGIN:VEVENT\r\nUID:s\r\nSUMMARY:Daily\r\n",
            "DTSTART;TZID=Europe/Paris:20261019T093000\r\n",
            "DTEND;TZID=Europe/Paris:20261019T094500\r\n",
            "RRULE:FREQ=WEEKLY;BYDAY=MO;COUNT=4\r\n",
            "EXDATE;TZID=Europe/Paris:20261026T093000\r\n",
            "END:VEVENT\r\n",
            // L'occurrence du 2 novembre est déplacée à 11h.
            "BEGIN:VEVENT\r\nUID:s\r\nSUMMARY:Daily (déplacé)\r\n",
            "RECURRENCE-ID;TZID=Europe/Paris:20261102T093000\r\n",
            "DTSTART;TZID=Europe/Paris:20261102T110000\r\n",
            "DTEND;TZID=Europe/Paris:20261102T111500\r\n",
            "END:VEVENT\r\n"
        ));
        let ev = events_between(
            &ics,
            utc("2026-10-01T00:00:00Z"),
            utc("2026-12-01T00:00:00Z"),
        );
        let got: Vec<(String, DateTime<Utc>)> =
            ev.iter().map(|e| (e.title.clone(), e.start)).collect();
        assert_eq!(
            got,
            vec![
                ("Daily".into(), utc("2026-10-19T07:30:00Z")),
                // 26/10 exclue ; 2/11 remplacée (heure d'hiver : UTC+1).
                ("Daily (déplacé)".into(), utc("2026-11-02T10:00:00Z")),
                ("Daily".into(), utc("2026-11-09T08:30:00Z")),
            ]
        );
    }

    #[test]
    fn cancelled_and_out_of_window_events_are_skipped() {
        let ics = cal(concat!(
            "BEGIN:VEVENT\r\nUID:c\r\nSUMMARY:Annulé\r\nSTATUS:CANCELLED\r\n",
            "DTSTART:20261001T090000Z\r\nEND:VEVENT\r\n",
            "BEGIN:VEVENT\r\nUID:o\r\nSUMMARY:Hier\r\n",
            "DTSTART:20260930T090000Z\r\nDTEND:20260930T100000Z\r\nEND:VEVENT\r\n"
        ));
        assert!(
            events_between(
                &ics,
                utc("2026-10-01T00:00:00Z"),
                utc("2026-10-02T00:00:00Z")
            )
            .is_empty()
        );
    }

    #[test]
    fn all_day_and_ongoing_events() {
        let ics = cal(concat!(
            "BEGIN:VEVENT\r\nUID:a\r\nSUMMARY:Congé\r\n",
            "DTSTART;VALUE=DATE:20261001\r\nDTEND;VALUE=DATE:20261002\r\nEND:VEVENT\r\n",
            "BEGIN:VEVENT\r\nUID:b\r\nSUMMARY:Atelier\r\n",
            "DTSTART:20261001T080000Z\r\nDTEND:20261001T120000Z\r\nEND:VEVENT\r\n"
        ));
        // Fenêtre qui commence pendant l'atelier : il est toujours là.
        let ev = events_between(
            &ics,
            utc("2026-10-01T10:00:00Z"),
            utc("2026-10-01T23:00:00Z"),
        );
        assert!(ev.iter().any(|e| e.title == "Atelier"));
        let conge = ev.iter().find(|e| e.title == "Congé").unwrap();
        assert!(conge.all_day);
        assert_eq!(conge.end - conge.start, Duration::days(1));
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("PT1H30M"), Some(Duration::minutes(90)));
        assert_eq!(parse_duration("P1D"), Some(Duration::days(1)));
        assert_eq!(parse_duration("P1W"), Some(Duration::weeks(1)));
        assert_eq!(parse_duration("-PT15M"), Some(Duration::minutes(-15)));
        assert_eq!(parse_duration("1H"), None);
    }

    #[test]
    fn garbage_does_not_panic() {
        for junk in [
            "",
            "BEGIN:VEVENT",
            "END:VEVENT\nBEGIN:VEVENT\nDTSTART:x",
            "\u{feff}::::",
        ] {
            let _ = events_between(junk, Utc::now(), Utc::now() + Duration::days(1));
        }
    }
}

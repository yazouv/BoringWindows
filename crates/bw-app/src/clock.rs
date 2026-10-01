//! Horloge de l'état ouvert : mise à jour une fois par minute, alignée.

use std::time::Duration;

use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};

const DAYS: [&str; 7] = [
    "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche",
];
const DAYS_EN: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const MONTHS_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const MONTHS: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

pub struct ClockText {
    pub time: String,
    pub date: String,
}

pub fn now() -> (ClockText, Duration) {
    let now = Local::now();
    (format(&now), until_next_minute(&now))
}

pub fn format<Tz: TimeZone>(t: &DateTime<Tz>) -> ClockText {
    let (weekday, month) = (
        t.weekday().num_days_from_monday() as usize,
        t.month0() as usize,
    );
    let date = match bw_i18n::lang() {
        bw_i18n::Lang::Fr => format!("{} {} {}", DAYS[weekday], t.day(), MONTHS[month]),
        bw_i18n::Lang::En => format!("{}, {} {}", DAYS_EN[weekday], MONTHS_EN[month], t.day()),
    };
    ClockText {
        time: format!("{:02}:{:02}", t.hour(), t.minute()),
        date,
    }
}

/// Délai jusqu'à la prochaine minute pile (+ une petite marge).
pub fn until_next_minute<Tz: TimeZone>(t: &DateTime<Tz>) -> Duration {
    let elapsed_ms = u64::from(t.second()) * 1000 + u64::from(t.timestamp_subsec_millis().min(999));
    Duration::from_millis(60_000 - elapsed_ms + 20)
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;

    use super::*;

    #[test]
    fn formats_in_french() {
        bw_i18n::set(bw_i18n::Lang::Fr);
        let t = FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2026, 10, 1, 9, 5, 42)
            .unwrap();
        let c = format(&t);
        assert_eq!(c.time, "09:05");
        assert_eq!(c.date, "jeudi 1 octobre");
        assert_eq!(until_next_minute(&t), Duration::from_millis(18_020));
    }
}

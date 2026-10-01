//! Fuseaux horaires des fichiers ICS : noms IANA (Google, iCloud), noms
//! Windows (Outlook : « Romance Standard Time »…) ou variantes préfixées.

use std::str::FromStr;

use chrono_tz::Tz;

/// Noms Windows les plus courants → IANA.
const WINDOWS_ZONES: &[(&str, &str)] = &[
    ("Romance Standard Time", "Europe/Paris"),
    ("W. Europe Standard Time", "Europe/Berlin"),
    ("Central Europe Standard Time", "Europe/Budapest"),
    ("Central European Standard Time", "Europe/Warsaw"),
    ("GMT Standard Time", "Europe/London"),
    ("Greenwich Standard Time", "Atlantic/Reykjavik"),
    ("E. Europe Standard Time", "Europe/Chisinau"),
    ("FLE Standard Time", "Europe/Kiev"),
    ("GTB Standard Time", "Europe/Bucharest"),
    ("Turkey Standard Time", "Europe/Istanbul"),
    ("Russian Standard Time", "Europe/Moscow"),
    ("Morocco Standard Time", "Africa/Casablanca"),
    ("Egypt Standard Time", "Africa/Cairo"),
    ("South Africa Standard Time", "Africa/Johannesburg"),
    ("Israel Standard Time", "Asia/Jerusalem"),
    ("Arabian Standard Time", "Asia/Dubai"),
    ("India Standard Time", "Asia/Kolkata"),
    ("Singapore Standard Time", "Asia/Singapore"),
    ("China Standard Time", "Asia/Shanghai"),
    ("Tokyo Standard Time", "Asia/Tokyo"),
    ("Korea Standard Time", "Asia/Seoul"),
    ("AUS Eastern Standard Time", "Australia/Sydney"),
    ("New Zealand Standard Time", "Pacific/Auckland"),
    ("Eastern Standard Time", "America/New_York"),
    ("Central Standard Time", "America/Chicago"),
    ("Mountain Standard Time", "America/Denver"),
    ("US Mountain Standard Time", "America/Phoenix"),
    ("Pacific Standard Time", "America/Los_Angeles"),
    ("Alaskan Standard Time", "America/Anchorage"),
    ("Hawaiian Standard Time", "Pacific/Honolulu"),
    ("Atlantic Standard Time", "America/Halifax"),
    ("Canada Central Standard Time", "America/Regina"),
    ("Central America Standard Time", "America/Guatemala"),
    ("SA Pacific Standard Time", "America/Bogota"),
    ("Pacific SA Standard Time", "America/Santiago"),
    ("Argentina Standard Time", "America/Buenos_Aires"),
    ("E. South America Standard Time", "America/Sao_Paulo"),
    ("UTC", "UTC"),
    ("Coordinated Universal Time", "UTC"),
];

/// Résout un TZID d'ICS en fuseau IANA, ou `None` s'il est inconnu.
pub fn resolve(tzid: &str) -> Option<Tz> {
    let tzid = tzid.trim().trim_matches('"');
    if let Ok(tz) = Tz::from_str(tzid) {
        return Some(tz);
    }
    if let Some((_, iana)) = WINDOWS_ZONES
        .iter()
        .find(|(win, _)| win.eq_ignore_ascii_case(tzid))
    {
        return Tz::from_str(iana).ok();
    }
    // « /mozilla.org/20050126_1/Europe/Paris », « (UTC+01:00) Europe/Paris »…
    let parts: Vec<&str> = tzid.split(['/', ' ']).collect();
    parts
        .windows(2)
        .rev()
        .find_map(|w| Tz::from_str(&format!("{}/{}", w[0], w[1])).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_common_forms() {
        assert_eq!(resolve("Europe/Paris"), Some(Tz::Europe__Paris));
        assert_eq!(resolve("\"Europe/Paris\""), Some(Tz::Europe__Paris));
        assert_eq!(resolve("Romance Standard Time"), Some(Tz::Europe__Paris));
        assert_eq!(
            resolve("pacific standard time"),
            Some(Tz::America__Los_Angeles)
        );
        assert_eq!(
            resolve("/mozilla.org/20050126_1/Europe/Paris"),
            Some(Tz::Europe__Paris)
        );
        assert_eq!(resolve("UTC"), Some(Tz::UTC));
        assert_eq!(resolve("Customized Time Zone"), None);
    }
}

//! Test d'un lien avant de l'enregistrer (assistant « Ajouter un calendrier »).

use bw_i18n::{Lang, tr};
use chrono::{Duration, Local, Utc};

use crate::config::{Source, SourceKind};
use crate::fetch::fetch_source;
use crate::ics::events_between;

/// Résultat lisible du test d'une source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    /// Événements dans les 30 prochains jours.
    pub upcoming: usize,
    /// Prochain événement : « R5A.07 - … · ven. 2 oct. 10:00 ».
    pub next: Option<String>,
}

/// Teste un lien ICS ou un fichier.
pub fn probe(url: &str) -> anyhow::Result<Probe> {
    probe_source(&Source {
        name: String::new(),
        kind: SourceKind::Ics,
        url: url.to_owned(),
        username: String::new(),
        password: String::new(),
    })
}

/// Télécharge et lit la source (bloquant : à appeler hors du thread UI).
pub fn probe_source(source: &Source) -> anyhow::Result<Probe> {
    let now = Utc::now();
    let text = fetch_source(source, now - Duration::days(1), now + Duration::days(31))?;
    anyhow::ensure!(
        text.contains("BEGIN:VCALENDAR"),
        tr!(
            "this link doesn't return an ICS calendar (web page? HTML link?)",
            "ce lien ne renvoie pas un calendrier ICS (page web ? lien HTML ?)"
        )
    );
    let now = Utc::now();
    let events = events_between(&text, now, now + Duration::days(30));
    let next = events
        .iter()
        .find(|e| e.start >= now || e.end > now)
        .map(|e| {
            let when = e.start.with_timezone(&Local);
            let format = match (bw_i18n::lang(), e.all_day) {
                (Lang::Fr, true) => "%d/%m",
                (Lang::Fr, false) => "%d/%m %H:%M",
                (Lang::En, true) => "%b %d",
                (Lang::En, false) => "%b %d %H:%M",
            };
            format!("{} · {}", e.title, when.format(format))
        });
    Ok(Probe {
        upcoming: events.len(),
        next,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probes_local_file_and_rejects_html() {
        let dir = std::env::temp_dir().join(format!("bw-probe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let start = (Utc::now() + Duration::days(1)).format("%Y%m%dT090000Z");
        let ics = dir.join("a.ics");
        std::fs::write(
            &ics,
            format!("BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:1\nSUMMARY:Cours\nDTSTART:{start}\nDURATION:PT1H\nEND:VEVENT\nEND:VCALENDAR\n"),
        )
        .unwrap();
        let p = probe(ics.to_str().unwrap()).unwrap();
        assert_eq!(p.upcoming, 1);
        assert!(p.next.unwrap().starts_with("Cours · "));

        let html = dir.join("b.ics");
        std::fs::write(&html, "<html>connexion requise</html>").unwrap();
        assert!(probe(html.to_str().unwrap()).is_err());
        assert!(probe("C:/n/existe/pas.ics").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}

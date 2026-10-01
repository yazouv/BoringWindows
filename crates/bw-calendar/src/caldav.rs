//! CalDAV (iCloud, Fastmail, Nextcloud…) : découverte des agendas, puis un
//! REPORT sur la période voulue. Le résultat est fusionné en un seul texte
//! ICS, lu ensuite comme n'importe quelle autre source.

use anyhow::{Context, bail};
use chrono::{DateTime, Utc};
use roxmltree::{Document, Node};

const DAV: &str = "DAV:";
const CALDAV: &str = "urn:ietf:params:xml:ns:caldav";

const PROPS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
  <d:prop><d:resourcetype/><d:displayname/><d:current-user-principal/><c:calendar-home-set/></d:prop>
</d:propfind>"#;

pub struct Account<'a> {
    pub url: &'a str,
    pub username: &'a str,
    pub password: &'a str,
}

/// Télécharge les événements entre `from` et `to` de tous les agendas du compte.
pub fn fetch(account: &Account, from: DateTime<Utc>, to: DateTime<Utc>) -> anyhow::Result<String> {
    let auth = format!(
        "Basic {}",
        base64(format!("{}:{}", account.username, account.password).as_bytes())
    );
    let calendars = discover(account.url, &auth)?;
    if calendars.is_empty() {
        bail!(bw_i18n::tr!(
            "no calendar found on this CalDAV account",
            "aucun agenda trouvé sur ce compte CalDAV"
        ));
    }
    let start = from.format("%Y%m%dT%H%M%SZ");
    let end = to.format("%Y%m%dT%H%M%SZ");
    let query = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<c:calendar-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
  <d:prop><c:calendar-data/></d:prop>
  <c:filter><c:comp-filter name="VCALENDAR"><c:comp-filter name="VEVENT">
    <c:time-range start="{start}" end="{end}"/>
  </c:comp-filter></c:comp-filter></c:filter>
</c:calendar-query>"#
    );
    let mut bodies = Vec::new();
    for calendar in &calendars {
        let xml = dav("REPORT", calendar, &auth, "1", &query)?;
        bodies.extend(calendar_data(&xml)?);
    }
    Ok(merge(&bodies))
}

fn dav(method: &str, url: &str, auth: &str, depth: &str, body: &str) -> anyhow::Result<String> {
    let bytes = bw_net::request(
        method,
        url,
        &[("Authorization", auth), ("Depth", depth)],
        Some(("application/xml; charset=utf-8", body)),
    )?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Adresses des agendas : l'adresse donnée si c'est un agenda, sinon ceux
/// du « calendar-home-set » de l'utilisateur.
fn discover(url: &str, auth: &str) -> anyhow::Result<Vec<String>> {
    let first = dav("PROPFIND", url, auth, "0", PROPS).context("PROPFIND")?;
    let entries = parse_multistatus(&first)?;
    if entries.iter().any(|e| e.is_calendar) {
        return Ok(vec![url.to_owned()]);
    }
    let mut home = entries.iter().find_map(|e| e.home_set.clone());
    if home.is_none()
        && let Some(principal) = entries.iter().find_map(|e| e.principal.clone())
    {
        let url = resolve(url, &principal);
        let xml = dav("PROPFIND", &url, auth, "0", PROPS)?;
        home = parse_multistatus(&xml)?
            .iter()
            .find_map(|e| e.home_set.clone());
    }
    let Some(home) = home else {
        bail!(bw_i18n::tr!(
            "CalDAV: calendar location not found (check the address)",
            "CalDAV : emplacement des agendas introuvable (vérifie l'adresse)"
        ));
    };
    let home = resolve(url, &home);
    let xml = dav("PROPFIND", &home, auth, "1", PROPS)?;
    Ok(parse_multistatus(&xml)?
        .into_iter()
        .filter(|e| e.is_calendar)
        .map(|e| resolve(&home, &e.href))
        .collect())
}

#[derive(Default)]
struct Entry {
    href: String,
    is_calendar: bool,
    principal: Option<String>,
    home_set: Option<String>,
}

fn named<'a>(node: &Node<'a, 'a>, ns: &str, name: &str) -> bool {
    node.is_element() && node.tag_name().name() == name && node.tag_name().namespace() == Some(ns)
}

fn child<'a>(node: Node<'a, 'a>, ns: &str, name: &str) -> Option<Node<'a, 'a>> {
    node.children().find(|c| named(c, ns, name))
}

fn href_of(node: Option<Node>) -> Option<String> {
    let href = child(node?, DAV, "href")?;
    Some(href.text()?.trim().to_owned())
}

fn parse_multistatus(xml: &str) -> anyhow::Result<Vec<Entry>> {
    let doc = Document::parse(xml).context("réponse CalDAV illisible")?;
    let mut out = Vec::new();
    for response in doc.descendants().filter(|n| named(n, DAV, "response")) {
        let mut entry = Entry {
            href: child(response, DAV, "href")
                .and_then(|h| h.text())
                .unwrap_or_default()
                .trim()
                .to_owned(),
            ..Entry::default()
        };
        for prop in response.descendants().filter(|n| named(n, DAV, "prop")) {
            if let Some(rt) = child(prop, DAV, "resourcetype") {
                entry.is_calendar |= child(rt, CALDAV, "calendar").is_some();
            }
            entry.principal = entry
                .principal
                .or_else(|| href_of(child(prop, DAV, "current-user-principal")));
            entry.home_set = entry
                .home_set
                .or_else(|| href_of(child(prop, CALDAV, "calendar-home-set")));
        }
        out.push(entry);
    }
    Ok(out)
}

fn calendar_data(xml: &str) -> anyhow::Result<Vec<String>> {
    let doc = Document::parse(xml).context("réponse CalDAV illisible")?;
    Ok(doc
        .descendants()
        .filter(|n| named(n, CALDAV, "calendar-data"))
        .filter_map(|n| n.text().map(str::to_owned))
        .collect())
}

/// Un seul VCALENDAR qui contient le corps de tous les autres.
fn merge(bodies: &[String]) -> String {
    let mut out = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n");
    for body in bodies {
        let lines: Vec<&str> = body.lines().collect();
        let begin = lines.iter().position(|l| l.trim() == "BEGIN:VCALENDAR");
        let end = lines.iter().rposition(|l| l.trim() == "END:VCALENDAR");
        if let (Some(b), Some(e)) = (begin, end)
            && b < e
        {
            for line in &lines[b + 1..e] {
                if !line.starts_with("VERSION:") && !line.starts_with("PRODID:") {
                    out.push_str(line);
                    out.push_str("\r\n");
                }
            }
        }
    }
    out.push_str("END:VCALENDAR\r\n");
    out
}

/// `href` relatif (`/dav/x/`) ou absolu, rapporté à `base`.
fn resolve(base: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_owned();
    }
    let (scheme, rest) = base.split_once("://").unwrap_or(("https", base));
    let host = rest.split('/').next().unwrap_or(rest);
    if href.starts_with('/') {
        format!("{scheme}://{host}{href}")
    } else {
        let dir = base.rsplit_once('/').map_or(base, |(d, _)| d);
        format!("{dir}/{href}")
    }
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn base64_matches_rfc() {
        assert_eq!(
            base64(b"Aladdin:open sesame"),
            "QWxhZGRpbjpvcGVuIHNlc2FtZQ=="
        );
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(base64(b""), "");
    }

    #[test]
    fn resolves_hrefs() {
        assert_eq!(resolve("https://h/a/b/", "/x/y/"), "https://h/x/y/");
        assert_eq!(resolve("https://h/a/b", "c/"), "https://h/a/c/");
        assert_eq!(resolve("https://h/a", "https://o/z"), "https://o/z");
    }

    #[test]
    fn merges_calendars() {
        let a = "BEGIN:VCALENDAR\nVERSION:2.0\nPRODID:x\nBEGIN:VEVENT\nUID:1\nEND:VEVENT\nEND:VCALENDAR".to_owned();
        let b = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:2\nEND:VEVENT\nEND:VCALENDAR".to_owned();
        let m = merge(&[a, b]);
        assert_eq!(m.matches("BEGIN:VCALENDAR").count(), 1);
        assert!(m.contains("UID:1") && m.contains("UID:2"));
        assert!(!m.contains("PRODID"));
    }

    const REPORT: &str = r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:response><d:href>/cal/perso/a.ics</d:href><d:propstat><d:prop><c:calendar-data>BEGIN:VCALENDAR
BEGIN:VEVENT
UID:abc
SUMMARY:Dentiste
DTSTART:20300101T090000Z
DURATION:PT1H
END:VEVENT
END:VCALENDAR</c:calendar-data></d:prop></d:propstat></d:response></d:multistatus>"#;

    const HOME: &str = r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:response><d:href>/cal/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat></d:response><d:response><d:href>/cal/perso/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/><c:calendar/></d:resourcetype></d:prop></d:propstat></d:response><d:response><d:href>/cal/inbox/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat></d:response></d:multistatus>"#;

    const PRINCIPAL: &str = r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:response><d:href>/principal/</d:href><d:propstat><d:prop><c:calendar-home-set><d:href>/cal/</d:href></c:calendar-home-set></d:prop></d:propstat></d:response></d:multistatus>"#;

    const ROOT: &str = r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>/</d:href><d:propstat><d:prop><d:current-user-principal><d:href>/principal/</d:href></d:current-user-principal></d:prop></d:propstat></d:response></d:multistatus>"#;

    /// Lit une requête complète : en-têtes, puis corps (longueur ou « chunked »).
    fn read_request(s: &mut std::net::TcpStream) -> String {
        let mut data = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = s.read(&mut chunk).unwrap();
            data.extend_from_slice(&chunk[..n]);
            let text = String::from_utf8_lossy(&data).to_string();
            if let Some(split) = text.find("\r\n\r\n") {
                let head = text[..split].to_lowercase();
                let body = &text[split + 4..];
                let complete = if let Some(len) =
                    head.lines().find_map(|l| l.strip_prefix("content-length:"))
                {
                    body.len() >= len.trim().parse::<usize>().unwrap()
                } else if head.contains("transfer-encoding: chunked") {
                    body.ends_with("0\r\n\r\n")
                } else {
                    true
                };
                if complete || n == 0 {
                    return text;
                }
            }
            assert!(n > 0, "connexion fermée trop tôt");
        }
    }

    /// Serveur CalDAV minimal : racine → principal → home-set → agenda → REPORT.
    fn serve(listener: TcpListener) {
        for stream in listener.incoming().take(4) {
            let mut s = stream.unwrap();
            let req = read_request(&mut s);
            let line = req.lines().next().unwrap().to_owned();
            assert!(
                req.to_lowercase().contains("authorization: basic "),
                "{req}"
            );
            let body = if line.starts_with("REPORT") {
                REPORT
            } else if line.contains(" /cal/ ") {
                HOME
            } else if line.contains("/principal/") {
                PRINCIPAL
            } else {
                ROOT
            };
            write!(
                s,
                "HTTP/1.1 207 Multi-Status\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    }

    #[test]
    fn discovers_and_fetches_from_a_server() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || serve(listener));
        let url = format!("http://127.0.0.1:{port}/");
        let ics = fetch(
            &Account {
                url: &url,
                username: "u",
                password: "p",
            },
            Utc::now(),
            Utc::now() + chrono::Duration::days(1),
        )
        .unwrap();
        server.join().unwrap();
        assert!(ics.contains("SUMMARY:Dentiste"), "{ics}");
        assert_eq!(ics.matches("BEGIN:VCALENDAR").count(), 1);
    }
}

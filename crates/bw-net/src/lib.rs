//! Requêtes HTTP GET (bloquantes : à appeler hors du thread UI).
//!
//! Sous Windows, le client HTTP du système : proxy et certificats de
//! l'entreprise sont pris en compte sans réglage. Ailleurs, `curl`.
//! Les redirections sont suivies ; l'en-tête `Authorization` n'est jamais
//! renvoyé à un autre hôte (téléchargements GitHub redirigés vers un CDN).

/// En-têtes supplémentaires : (nom, valeur).
pub type Headers<'a> = &'a [(&'a str, &'a str)];

/// Corps de la réponse ; erreur si le statut n'est pas 2xx.
pub fn get(url: &str, headers: Headers) -> anyhow::Result<Vec<u8>> {
    imp::request("GET", url, headers, None)
}

/// Requête avec méthode et corps (PROPFIND, REPORT…) ; corps de la réponse.
pub fn request(
    method: &str,
    url: &str,
    headers: Headers,
    body: Option<(&str, &str)>,
) -> anyhow::Result<Vec<u8>> {
    imp::request(method, url, headers, body)
}

/// Corps de la réponse en texte (UTF-8, caractères invalides remplacés).
pub fn get_text(url: &str, headers: Headers) -> anyhow::Result<String> {
    Ok(String::from_utf8_lossy(&get(url, headers)?).into_owned())
}

/// Hôte d'une URL (`https://hôte:port/…` → `hôte:port`), pour les redirections.
#[cfg_attr(not(windows), allow(dead_code))]
fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', '?', '#']).next().unwrap_or_default()
}

#[cfg(windows)]
mod imp {
    use windows::Foundation::Uri;
    use windows::Storage::Streams::{DataReader, UnicodeEncoding};
    use windows::Web::Http::Filters::{HttpBaseProtocolFilter, HttpCacheReadBehavior};
    use windows::Web::Http::{HttpClient, HttpMethod, HttpRequestMessage, HttpStringContent};
    use windows::core::HSTRING;

    use super::{Headers, host};

    pub fn request(
        method: &str,
        url: &str,
        headers: Headers,
        body: Option<(&str, &str)>,
    ) -> anyhow::Result<Vec<u8>> {
        let filter = HttpBaseProtocolFilter::new()?;
        // Redirections suivies à la main, pour ne pas envoyer un jeton ailleurs.
        filter.SetAllowAutoRedirect(false)?;
        filter
            .CacheControl()?
            .SetReadBehavior(HttpCacheReadBehavior::MostRecent)?;
        let client = HttpClient::Create(&filter)?;

        let mut uri = Uri::CreateUri(&HSTRING::from(url))?;
        let mut headers = headers.to_vec();
        for _ in 0..8 {
            let http_method = HttpMethod::Create(&HSTRING::from(method))?;
            let request = HttpRequestMessage::Create(&http_method, &uri)?;
            if let Some((content_type, text)) = body {
                let content = HttpStringContent::CreateFromStringWithEncodingAndMediaType(
                    &HSTRING::from(text),
                    UnicodeEncoding::Utf8,
                    &HSTRING::from(content_type),
                )?;
                request.SetContent(&content)?;
            }
            for (name, value) in &headers {
                request
                    .Headers()?
                    .TryAppendWithoutValidation(&HSTRING::from(*name), &HSTRING::from(*value))?;
            }
            let response = client.SendRequestAsync(&request)?.join()?;
            let status = response.StatusCode()?.0;
            if (300..400).contains(&status) {
                let location = response.Headers()?.Location()?;
                let next = Uri::CreateWithRelativeUri(&uri.AbsoluteUri()?, &location.ToString()?)?;
                let (from, to) = (
                    uri.AbsoluteUri()?.to_string(),
                    next.AbsoluteUri()?.to_string(),
                );
                if host(&from) != host(&to) {
                    headers.retain(|(name, _)| !name.eq_ignore_ascii_case("authorization"));
                }
                uri = next;
                continue;
            }
            anyhow::ensure!((200..300).contains(&status), "HTTP {status}");
            let buffer = response.Content()?.ReadAsBufferAsync()?.join()?;
            let mut bytes = vec![0; buffer.Length()? as usize];
            DataReader::FromBuffer(&buffer)?.ReadBytes(&mut bytes)?;
            return Ok(bytes);
        }
        anyhow::bail!("HTTP : trop de redirections")
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Headers;

    pub fn request(
        method: &str,
        url: &str,
        headers: Headers,
        body: Option<(&str, &str)>,
    ) -> anyhow::Result<Vec<u8>> {
        use std::io::Write;
        use std::process::Stdio;

        // Tout passe par l'entrée standard (`-K -`) : la ligne de commande est
        // lisible par les autres comptes (`ps`), pas le lien (jeton d'un lien
        // ICS secret) ni les en-têtes (`Authorization`).
        let config = curl_config(method, url, headers, body);
        let mut child = std::process::Command::new("curl")
            .args(["-K", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(config.as_bytes())?;
        }
        let out = child.wait_with_output()?;
        anyhow::ensure!(
            out.status.success(),
            "curl : {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
        Ok(out.stdout)
    }

    /// Fichier de configuration curl équivalent aux options de la requête.
    /// `location` ne renvoie pas `Authorization` à un autre hôte (curl ≥ 7.58).
    pub(super) fn curl_config(
        method: &str,
        url: &str,
        headers: Headers,
        body: Option<(&str, &str)>,
    ) -> String {
        let mut c = String::from("silent\nshow-error\nfail\nlocation\nmax-time = 120\n");
        c += &format!("request = {}\n", quote(method));
        for (name, value) in headers {
            c += &format!("header = {}\n", quote(&format!("{name}: {value}")));
        }
        if let Some((content_type, text)) = body {
            c += &format!(
                "header = {}\n",
                quote(&format!("Content-Type: {content_type}"))
            );
            // `data-raw` : un « @ » en tête n'est pas lu comme un nom de fichier.
            c += &format!("data-raw = {}\n", quote(text));
        }
        c += &format!("url = {}\n", quote(url));
        c
    }

    /// Chaîne entre guillemets au format des fichiers de configuration curl.
    fn quote(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 2);
        out.push('"');
        for ch in s.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts() {
        assert_eq!(host("https://api.github.com/repos/x"), "api.github.com");
        assert_eq!(host("https://h:8080?q"), "h:8080");
        assert_eq!(host("http://h"), "h");
    }

    #[cfg(not(windows))]
    #[test]
    fn curl_config_keeps_secrets_off_the_command_line() {
        let c = imp::curl_config(
            "REPORT",
            "https://h/cal?token=s3cret",
            &[("Authorization", "Basic YWxpY2U6cGFzcw==")],
            Some(("application/xml", "<a b=\"c\">\n\\</a>")),
        );
        assert!(c.contains("request = \"REPORT\"\n"));
        assert!(c.contains("header = \"Authorization: Basic YWxpY2U6cGFzcw==\"\n"));
        assert!(c.contains("data-raw = \"<a b=\\\"c\\\">\\n\\\\</a>\"\n"));
        assert!(c.ends_with("url = \"https://h/cal?token=s3cret\"\n"));
    }

    /// Requête réelle à travers curl (serveur local) : en-têtes et corps arrivent.
    #[cfg(not(windows))]
    #[test]
    fn curl_sends_headers_and_body() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            s.set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut req = Vec::new();
            let mut buf = [0; 4096];
            // En-têtes + corps (Content-Length connu ici : 9 octets).
            while !String::from_utf8_lossy(&req).contains("<x>\"y\"</x>") {
                let n = s.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                req.extend_from_slice(&buf[..n]);
            }
            s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
            String::from_utf8_lossy(&req).into_owned()
        });
        let body = request(
            "REPORT",
            &format!("http://{addr}/cal"),
            &[("Authorization", "Bearer t0k3n")],
            Some(("application/xml", "<x>\"y\"</x>")),
        )
        .unwrap();
        assert_eq!(body, b"ok");
        let req = server.join().unwrap();
        assert!(req.starts_with("REPORT /cal HTTP/1.1"), "{req}");
        assert!(req.contains("Authorization: Bearer t0k3n"), "{req}");
        assert!(req.contains("Content-Type: application/xml"), "{req}");
    }
}

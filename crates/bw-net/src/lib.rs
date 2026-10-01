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

        // `-L` ne renvoie pas `Authorization` à un autre hôte (curl ≥ 7.58).
        let mut cmd = std::process::Command::new("curl");
        cmd.args(["-fsSL", "--max-time", "120", "-X", method]);
        for (name, value) in headers {
            cmd.arg("-H").arg(format!("{name}: {value}"));
        }
        if let Some((content_type, _)) = body {
            cmd.arg("-H").arg(format!("Content-Type: {content_type}"));
            cmd.args(["--data-binary", "@-"]);
        }
        let mut child = cmd
            .arg(url)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        if let (Some(mut stdin), Some((_, text))) = (child.stdin.take(), body) {
            stdin.write_all(text.as_bytes())?;
        }
        let out = child.wait_with_output()?;
        anyhow::ensure!(
            out.status.success(),
            "curl : {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
        Ok(out.stdout)
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
}

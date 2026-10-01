//! Téléchargement d'un calendrier : client HTTP de Windows (proxy et
//! certificats du système), `curl` ailleurs, ou fichier local.

use std::path::Path;

pub fn fetch(url: &str) -> anyhow::Result<String> {
    let url = url.trim();
    // webcal:// est un alias de https:// utilisé par les liens « s'abonner ».
    let url = match url.strip_prefix("webcal://") {
        Some(rest) => format!("https://{rest}"),
        None => url.to_owned(),
    };
    if let Some(path) = url.strip_prefix("file://") {
        return Ok(std::fs::read_to_string(path)?);
    }
    if !url.starts_with("https://") && !url.starts_with("http://") {
        anyhow::ensure!(Path::new(&url).is_file(), "fichier introuvable : {url}");
        return Ok(std::fs::read_to_string(&url)?);
    }
    http_get(&url)
}

#[cfg(windows)]
fn http_get(url: &str) -> anyhow::Result<String> {
    use windows::Foundation::Uri;
    use windows::Web::Http::HttpClient;
    use windows::core::HSTRING;

    let client = HttpClient::new()?;
    let text = client
        .GetStringAsync(&Uri::CreateUri(&HSTRING::from(url))?)?
        .join()?;
    Ok(text.to_string_lossy())
}

#[cfg(not(windows))]
fn http_get(url: &str) -> anyhow::Result<String> {
    let out = std::process::Command::new("curl")
        .args(["-fsSL", "--max-time", "30", url])
        .output()?;
    anyhow::ensure!(
        out.status.success(),
        "curl : {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

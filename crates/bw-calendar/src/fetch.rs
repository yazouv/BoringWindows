//! Téléchargement d'un calendrier (voir `bw-net`) ou lecture d'un fichier local.

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
        anyhow::ensure!(
            Path::new(&url).is_file(),
            bw_i18n::tr!("file not found: {url}", "fichier introuvable : {url}")
        );
        return Ok(std::fs::read_to_string(&url)?);
    }
    bw_net::get_text(&url, &[])
}

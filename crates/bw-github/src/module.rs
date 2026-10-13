//! Module « github » : un relevé GraphQL à intervalle fixe (comme la météo : pas
//! d'événement à écouter sans serveur public), plus vite après un échec.

use std::time::{Duration, Instant};

use bw_core::{Attention, Module, ModuleCtx};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::{GithubConfig, GithubSnapshot, notices, parse_response, request_body};

pub const MODULE_ID: &str = "github";
/// Délai maximal d'une requête.
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
/// Nouvel essai après un échec réseau (au réveil, par exemple).
const RETRY_AFTER: Duration = Duration::from_secs(120);
/// Jeton refusé ou introuvable : inutile d'insister toutes les deux minutes.
const BAD_TOKEN_RETRY: Duration = Duration::from_secs(900);
const ENDPOINT: &str = "https://api.github.com/graphql";

pub struct GithubModule {
    config: GithubConfig,
    refresh: Option<UnboundedSender<()>>,
}

impl GithubModule {
    pub fn new(config: GithubConfig) -> Self {
        Self {
            config,
            refresh: None,
        }
    }
}

impl Module for GithubModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        let (tx, mut rx) = unbounded_channel::<()>();
        self.refresh = Some(tx);
        let config = self.config.clone();
        let task_ctx = ctx.clone();
        ctx.spawn(async move {
            let ctx = task_ctx;
            let mut token: Option<String> = None;
            let mut previous: Option<GithubSnapshot> = None;
            let mut notice_end: Option<Instant> = None;
            let mut next_fetch = Instant::now();
            let mut warned = false;
            loop {
                if Instant::now() >= next_fetch {
                    let wait = match fetch(&config, &mut token).await {
                        Ok(snapshot) => {
                            warned = false;
                            ctx.set_state(snapshot.clone());
                            if let Some(prev) = &previous
                                && let Some(notice) = notices(prev, &snapshot, &config)
                            {
                                log::info!("github : {}", notice.text);
                                ctx.set_attention(notice.attention, Some(notice.text));
                                notice_end = Some(Instant::now() + config.show_for());
                            }
                            previous = Some(snapshot);
                            config.refresh()
                        }
                        Err(e) => {
                            // Pas le jeton, ni une partie : seulement la raison.
                            log::warn!("github : {e:#}");
                            let rejected = token_problem(&e);
                            if rejected {
                                token = None;
                                if !warned {
                                    warned = true;
                                    ctx.set_attention(
                                        Attention::High,
                                        Some(bw_i18n::tr!(
                                            "GitHub: token missing or refused",
                                            "GitHub : jeton absent ou refusé"
                                        )),
                                    );
                                    notice_end = Some(Instant::now() + config.show_for());
                                }
                                BAD_TOKEN_RETRY
                            } else {
                                RETRY_AFTER
                            }
                        }
                    };
                    next_fetch = Instant::now() + wait;
                }
                if notice_end.is_some_and(|end| Instant::now() >= end) {
                    ctx.set_attention(Attention::None, None);
                    notice_end = None;
                }
                let next = [Some(next_fetch), notice_end]
                    .into_iter()
                    .flatten()
                    .min()
                    .unwrap_or(next_fetch);
                tokio::select! {
                    () = tokio::time::sleep(next.saturating_duration_since(Instant::now())) => {}
                    msg = rx.recv() => match msg {
                        // Relire tout de suite.
                        Some(()) => next_fetch = Instant::now(),
                        None => break,
                    },
                }
            }
        });
        Ok(())
    }

    /// Action : `refresh` (relire tout de suite).
    fn on_action(&mut self, action: &str) {
        if action == "refresh"
            && let Some(tx) = &self.refresh
        {
            let _ = tx.send(());
        }
    }
}

/// Jeton absent, introuvable ou refusé par GitHub (HTTP 401).
fn token_problem(error: &anyhow::Error) -> bool {
    let text = format!("{error:#}");
    text.contains("401") || text.contains("jeton")
}

async fn fetch(
    config: &GithubConfig,
    token: &mut Option<String>,
) -> anyhow::Result<GithubSnapshot> {
    if token.is_none() {
        let config = config.clone();
        *token = Some(
            tokio::task::spawn_blocking(move || resolve_token(&config))
                .await
                .map_err(|e| anyhow::anyhow!("jeton : {e}"))??,
        );
    }
    let token = token.clone().unwrap_or_default();
    let body = tokio::time::timeout(
        FETCH_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            let auth = format!("Bearer {token}");
            bw_net::request(
                "POST",
                ENDPOINT,
                &[
                    ("Authorization", auth.as_str()),
                    ("User-Agent", "BoringWindows"),
                    ("Accept", "application/vnd.github+json"),
                    ("Accept-Encoding", "identity"),
                ],
                Some(("application/json", &request_body())),
            )
        }),
    )
    .await
    .map_err(|_| anyhow::anyhow!("délai dépassé"))??
    .map_err(|e| anyhow::anyhow!("requête : {e}"))?;
    parse_response(&String::from_utf8_lossy(&body))
}

/// Le jeton : celui de la config (`secret:<id>` ou en clair), sinon celui de la
/// session du GitHub CLI (`gh auth token`).
fn resolve_token(config: &GithubConfig) -> anyhow::Result<String> {
    let configured = config.token.trim();
    if !configured.is_empty() {
        let token = bw_secrets::resolve(configured)
            .map_err(|e| anyhow::anyhow!("jeton illisible : {e}"))?;
        anyhow::ensure!(!token.trim().is_empty(), "jeton vide");
        return Ok(token.trim().to_owned());
    }
    anyhow::ensure!(
        config.use_gh_cli,
        "pas de jeton ([modules.github] token) et GitHub CLI désactivé"
    );
    gh_token()
}

/// `gh auth token` : le jeton de la session GitHub CLI de l'utilisateur.
fn gh_token() -> anyhow::Result<String> {
    #[cfg(windows)]
    let candidates = [
        std::ffi::OsString::from("gh"),
        std::ffi::OsString::from(r"C:\Program Files\GitHub CLI\gh.exe"),
    ];
    #[cfg(not(windows))]
    let candidates = [std::ffi::OsString::from("gh")];

    let mut last = anyhow::anyhow!("jeton : GitHub CLI introuvable");
    for program in candidates {
        let mut command = std::process::Command::new(&program);
        command
            .args(["auth", "token"])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // Pas de fenêtre de console qui clignote.
            command.creation_flags(0x0800_0000);
        }
        match command.output() {
            Ok(out) if out.status.success() => {
                let token = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                if !token.is_empty() {
                    return Ok(token);
                }
                last = anyhow::anyhow!("jeton : GitHub CLI sans session (gh auth login)");
            }
            Ok(_) => last = anyhow::anyhow!("jeton : GitHub CLI sans session (gh auth login)"),
            Err(_) => {}
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Réseau et session `gh` requis : `cargo test -p bw-github -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn fetches_the_real_status() {
        let config = GithubConfig::default();
        let token = resolve_token(&config).expect("gh auth login");
        let auth = format!("Bearer {token}");
        let body = bw_net::request(
            "POST",
            ENDPOINT,
            &[
                ("Authorization", auth.as_str()),
                ("User-Agent", "BoringWindows"),
                ("Accept", "application/vnd.github+json"),
                ("Accept-Encoding", "identity"),
            ],
            Some(("application/json", &request_body())),
        )
        .unwrap();
        let snapshot = parse_response(&String::from_utf8_lossy(&body)).unwrap();
        assert!(!snapshot.login.is_empty());
        println!(
            "{} : {} PR ouvertes, {} revues demandées, ton {:?}\n{}",
            snapshot.login,
            snapshot.prs.len(),
            snapshot.reviews.len(),
            snapshot.tone(),
            snapshot.detail()
        );
    }
}

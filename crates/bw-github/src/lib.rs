//! Statut GitHub : la CI de tes pull requests ouvertes et les revues qu'on te
//! demande, en une icône dans l'en-tête de l'île ouverte et des annonces dans
//! la pilule (CI réussie ou en échec, revue demandée).
//!
//! Un seul relevé GraphQL par intervalle (`GithubConfig::refresh_secs`, deux
//! minutes par défaut) : GitHub n'a pas d'événement à écouter sans serveur
//! public. Le jeton vient du coffre de l'OS ou de la session du GitHub CLI.
//!
//! La logique (`parse_response`, `notices`, `GithubSnapshot::detail`…) est pure.

mod config;
mod module;

use bw_core::Attention;
use bw_i18n::tr;

pub use config::GithubConfig;
pub use module::{GithubModule, MODULE_ID};

/// État de la CI du dernier commit d'une pull request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ci {
    /// Pas de CI sur cette pull request.
    None,
    Running,
    Pass,
    Fail,
}

impl Ci {
    /// `statusCheckRollup.state` de GraphQL.
    fn from_rollup(state: Option<&str>) -> Self {
        match state {
            Some("SUCCESS") => Self::Pass,
            Some("FAILURE" | "ERROR") => Self::Fail,
            Some("PENDING" | "EXPECTED") => Self::Running,
            _ => Self::None,
        }
    }
}

/// Une de tes pull requests ouvertes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pr {
    /// `propriétaire/dépôt`.
    pub repo: String,
    pub number: u32,
    pub title: String,
    pub url: String,
    pub draft: bool,
    pub ci: Ci,
}

impl Pr {
    fn same_as(&self, other: &Self) -> bool {
        self.repo == other.repo && self.number == other.number
    }

    /// `dépôt#12`, sans le propriétaire.
    fn short(&self) -> String {
        let repo = self.repo.rsplit('/').next().unwrap_or(&self.repo);
        format!("{repo}#{}", self.number)
    }
}

/// Une pull request d'un autre, dont on te demande la revue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRequest {
    pub repo: String,
    pub number: u32,
    pub title: String,
    pub url: String,
}

impl ReviewRequest {
    fn same_as(&self, other: &Self) -> bool {
        self.repo == other.repo && self.number == other.number
    }

    fn short(&self) -> String {
        let repo = self.repo.rsplit('/').next().unwrap_or(&self.repo);
        format!("{repo}#{}", self.number)
    }
}

/// Ce que publie le module.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GithubSnapshot {
    pub login: String,
    pub prs: Vec<Pr>,
    pub reviews: Vec<ReviewRequest>,
}

/// Couleur de l'icône : le pire état de tes pull requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Idle,
    Running,
    Pass,
    Fail,
}

impl GithubSnapshot {
    pub fn tone(&self) -> Tone {
        let any = |ci| self.prs.iter().any(|p| p.ci == ci);
        if any(Ci::Fail) {
            Tone::Fail
        } else if any(Ci::Running) {
            Tone::Running
        } else if any(Ci::Pass) {
            Tone::Pass
        } else {
            Tone::Idle
        }
    }

    /// Un point sur l'icône : des revues t'attendent.
    pub fn has_reviews(&self) -> bool {
        !self.reviews.is_empty()
    }

    /// « 2 PR ouvertes · CI en échec : api#31 · 1 revue demandée » : détail
    /// montré au survol, dans la bulle sous l'en-tête.
    pub fn detail(&self) -> String {
        let mut parts = Vec::new();
        let count = self.prs.len();
        parts.push(match count {
            0 => tr!("No open PR", "Aucune PR ouverte"),
            1 => tr!("1 open PR", "1 PR ouverte"),
            _ => tr!("{count} open PRs", "{count} PR ouvertes"),
        });
        let failing: Vec<String> = self
            .prs
            .iter()
            .filter(|p| p.ci == Ci::Fail)
            .take(2)
            .map(Pr::short)
            .collect();
        if !failing.is_empty() {
            let list = failing.join(", ");
            parts.push(tr!("CI failing: {list}", "CI en échec : {list}"));
        } else if self.prs.iter().any(|p| p.ci == Ci::Running) {
            parts.push(tr!("CI running", "CI en cours"));
        } else if self.prs.iter().any(|p| p.ci == Ci::Pass) {
            parts.push(tr!("CI passing", "CI réussie"));
        }
        match self.reviews.len() {
            0 => {}
            1 => parts.push(tr!("1 review requested", "1 revue demandée")),
            n => parts.push(tr!("{n} reviews requested", "{n} revues demandées")),
        }
        parts.join(" · ")
    }

    /// Page à ouvrir au clic : la PR en échec, sinon la première revue
    /// demandée, sinon ta première PR, sinon tes PR sur github.com.
    pub fn open_url(&self) -> String {
        self.prs
            .iter()
            .find(|p| p.ci == Ci::Fail)
            .map(|p| p.url.clone())
            .or_else(|| self.reviews.first().map(|r| r.url.clone()))
            .or_else(|| self.prs.first().map(|p| p.url.clone()))
            .unwrap_or_else(|| "https://github.com/pulls".to_owned())
    }
}

// --- Relevé --------------------------------------------------------------------

/// Requête GraphQL : tes pull requests ouvertes avec la CI de leur dernier
/// commit, et celles dont on te demande la revue.
pub const QUERY: &str = "query { \
  viewer { login pullRequests(first: 20, states: OPEN, orderBy: {field: UPDATED_AT, direction: DESC}) { \
    nodes { number title url isDraft repository { nameWithOwner } \
      commits(last: 1) { nodes { commit { statusCheckRollup { state } } } } } } } \
  reviews: search(query: \"is:pr is:open review-requested:@me archived:false\", type: ISSUE, first: 20) { \
    nodes { ... on PullRequest { number title url repository { nameWithOwner } } } } }";

/// Corps JSON de la requête.
pub fn request_body() -> String {
    serde_json::json!({ "query": QUERY }).to_string()
}

/// Lit la réponse GraphQL. Les entrées incomplètes sont ignorées.
pub fn parse_response(body: &str) -> anyhow::Result<GithubSnapshot> {
    use serde_json::Value;

    let json: Value = serde_json::from_str(body)?;
    if let Some(errors) = json.get("errors").and_then(Value::as_array)
        && let Some(first) = errors.first()
    {
        let message = first.get("message").and_then(Value::as_str).unwrap_or("?");
        anyhow::bail!("GitHub : {message}");
    }
    let data = json
        .get("data")
        .ok_or_else(|| anyhow::anyhow!("réponse GitHub sans données"))?;
    let text = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).map(str::to_owned);
    let repo = |v: &Value| {
        v.get("repository")
            .and_then(|r| r.get("nameWithOwner"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let number = |v: &Value| {
        v.get("number")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
    };

    let viewer = data.get("viewer");
    let prs = viewer
        .and_then(|v| v.pointer("/pullRequests/nodes"))
        .and_then(Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(|n| {
                    let state = n
                        .pointer("/commits/nodes/0/commit/statusCheckRollup/state")
                        .and_then(Value::as_str);
                    Some(Pr {
                        repo: repo(n)?,
                        number: number(n)?,
                        title: text(n, "title")?,
                        url: text(n, "url")?,
                        draft: n.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
                        ci: Ci::from_rollup(state),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let reviews = data
        .pointer("/reviews/nodes")
        .and_then(Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(|n| {
                    Some(ReviewRequest {
                        repo: repo(n)?,
                        number: number(n)?,
                        title: text(n, "title")?,
                        url: text(n, "url")?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(GithubSnapshot {
        login: viewer.and_then(|v| text(v, "login")).unwrap_or_default(),
        prs,
        reviews,
    })
}

// --- Annonces ------------------------------------------------------------------

/// Annonce à faire dans la pilule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub text: String,
    pub attention: Attention,
}

/// Compare deux relevés et dit quoi annoncer, selon la config.
///
/// Le premier relevé sert de référence : rien à annoncer. Un échec de CI passe
/// avant une revue demandée, qui passe avant une CI réussie ; s'il y a
/// d'autres annonces, leur nombre est ajouté (« +2 »).
pub fn notices(
    prev: &GithubSnapshot,
    now: &GithubSnapshot,
    config: &GithubConfig,
) -> Option<Notice> {
    let mut found: Vec<(u8, String)> = Vec::new();
    if config.announce_ci {
        for pr in &now.prs {
            let Some(old) = prev.prs.iter().find(|p| p.same_as(pr)) else {
                continue;
            };
            let short = pr.short();
            match (old.ci, pr.ci) {
                (before, Ci::Fail) if before != Ci::Fail => {
                    found.push((0, tr!("CI failed · {short}", "CI en échec · {short}")));
                }
                (Ci::Running, Ci::Pass) => {
                    found.push((2, tr!("CI passed · {short}", "CI réussie · {short}")));
                }
                _ => {}
            }
        }
    }
    if config.announce_reviews {
        for review in &now.reviews {
            if !prev.reviews.iter().any(|r| r.same_as(review)) {
                let short = review.short();
                found.push((
                    1,
                    tr!("Review requested · {short}", "Revue demandée · {short}"),
                ));
            }
        }
    }
    found.sort_by_key(|(rank, _)| *rank);
    let (_, first) = found.first()?.clone();
    let more = found.len() - 1;
    Some(Notice {
        text: if more > 0 {
            format!("{first} (+{more})")
        } else {
            first
        },
        attention: Attention::High,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La langue est globale au processus : les tests qui en dépendent se
    /// passent le relais au lieu de se la disputer.
    static LANG: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn english() -> std::sync::MutexGuard<'static, ()> {
        let guard = LANG.lock().unwrap_or_else(|e| e.into_inner());
        bw_i18n::set(bw_i18n::Lang::En);
        guard
    }

    fn pr(repo: &str, number: u32, ci: Ci) -> Pr {
        Pr {
            repo: repo.to_owned(),
            number,
            title: format!("Titre {number}"),
            url: format!("https://github.com/{repo}/pull/{number}"),
            draft: false,
            ci,
        }
    }

    fn review(repo: &str, number: u32) -> ReviewRequest {
        ReviewRequest {
            repo: repo.to_owned(),
            number,
            title: format!("Revue {number}"),
            url: format!("https://github.com/{repo}/pull/{number}"),
        }
    }

    fn snapshot(prs: Vec<Pr>, reviews: Vec<ReviewRequest>) -> GithubSnapshot {
        GithubSnapshot {
            login: "yazouv".to_owned(),
            prs,
            reviews,
        }
    }

    const RESPONSE: &str = r#"{
      "data": {
        "viewer": {
          "login": "yazouv",
          "pullRequests": { "nodes": [
            { "number": 31, "title": "Réseau", "url": "https://github.com/yazouv/BoringWindows/pull/31",
              "isDraft": false, "repository": { "nameWithOwner": "yazouv/BoringWindows" },
              "commits": { "nodes": [ { "commit": { "statusCheckRollup": { "state": "FAILURE" } } } ] } },
            { "number": 32, "title": "Docs", "url": "https://github.com/yazouv/BoringWindows/pull/32",
              "isDraft": true, "repository": { "nameWithOwner": "yazouv/BoringWindows" },
              "commits": { "nodes": [ { "commit": { "statusCheckRollup": null } } ] } },
            { "title": "Sans numéro" }
          ] }
        },
        "reviews": { "nodes": [
          { "number": 7, "title": "Corrige le bug", "url": "https://github.com/acme/api/pull/7",
            "repository": { "nameWithOwner": "acme/api" } },
          {}
        ] }
      }
    }"#;

    #[test]
    fn parses_a_graphql_response() {
        let s = parse_response(RESPONSE).unwrap();
        assert_eq!(s.login, "yazouv");
        assert_eq!(s.prs.len(), 2, "l'entrée sans numéro est ignorée");
        assert_eq!(s.prs[0].ci, Ci::Fail);
        assert_eq!(s.prs[1].ci, Ci::None);
        assert!(s.prs[1].draft);
        assert_eq!(s.reviews.len(), 1);
        assert_eq!(s.reviews[0].repo, "acme/api");
        assert_eq!(s.tone(), Tone::Fail);
        assert!(s.has_reviews());
    }

    #[test]
    fn rejects_errors_and_garbage() {
        let err = parse_response(r#"{"errors":[{"message":"Bad credentials"}]}"#).unwrap_err();
        assert!(err.to_string().contains("Bad credentials"));
        assert!(parse_response("pas du json").is_err());
        assert!(parse_response("{}").is_err());
        // Un compte sans rien : relevé vide, pas une erreur.
        let empty = parse_response(r#"{"data":{"viewer":{"login":"x","pullRequests":{"nodes":[]}},"reviews":{"nodes":[]}}}"#)
            .unwrap();
        assert!(empty.prs.is_empty() && empty.reviews.is_empty());
        assert_eq!(empty.tone(), Tone::Idle);
    }

    #[test]
    fn rollup_states() {
        assert_eq!(Ci::from_rollup(Some("SUCCESS")), Ci::Pass);
        assert_eq!(Ci::from_rollup(Some("ERROR")), Ci::Fail);
        assert_eq!(Ci::from_rollup(Some("PENDING")), Ci::Running);
        assert_eq!(Ci::from_rollup(Some("EXPECTED")), Ci::Running);
        assert_eq!(Ci::from_rollup(None), Ci::None);
        assert_eq!(Ci::from_rollup(Some("???")), Ci::None);
    }

    #[test]
    fn tone_is_the_worst_state() {
        let tone = |cis: &[Ci]| {
            snapshot(
                cis.iter()
                    .enumerate()
                    .map(|(i, c)| pr("a/b", i as u32, *c))
                    .collect(),
                vec![],
            )
            .tone()
        };
        assert_eq!(tone(&[]), Tone::Idle);
        assert_eq!(tone(&[Ci::None]), Tone::Idle);
        assert_eq!(tone(&[Ci::Pass, Ci::None]), Tone::Pass);
        assert_eq!(tone(&[Ci::Pass, Ci::Running]), Tone::Running);
        assert_eq!(tone(&[Ci::Running, Ci::Fail, Ci::Pass]), Tone::Fail);
    }

    #[test]
    fn details_and_links() {
        let _lang = english();
        let none = snapshot(vec![], vec![]);
        assert_eq!(none.detail(), "No open PR");
        assert_eq!(none.open_url(), "https://github.com/pulls");

        let busy = snapshot(
            vec![
                pr("yazouv/api", 31, Ci::Fail),
                pr("yazouv/web", 4, Ci::Pass),
            ],
            vec![review("acme/lib", 7), review("acme/lib", 9)],
        );
        assert_eq!(
            busy.detail(),
            "2 open PRs · CI failing: api#31 · 2 reviews requested"
        );
        assert_eq!(busy.open_url(), "https://github.com/yazouv/api/pull/31");

        let calm = snapshot(
            vec![pr("yazouv/web", 4, Ci::Pass)],
            vec![review("acme/lib", 7)],
        );
        assert_eq!(calm.detail(), "1 open PR · CI passing · 1 review requested");
        // Pas de PR en échec : la revue qu'on attend de toi passe avant.
        assert_eq!(calm.open_url(), "https://github.com/acme/lib/pull/7");
        let mine = snapshot(vec![pr("yazouv/web", 4, Ci::Pass)], vec![]);
        assert_eq!(mine.open_url(), "https://github.com/yazouv/web/pull/4");
    }

    #[test]
    fn announces_ci_transitions() {
        let _lang = english();
        let config = GithubConfig::default();
        let running = snapshot(vec![pr("o/api", 3, Ci::Running)], vec![]);
        let passed = snapshot(vec![pr("o/api", 3, Ci::Pass)], vec![]);
        let failed = snapshot(vec![pr("o/api", 3, Ci::Fail)], vec![]);

        assert_eq!(
            notices(&running, &passed, &config).unwrap().text,
            "CI passed · api#3"
        );
        assert_eq!(
            notices(&running, &failed, &config).unwrap().text,
            "CI failed · api#3"
        );
        // Une CI qui repasse au vert sans avoir tourné ne s'annonce pas.
        let unchanged = snapshot(vec![pr("o/api", 3, Ci::None)], vec![]);
        assert_eq!(notices(&unchanged, &passed, &config), None);
        // Rien de nouveau, rien à dire.
        assert_eq!(notices(&failed, &failed, &config), None);
        // Une PR vue pour la première fois sert de référence.
        assert_eq!(notices(&snapshot(vec![], vec![]), &failed, &config), None);
    }

    #[test]
    fn announces_new_review_requests_once() {
        let _lang = english();
        let config = GithubConfig::default();
        let before = snapshot(vec![], vec![review("acme/lib", 7)]);
        let after = snapshot(vec![], vec![review("acme/lib", 7), review("acme/web", 12)]);
        let notice = notices(&before, &after, &config).unwrap();
        assert_eq!(notice.text, "Review requested · web#12");
        assert_eq!(notice.attention, Attention::High);
        assert_eq!(notices(&after, &after, &config), None);
    }

    #[test]
    fn failures_come_first_and_extras_are_counted() {
        let _lang = english();
        let config = GithubConfig::default();
        let before = snapshot(vec![pr("o/api", 3, Ci::Running)], vec![]);
        let after = snapshot(vec![pr("o/api", 3, Ci::Fail)], vec![review("acme/lib", 7)]);
        assert_eq!(
            notices(&before, &after, &config).unwrap().text,
            "CI failed · api#3 (+1)"
        );
    }

    #[test]
    fn announcements_follow_the_config() {
        let running = snapshot(vec![pr("o/api", 3, Ci::Running)], vec![]);
        let failed = snapshot(vec![pr("o/api", 3, Ci::Fail)], vec![review("a/b", 1)]);
        let no_ci = GithubConfig {
            announce_ci: false,
            ..GithubConfig::default()
        };
        assert!(notices(&running, &failed, &no_ci).is_some_and(|n| n.text.contains("#1")));
        let quiet = GithubConfig {
            announce_ci: false,
            announce_reviews: false,
            ..GithubConfig::default()
        };
        assert_eq!(notices(&running, &failed, &quiet), None);
    }

    #[test]
    fn request_body_is_valid_json() {
        let body: serde_json::Value = serde_json::from_str(&request_body()).unwrap();
        assert!(
            body["query"]
                .as_str()
                .unwrap()
                .contains("statusCheckRollup")
        );
    }
}

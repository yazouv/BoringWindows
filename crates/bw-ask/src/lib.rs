//! Une question posée à Claude ou à Gemini depuis l'île : tu tapes dans une
//! petite fenêtre sous l'île, la réponse s'affiche dans l'île ouverte.
//!
//! Un seul champ `model` choisit le fournisseur et le modèle (`claude-…` ou
//! `gemini-…`) ; la clé de chaque fournisseur vient du coffre de l'OS
//! (`secret:<id>`, écrit par les réglages). Aucune conversation n'est gardée :
//! chaque question part seule.
//!
//! La logique (corps de requête, lecture de la réponse, messages d'erreur) est
//! pure ; seule `ask` fait du réseau, et bloque : à appeler hors du thread UI.

use bw_i18n::tr;
use serde::Deserialize;
use serde_json::{Value, json};

pub const MODULE_ID: &str = "ask";

/// Longueur maximale d'une question (caractères).
pub const MAX_QUESTION: usize = 4000;

const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_MODELS_URL: &str = "https://api.anthropic.com/v1/models";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const GEMINI_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Claude,
    Gemini,
}

impl Provider {
    /// Fournisseur d'un identifiant de modèle (`claude-sonnet-5-5`, `gemini-2.5-flash`).
    pub fn of_model(model: &str) -> Option<Self> {
        let model = model.trim();
        if model.starts_with("claude") {
            Some(Self::Claude)
        } else if model.starts_with("gemini") {
            Some(Self::Gemini)
        } else {
            None
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::Gemini => "Gemini",
        }
    }
}

/// Un modèle proposé dans la fenêtre de saisie et dans les réglages. `model`
/// accepte aussi n'importe quel autre identifiant écrit à la main dans config.toml.
#[derive(Debug, Clone, Copy)]
pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
}

pub const PRESETS: [Preset; 7] = [
    Preset {
        id: "claude-haiku-5-5",
        label: "Claude Haiku 5.5",
    },
    Preset {
        id: "claude-sonnet-5-5",
        label: "Claude Sonnet 5.5",
    },
    Preset {
        id: "claude-opus-5-5",
        label: "Claude Opus 5.5",
    },
    Preset {
        id: "gemini-2.5-flash-lite",
        label: "Gemini 2.5 Flash-Lite",
    },
    Preset {
        id: "gemini-2.5-flash",
        label: "Gemini 2.5 Flash",
    },
    Preset {
        id: "gemini-2.5-pro",
        label: "Gemini 2.5 Pro",
    },
    Preset {
        id: "gemini-3.1-flash-lite",
        label: "Gemini 3.1 Flash-Lite",
    },
];

/// Rang d'un modèle dans `PRESETS` ; `None` pour un identifiant écrit à la main.
pub fn preset_index(model: &str) -> Option<usize> {
    PRESETS.iter().position(|p| p.id == model.trim())
}

/// Nom lisible d'un modèle : celui du préréglage, sinon l'identifiant.
pub fn label(model: &str) -> String {
    preset_index(model).map_or_else(|| model.trim().to_owned(), |i| PRESETS[i].label.to_owned())
}

/// Un modèle de la liste déroulante : l'identifiant écrit dans la config, et son nom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
}

/// Modèles à proposer. Pour chaque fournisseur, ceux qu'il annonce pour la clé
/// (`remote`, voir `list_models`), sinon les préréglages : un modèle retiré ou
/// pas encore ouvert à ce compte ferait répondre « modèle inconnu ».
pub fn model_list(remote: &[ModelInfo]) -> Vec<ModelInfo> {
    let mut out = Vec::new();
    for provider in [Provider::Claude, Provider::Gemini] {
        let own: Vec<&ModelInfo> = remote
            .iter()
            .filter(|m| Provider::of_model(&m.id) == Some(provider))
            .collect();
        if own.is_empty() {
            out.extend(
                PRESETS
                    .iter()
                    .filter(|p| Provider::of_model(p.id) == Some(provider))
                    .map(|p| ModelInfo {
                        id: p.id.to_owned(),
                        label: p.label.to_owned(),
                    }),
            );
        } else {
            out.extend(own.into_iter().cloned());
        }
    }
    out
}

/// Mots d'un identifiant Gemini qui n'est pas un modèle de texte.
const NOT_TEXT: [&str; 7] = [
    "embedding",
    "tts",
    "image",
    "live",
    "audio",
    "robotics",
    "computer-use",
];

/// Modèles de texte d'une réponse « liste des modèles ».
pub fn parse_models(provider: Provider, body: &str) -> anyhow::Result<Vec<ModelInfo>> {
    let json: Value = serde_json::from_str(body)?;
    if let Some(message) = json.pointer("/error/message").and_then(Value::as_str) {
        anyhow::bail!("{message}");
    }
    let mut models = Vec::new();
    match provider {
        Provider::Claude => {
            for m in json
                .get("data")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(id) = m.get("id").and_then(Value::as_str) else {
                    continue;
                };
                if Provider::of_model(id) == Some(provider) {
                    let label = m.get("display_name").and_then(Value::as_str).unwrap_or(id);
                    models.push(ModelInfo {
                        id: id.to_owned(),
                        label: label.to_owned(),
                    });
                }
            }
        }
        Provider::Gemini => {
            for m in json
                .get("models")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(id) = m
                    .get("name")
                    .and_then(Value::as_str)
                    .map(|n| n.trim_start_matches("models/"))
                else {
                    continue;
                };
                let generates = m
                    .get("supportedGenerationMethods")
                    .and_then(Value::as_array)
                    .is_some_and(|methods| methods.iter().any(|v| v == "generateContent"));
                if generates
                    && Provider::of_model(id) == Some(provider)
                    && !NOT_TEXT.iter().any(|word| id.contains(word))
                {
                    let label = m.get("displayName").and_then(Value::as_str).unwrap_or(id);
                    models.push(ModelInfo {
                        id: id.to_owned(),
                        label: label.to_owned(),
                    });
                }
            }
            models.sort_by(|a, b| a.id.cmp(&b.id));
        }
    }
    Ok(models)
}

/// Modèles que le fournisseur ouvre à cette clé. Bloque : hors du thread UI.
pub fn list_models(config: &AskConfig, provider: Provider) -> Result<Vec<ModelInfo>, String> {
    let key = resolve_key(config, provider)?;
    let (url, key_header) = match provider {
        Provider::Claude => (format!("{ANTHROPIC_MODELS_URL}?limit=100"), "x-api-key"),
        Provider::Gemini => (format!("{GEMINI_URL}?pageSize=200"), "x-goog-api-key"),
    };
    let mut headers: Vec<(&str, &str)> =
        vec![(key_header, key.as_str()), ("Accept-Encoding", "identity")];
    if provider == Provider::Claude {
        headers.push(("anthropic-version", ANTHROPIC_VERSION));
    }
    let body = bw_net::get(&url, &headers).map_err(|e| friendly_error(provider, &e))?;
    parse_models(provider, &String::from_utf8_lossy(&body))
        .map_err(|e| format!("{} : {e:#}", provider.name()))
}

const DEFAULT_SYSTEM: &str = "Answer briefly and directly, in a few sentences, in the language \
of the question. Plain text only: no markdown, no lists unless asked.";

/// Section `[modules.ask]` de config.toml.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AskConfig {
    pub enabled: bool,
    /// Modèle : `claude-…` ou `gemini-…`.
    pub model: String,
    /// Clé Anthropic : `secret:<id>` (coffre de l'OS) ou la clé elle-même.
    pub claude_key: String,
    /// Clé Google AI Studio : `secret:<id>` ou la clé elle-même.
    pub gemini_key: String,
    /// Longueur maximale de la réponse (jetons).
    pub max_tokens: u32,
    /// Consigne donnée au modèle ; vide : réponses brèves, en texte brut.
    pub system: String,
}

impl Default for AskConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model: "claude-sonnet-5-5".into(),
            claude_key: String::new(),
            gemini_key: String::new(),
            max_tokens: 1024,
            system: String::new(),
        }
    }
}

impl AskConfig {
    pub fn from_table(table: Option<&toml::Table>) -> anyhow::Result<Self> {
        let config: Self = match table {
            Some(t) => toml::Value::Table(t.clone()).try_into()?,
            None => Self::default(),
        };
        anyhow::ensure!(
            Provider::of_model(&config.model).is_some(),
            tr!(
                "modules.ask.model must start with \"claude\" or \"gemini\"",
                "modules.ask.model doit commencer par « claude » ou « gemini »"
            )
        );
        anyhow::ensure!(
            (64..=8192).contains(&config.max_tokens),
            tr!(
                "modules.ask.max_tokens must be between 64 and 8192",
                "modules.ask.max_tokens doit être entre 64 et 8192"
            )
        );
        Ok(config)
    }

    /// Fournisseur du modèle choisi.
    pub fn provider(&self) -> Provider {
        Provider::of_model(&self.model).unwrap_or(Provider::Claude)
    }

    /// Une clé est enregistrée pour ce fournisseur.
    pub fn has_key(&self, provider: Provider) -> bool {
        !self.key_field(provider).trim().is_empty()
    }

    pub fn key_field(&self, provider: Provider) -> &str {
        match provider {
            Provider::Claude => &self.claude_key,
            Provider::Gemini => &self.gemini_key,
        }
    }

    fn system_prompt(&self) -> &str {
        let system = self.system.trim();
        if system.is_empty() {
            DEFAULT_SYSTEM
        } else {
            system
        }
    }
}

/// Requête HTTP prête à partir, hors clé (ajoutée à l'envoi).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub url: String,
    /// En-tête portant la clé.
    pub key_header: &'static str,
    pub headers: Vec<(&'static str, &'static str)>,
    pub body: String,
}

/// Corps et adresse de la requête pour ce modèle.
pub fn build_request(config: &AskConfig, question: &str) -> Request {
    let model = config.model.trim();
    match config.provider() {
        Provider::Claude => Request {
            url: ANTHROPIC_URL.to_owned(),
            key_header: "x-api-key",
            headers: vec![("anthropic-version", ANTHROPIC_VERSION)],
            body: json!({
                "model": model,
                "max_tokens": config.max_tokens,
                "system": config.system_prompt(),
                "messages": [{ "role": "user", "content": question }],
            })
            .to_string(),
        },
        Provider::Gemini => Request {
            url: format!("{GEMINI_URL}/{model}:generateContent"),
            // En-tête plutôt que `?key=` : la clé ne finit pas dans une URL.
            key_header: "x-goog-api-key",
            headers: Vec::new(),
            body: json!({
                "systemInstruction": { "parts": [{ "text": config.system_prompt() }] },
                "contents": [{ "role": "user", "parts": [{ "text": question }] }],
                "generationConfig": { "maxOutputTokens": config.max_tokens },
            })
            .to_string(),
        },
    }
}

/// Texte de la réponse. Une réponse vide ou coupée est une erreur lisible.
pub fn parse_response(provider: Provider, body: &str) -> anyhow::Result<String> {
    let json: Value = serde_json::from_str(body)?;
    if let Some(message) = json.pointer("/error/message").and_then(Value::as_str) {
        anyhow::bail!("{message}");
    }
    let (parts, stop, cut) = match provider {
        Provider::Claude => (
            json.get("content").and_then(Value::as_array),
            json.get("stop_reason").and_then(Value::as_str),
            "max_tokens",
        ),
        Provider::Gemini => (
            json.pointer("/candidates/0/content/parts")
                .and_then(Value::as_array),
            json.pointer("/candidates/0/finishReason")
                .and_then(Value::as_str),
            "MAX_TOKENS",
        ),
    };
    let text: String = parts
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();
    let text = text.trim();
    if text.is_empty() {
        if stop == Some(cut) {
            anyhow::bail!(tr!(
                "The answer was cut off: raise modules.ask.max_tokens",
                "Réponse coupée : augmente modules.ask.max_tokens"
            ));
        }
        if let Some(reason) = json
            .pointer("/promptFeedback/blockReason")
            .and_then(Value::as_str)
        {
            anyhow::bail!(tr!(
                "Blocked by Gemini ({reason})",
                "Bloqué par Gemini ({reason})"
            ));
        }
        anyhow::bail!(tr!("Empty answer", "Réponse vide"));
    }
    Ok(text.to_owned())
}

/// Message pour l'île à partir d'une erreur réseau (`HTTP 401`, délai…).
/// Jamais la clé, ni le corps de la requête.
pub fn friendly_error(provider: Provider, error: &anyhow::Error) -> String {
    let text = format!("{error:#}");
    let name = provider.name();
    let status = text
        .strip_prefix("HTTP ")
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|code| code.parse::<u16>().ok());
    match status {
        // Gemini répond 400 à une clé invalide.
        Some(401 | 403) => tr!("{name}: the key was refused", "{name} : clé refusée"),
        Some(400) if provider == Provider::Gemini => tr!(
            "{name}: bad request or invalid key",
            "{name} : requête invalide ou clé refusée"
        ),
        Some(404) => tr!(
            "{name}: unknown model (pick another one in the list)",
            "{name} : modèle inconnu (choisis-en un autre dans la liste)"
        ),
        Some(429) => tr!(
            "{name}: rate limit or quota reached",
            "{name} : limite ou quota atteint"
        ),
        Some(code) if code >= 500 => tr!(
            "{name} is unavailable (HTTP {code})",
            "{name} est indisponible (HTTP {code})"
        ),
        Some(code) => tr!("{name}: HTTP {code}", "{name} : HTTP {code}"),
        None => format!("{name} : {text}"),
    }
}

/// Pose la question. Bloque jusqu'à la réponse : hors du thread UI.
pub fn ask(config: &AskConfig, question: &str) -> Result<String, String> {
    let provider = config.provider();
    let question: String = question.trim().chars().take(MAX_QUESTION).collect();
    if question.is_empty() {
        return Err(tr!("Empty question", "Question vide"));
    }
    let key = resolve_key(config, provider)?;
    let request = build_request(config, &question);
    let mut headers: Vec<(&str, &str)> = request.headers.clone();
    headers.push((request.key_header, key.as_str()));
    headers.push(("Accept-Encoding", "identity"));
    let body = bw_net::request(
        "POST",
        &request.url,
        &headers,
        Some(("application/json", &request.body)),
    )
    .map_err(|e| friendly_error(provider, &e))?;
    parse_response(provider, &String::from_utf8_lossy(&body))
        .map_err(|e| format!("{} : {e:#}", provider.name()))
}

fn resolve_key(config: &AskConfig, provider: Provider) -> Result<String, String> {
    let name = provider.name();
    let field = config.key_field(provider).trim();
    if field.is_empty() {
        return Err(tr!(
            "No {name} key: add one in the settings",
            "Pas de clé {name} : ajoute-la dans les réglages"
        ));
    }
    let key = bw_secrets::resolve(field).map_err(|e| {
        // Seule la raison, jamais la valeur.
        log::warn!("ask : clé illisible : {e:#}");
        tr!("{name}: key unreadable", "{name} : clé illisible")
    })?;
    let key = key.trim().to_owned();
    if key.is_empty() {
        return Err(tr!("{name}: empty key", "{name} : clé vide"));
    }
    Ok(key)
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

    fn config(model: &str) -> AskConfig {
        AskConfig {
            model: model.to_owned(),
            ..AskConfig::default()
        }
    }

    #[test]
    fn provider_comes_from_the_model() {
        assert_eq!(
            Provider::of_model("claude-opus-5-5"),
            Some(Provider::Claude)
        );
        assert_eq!(
            Provider::of_model(" gemini-2.5-pro"),
            Some(Provider::Gemini)
        );
        assert_eq!(Provider::of_model("gpt-5"), None);
        for preset in PRESETS {
            assert!(Provider::of_model(preset.id).is_some(), "{}", preset.id);
        }
        assert_eq!(label("claude-haiku-5-5"), "Claude Haiku 5.5");
        assert_eq!(label("gemini-9-test"), "gemini-9-test");
        assert_eq!(preset_index("gemini-2.5-flash"), Some(4));
        assert_eq!(preset_index("autre"), None);
    }

    #[test]
    fn config_defaults_and_validation() {
        let _lang = english();
        let c = AskConfig::from_table(None).unwrap();
        assert!(!c.enabled);
        assert_eq!(c.provider(), Provider::Claude);
        assert!(!c.has_key(Provider::Claude));
        let parse = |text: &str| {
            let t: toml::Table = toml::from_str(text).unwrap();
            AskConfig::from_table(Some(&t))
        };
        let c = parse("model = \"gemini-2.5-flash\"\ngemini_key = \"secret:ask-1\"").unwrap();
        assert_eq!(c.provider(), Provider::Gemini);
        assert!(c.has_key(Provider::Gemini) && !c.has_key(Provider::Claude));
        for bad in [
            "model = \"gpt-5\"",
            "max_tokens = 10",
            "max_tokens = 99999",
            "fournisseur = \"x\"",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn claude_request() {
        let r = build_request(&config("claude-haiku-5-5"), "Capitale de la France ?");
        assert_eq!(r.url, "https://api.anthropic.com/v1/messages");
        assert_eq!(r.key_header, "x-api-key");
        assert_eq!(r.headers, vec![("anthropic-version", "2023-06-01")]);
        let body: Value = serde_json::from_str(&r.body).unwrap();
        assert_eq!(body["model"], "claude-haiku-5-5");
        assert_eq!(body["max_tokens"], 1024);
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "Capitale de la France ?");
        assert!(body["system"].as_str().unwrap().contains("briefly"));
    }

    #[test]
    fn gemini_request_keeps_the_key_out_of_the_url() {
        let mut c = config("gemini-2.5-flash");
        c.system = "Réponds en un mot.".into();
        let r = build_request(&c, "Salut");
        assert_eq!(
            r.url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent"
        );
        assert_eq!(r.key_header, "x-goog-api-key");
        assert!(!r.url.contains("key="));
        let body: Value = serde_json::from_str(&r.body).unwrap();
        assert_eq!(body["contents"][0]["parts"][0]["text"], "Salut");
        assert_eq!(
            body["systemInstruction"]["parts"][0]["text"],
            "Réponds en un mot."
        );
        assert_eq!(body["generationConfig"]["maxOutputTokens"], 1024);
    }

    #[test]
    fn model_lists_follow_the_provider() {
        let gemini = r#"{"models":[
            {"name":"models/gemini-3.5-flash","displayName":"Gemini 3.5 Flash","supportedGenerationMethods":["generateContent","countTokens"]},
            {"name":"models/gemini-3.5-flash-preview-tts","displayName":"TTS","supportedGenerationMethods":["generateContent"]},
            {"name":"models/gemini-embedding-001","displayName":"Embedding","supportedGenerationMethods":["embedContent"]},
            {"name":"models/gemma-3-27b","displayName":"Gemma","supportedGenerationMethods":["generateContent"]},
            {"name":"models/gemini-3.5-flash-lite","supportedGenerationMethods":["generateContent"]}
        ]}"#;
        let parsed = parse_models(Provider::Gemini, gemini).unwrap();
        let ids: Vec<&str> = parsed.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["gemini-3.5-flash", "gemini-3.5-flash-lite"]);
        assert_eq!(parsed[0].label, "Gemini 3.5 Flash");
        assert_eq!(parsed[1].label, "gemini-3.5-flash-lite");

        let claude = r#"{"data":[{"id":"claude-sonnet-5-5","display_name":"Claude Sonnet 5.5"},{"id":"autre"}]}"#;
        let parsed = parse_models(Provider::Claude, claude).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].label, "Claude Sonnet 5.5");
        assert!(parse_models(Provider::Claude, r#"{"error":{"message":"nope"}}"#).is_err());

        // Sans liste, les préréglages ; avec une liste, elle remplace ceux du fournisseur.
        let presets = model_list(&[]);
        assert_eq!(presets.len(), PRESETS.len());
        let mixed = model_list(&parse_models(Provider::Gemini, gemini).unwrap());
        assert!(mixed.iter().any(|m| m.id == "claude-haiku-5-5"));
        assert!(mixed.iter().any(|m| m.id == "gemini-3.5-flash"));
        assert!(!mixed.iter().any(|m| m.id == "gemini-2.5-flash"));
    }

    #[test]
    fn parses_claude_answers() {
        let body = r#"{"content":[{"type":"text","text":"Paris"},{"type":"text","text":"."}],"stop_reason":"end_turn"}"#;
        assert_eq!(parse_response(Provider::Claude, body).unwrap(), "Paris.");
    }

    #[test]
    fn parses_gemini_answers() {
        let body =
            r#"{"candidates":[{"content":{"parts":[{"text":" Paris "}]},"finishReason":"STOP"}]}"#;
        assert_eq!(parse_response(Provider::Gemini, body).unwrap(), "Paris");
    }

    #[test]
    fn explains_empty_and_failed_answers() {
        let _lang = english();
        let cut = r#"{"content":[],"stop_reason":"max_tokens"}"#;
        assert!(
            parse_response(Provider::Claude, cut)
                .unwrap_err()
                .to_string()
                .contains("max_tokens")
        );
        let cut = r#"{"candidates":[{"content":{"parts":[]},"finishReason":"MAX_TOKENS"}]}"#;
        assert!(
            parse_response(Provider::Gemini, cut)
                .unwrap_err()
                .to_string()
                .contains("cut off")
        );
        let blocked = r#"{"promptFeedback":{"blockReason":"SAFETY"}}"#;
        assert!(
            parse_response(Provider::Gemini, blocked)
                .unwrap_err()
                .to_string()
                .contains("SAFETY")
        );
        let api = r#"{"error":{"type":"x","message":"model: unknown"}}"#;
        assert_eq!(
            parse_response(Provider::Claude, api)
                .unwrap_err()
                .to_string(),
            "model: unknown"
        );
        assert!(parse_response(Provider::Claude, "pas du json").is_err());
        assert!(parse_response(Provider::Gemini, "{}").is_err());
    }

    #[test]
    fn friendly_errors_never_echo_secrets() {
        let _lang = english();
        let err = |s: &str| anyhow::anyhow!("{s}");
        assert_eq!(
            friendly_error(Provider::Claude, &err("HTTP 401")),
            "Claude: the key was refused"
        );
        assert_eq!(
            friendly_error(Provider::Gemini, &err("HTTP 400")),
            "Gemini: bad request or invalid key"
        );
        assert_eq!(
            friendly_error(Provider::Gemini, &err("HTTP 404")),
            "Gemini: unknown model (pick another one in the list)"
        );
        assert_eq!(
            friendly_error(Provider::Claude, &err("HTTP 429")),
            "Claude: rate limit or quota reached"
        );
        assert_eq!(
            friendly_error(Provider::Claude, &err("HTTP 529")),
            "Claude is unavailable (HTTP 529)"
        );
        assert!(friendly_error(Provider::Claude, &err("délai dépassé")).contains("délai"));
    }

    #[test]
    fn missing_key_is_reported_before_any_request() {
        let _lang = english();
        let err = ask(&config("gemini-2.5-flash"), "Salut").unwrap_err();
        assert_eq!(err, "No Gemini key: add one in the settings");
        assert_eq!(
            ask(&config("claude-haiku-5-5"), "   ").unwrap_err(),
            "Empty question"
        );
    }
}

//! Lecture tolérante du JSON envoyé par Claude Code sur stdin, réduit au
//! strict nécessaire : le contenu des fichiers ou des prompts ne quitte pas
//! le relais.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Événement résumé, tel qu'il transite entre le relais et l'app.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookEvent {
    pub session_id: String,
    #[serde(default)]
    pub cwd: String,
    /// `hook_event_name` : SessionStart, PreToolUse, Notification…
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// Résumé lisible de l'appel d'outil (commande, fichier, URL…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_detail: Option<String>,
}

const MAX_DETAIL: usize = 200;
const MAX_MESSAGE: usize = 200;

impl HookEvent {
    /// `None` si l'entrée n'est pas un événement exploitable.
    pub fn from_hook_input(input: &str) -> Option<Self> {
        let v: Value = serde_json::from_str(input).ok()?;
        let str_field = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        let session_id = str_field("session_id").filter(|s| !s.is_empty())?;
        let kind = str_field("hook_event_name")?;
        let tool_name = str_field("tool_name");
        let tool_detail = tool_name
            .as_deref()
            .and_then(|name| tool_detail(name, v.get("tool_input")?));
        Some(Self {
            session_id,
            cwd: str_field("cwd").unwrap_or_default(),
            kind,
            notification_type: str_field("notification_type"),
            message: str_field("message").map(|m| truncate(&m, MAX_MESSAGE)),
            tool_name,
            tool_detail,
        })
    }

    /// Nom du projet : dernier composant du dossier de travail.
    pub fn project(&self) -> String {
        project_name(&self.cwd)
    }
}

/// Outils « demande de permission » qui sont en fait des questions posées à
/// l'utilisateur : la réponse se fait dans le terminal, pas par Autoriser/Refuser.
pub fn is_interactive_tool(tool: &str) -> bool {
    matches!(tool, "AskUserQuestion" | "ExitPlanMode")
}

pub fn project_name(cwd: &str) -> String {
    cwd.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("Claude")
        .to_owned()
}

fn tool_detail(tool: &str, input: &Value) -> Option<String> {
    let field = |k: &str| input.get(k).and_then(Value::as_str);
    let detail = match tool {
        "Bash" | "PowerShell" => field("command")?.to_owned(),
        "Read" | "Write" | "Edit" | "MultiEdit" | "NotebookEdit" => {
            let path = field("file_path").or_else(|| field("notebook_path"))?;
            project_name(path)
        }
        "WebFetch" => field("url")?.to_owned(),
        "WebSearch" => field("query")?.to_owned(),
        "Glob" | "Grep" => field("pattern")?.to_owned(),
        // Outil inconnu (MCP…) : premier champ texte.
        _ => input
            .as_object()?
            .values()
            .find_map(Value::as_str)?
            .to_owned(),
    };
    // Une seule ligne, sans espaces superflus.
    let one_line = detail.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(truncate(&one_line, MAX_DETAIL))
}

fn truncate(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pre_tool_use_without_leaking_content() {
        let input = r#"{
            "session_id": "abc", "cwd": "C:\\Users\\me\\proj\\boring", "hook_event_name": "PreToolUse",
            "tool_name": "Write", "tool_input": {"file_path": "/x/src/main.rs", "content": "SECRET"}
        }"#;
        let e = HookEvent::from_hook_input(input).unwrap();
        assert_eq!(e.kind, "PreToolUse");
        assert_eq!(e.project(), "boring");
        assert_eq!(e.tool_detail.as_deref(), Some("main.rs"));
        assert!(!serde_json::to_string(&e).unwrap().contains("SECRET"));
    }

    #[test]
    fn bash_command_is_flattened_and_truncated() {
        let long = "x".repeat(500);
        let input = format!(
            r#"{{"session_id":"s","hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{{"command":"cargo  test\n && {long}"}}}}"#
        );
        let e = HookEvent::from_hook_input(&input).unwrap();
        let d = e.tool_detail.unwrap();
        assert!(d.starts_with("cargo test && xxx"));
        assert_eq!(d.chars().count(), MAX_DETAIL + 1);
    }

    #[test]
    fn notification_fields() {
        let e = HookEvent::from_hook_input(
            r#"{"session_id":"s","cwd":"/a/b/","hook_event_name":"Notification","notification_type":"idle_prompt","message":"Claude is waiting"}"#,
        )
        .unwrap();
        assert_eq!(e.notification_type.as_deref(), Some("idle_prompt"));
        assert_eq!(e.project(), "b");
    }

    #[test]
    fn rejects_garbage() {
        assert!(HookEvent::from_hook_input("").is_none());
        assert!(HookEvent::from_hook_input("[1]").is_none());
        assert!(HookEvent::from_hook_input(r#"{"hook_event_name":"Stop"}"#).is_none());
        assert!(
            HookEvent::from_hook_input(r#"{"session_id":"","hook_event_name":"Stop"}"#).is_none()
        );
    }

    #[test]
    fn unknown_tool_uses_first_string() {
        let e = HookEvent::from_hook_input(
            r#"{"session_id":"s","hook_event_name":"PreToolUse","tool_name":"mcp__x__y","tool_input":{"n":1,"q":"hello"}}"#,
        )
        .unwrap();
        assert_eq!(e.tool_detail.as_deref(), Some("hello"));
    }

    #[test]
    fn project_name_edge_cases() {
        assert_eq!(project_name(""), "Claude");
        assert_eq!(project_name("/"), "Claude");
        assert_eq!(project_name("D:\\dev\\app\\"), "app");
    }
}

//! Le relais exécuté par Claude Code (`boringwindows hook`).
//!
//! Règle d'or : ne jamais bloquer ni casser une session Claude. Toute erreur
//! (app fermée, lente, plantée, entrée illisible) se termine par une sortie
//! silencieuse avec le code 0 ; Claude continue alors comme sans hook.

use std::io::{BufRead, BufReader, Read, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::event::HookEvent;
use crate::ipc::{self, Decision, Message, Reply};

/// Délai pour joindre l'app et lui remettre l'événement.
pub const CONNECT_BUDGET: Duration = Duration::from_millis(300);
/// Attente maximale d'une décision (sous le timeout de 300 s posé dans settings.json).
pub const REPLY_CAP: Duration = Duration::from_secs(290);
/// Argument du hook bloquant posé sur `PreToolUse` d'`AskUserQuestion`
/// (`bw-hook hook question`) : seule cette entrée attend une réponse de l'île.
pub const QUESTION_ARG: &str = "question";

enum Step {
    Sent,
    Reply(Reply),
}

/// Ce qu'il s'est passé, pour le journal et le diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// L'app n'a pas pu être jointe (fermée, autre utilisateur, pipe introuvable…).
    Unreachable(String),
    /// Événement remis, aucune réponse attendue.
    Sent,
    /// Réponse de l'île à une demande de permission ou à une question.
    Decided(Reply),
    /// Demande remise, mais pas de réponse exploitable (délai, app fermée entre-temps).
    NoReply,
}

impl Outcome {
    pub fn reply(&self) -> Option<&Reply> {
        match self {
            Self::Decided(r) => Some(r),
            _ => None,
        }
    }
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(e) => write!(f, "app injoignable ({e})"),
            Self::Sent => f.write_str("remis"),
            Self::Decided(r) => write!(f, "réponse : {:?}", r.decision),
            Self::NoReply => f.write_str("remis, sans réponse"),
        }
    }
}

/// Point d'entrée : lit stdin, relaie, écrit éventuellement la décision sur
/// stdout. `mode` : l'argument après `hook` (`question` ou rien). Retourne le
/// code de sortie (toujours 0).
pub fn run(mode: Option<&str>) -> i32 {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        journal("?", "stdin illisible");
        return 0;
    }
    // Un BOM (entrée écrite par PowerShell…) ferait échouer serde_json.
    let raw: Value = serde_json::from_str(input.trim_start_matches('\u{feff}')).unwrap_or_default();
    let Some(event) = HookEvent::from_value(&raw) else {
        journal("?", "événement illisible");
        return 0;
    };
    let question = mode == Some(QUESTION_ARG);
    let wants_reply = if question {
        // Seulement une question que l'île sait afficher ; sinon, sortie
        // muette : Claude la pose dans le terminal.
        if !is_island_question(&event) {
            return 0;
        }
        true
    } else {
        // Les questions et les plans passent par la demande de permission,
        // qui ne les retient pas : réponse dans le terminal.
        event.kind == "PermissionRequest"
            && !event
                .tool_name
                .as_deref()
                .is_some_and(crate::event::is_interactive_tool)
    };
    let kind = event.kind.clone();
    let message = Message {
        v: ipc::PROTOCOL_VERSION,
        wants_reply,
        ancestors: process::ancestors(),
        console_window: process::console_window(),
        event,
    };

    let endpoint = ipc::endpoint();
    let doctor = message.event.session_id == crate::doctor::SESSION;
    let outcome = relay(&endpoint, &message);
    // Les tests du diagnostic ne polluent pas le journal qu'il relit.
    if !doctor {
        let label = if question { "Question" } else { kind.as_str() };
        journal(label, &outcome.to_string());
    }
    if let Some(out) = outcome.reply().and_then(|r| hook_output(r, &raw)) {
        println!("{out}");
    }
    0
}

/// `PreToolUse` d'`AskUserQuestion` dont les questions tiennent dans l'île.
pub fn is_island_question(event: &HookEvent) -> bool {
    event.kind == "PreToolUse"
        && event.tool_name.as_deref() == Some("AskUserQuestion")
        && !event.questions.is_empty()
}

/// Envoie `message` et attend la décision si besoin.
pub fn relay(endpoint: &str, message: &Message) -> Outcome {
    let (tx, rx) = mpsc::channel();
    let endpoint = endpoint.to_owned();
    let Ok(line) = serde_json::to_string(message) else {
        return Outcome::Unreachable("message non sérialisable".into());
    };
    let wants_reply = message.wants_reply;

    // Le travail bloquant se fait dans un thread : le thread principal garde
    // la maîtrise des délais et peut sortir même si le pipe est figé.
    std::thread::spawn(move || {
        let deadline = Instant::now() + CONNECT_BUDGET;
        let mut stream = match connect(&endpoint, deadline) {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(Err(e.to_string()));
                return;
            }
        };
        if let Err(e) = stream
            .write_all(format!("{line}\n").as_bytes())
            .and_then(|()| stream.flush())
        {
            let _ = tx.send(Err(format!("écriture : {e}")));
            return;
        }
        let _ = tx.send(Ok(Step::Sent));
        if !wants_reply {
            return;
        }
        let mut reply = String::new();
        let mut reader = BufReader::new(stream.take(ipc::MAX_LINE));
        if reader.read_line(&mut reply).is_ok()
            && let Ok(reply) = serde_json::from_str::<Reply>(&reply)
        {
            let _ = tx.send(Ok(Step::Reply(reply)));
        }
    });

    match rx.recv_timeout(CONNECT_BUDGET) {
        Ok(Ok(Step::Sent)) => {}
        Ok(Err(e)) => return Outcome::Unreachable(e),
        _ => return Outcome::Unreachable("pas de réponse en 300 ms".into()),
    }
    if !wants_reply {
        return Outcome::Sent;
    }
    match rx.recv_timeout(REPLY_CAP) {
        Ok(Ok(Step::Reply(reply))) => Outcome::Decided(reply),
        _ => Outcome::NoReply,
    }
}

/// Journal du relais (`hook.log`), une ligne par appel, pour le diagnostic.
/// Plafonné : au-delà de 256 Ko, l'ancien journal est mis de côté.
fn journal(kind: &str, outcome: &str) {
    const MAX: u64 = 256 * 1024;
    let path = crate::install::hook_log_path();
    if path.metadata().is_ok_and(|m| m.len() > MAX) {
        let _ = std::fs::rename(&path, path.with_extension("log.old"));
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let line = format!(
        "{:02}:{:02}:{:02} UTC  {kind:<18} {outcome}\n",
        secs / 3600 % 24,
        secs / 60 % 60,
        secs % 60
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Sortie JSON pour Claude Code. `input` est l'entrée brute du hook : les
/// questions et les règles renvoyées en viennent, l'app ne fournit que des
/// rangs et ne peut donc rien ajouter. `None` : rien à écrire, Claude demande
/// dans le terminal.
pub fn hook_output(reply: &Reply, input: &Value) -> Option<Value> {
    let kind = input.get("hook_event_name").and_then(Value::as_str);
    let permission = |decision: Value| {
        serde_json::json!({
            "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": decision }
        })
    };
    match reply.decision {
        Decision::Ask => None,
        Decision::Answer => {
            if kind != Some("PreToolUse") {
                return None;
            }
            let mut updated = input.get("tool_input")?.clone();
            updated["answers"] = answers(&updated, &reply.answers)?;
            Some(serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "allow",
                    "updatedInput": updated,
                }
            }))
        }
        _ if kind != Some("PermissionRequest") => None,
        Decision::Allow => Some(permission(serde_json::json!({ "behavior": "allow" }))),
        Decision::AllowAlways => {
            let mut decision = serde_json::json!({ "behavior": "allow" });
            // Seulement des règles que l'île a pu résumer, donc montrer.
            if let Some(event) = HookEvent::from_value(input)
                && event.always.is_some()
                && let Some(suggestions) = input.get("permission_suggestions")
            {
                decision["updatedPermissions"] = suggestions.clone();
            }
            Some(permission(decision))
        }
        Decision::Deny => Some(permission(serde_json::json!({
            "behavior": "deny",
            "message": bw_i18n::tr!("Denied from BoringWindows", "Refusé depuis BoringWindows"),
        }))),
    }
}

/// `answers` d'`AskUserQuestion` : texte exact de chaque question → libellés
/// choisis, séparés par des virgules. `None` si les rangs ne collent pas.
fn answers(tool_input: &Value, picks: &[Vec<usize>]) -> Option<Value> {
    let questions = tool_input.get("questions")?.as_array()?;
    if picks.len() != questions.len() {
        return None;
    }
    let mut out = serde_json::Map::new();
    for (q, picked) in questions.iter().zip(picks) {
        let multi = q
            .get("multiSelect")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if picked.is_empty() || (!multi && picked.len() > 1) {
            return None;
        }
        let options = q.get("options")?.as_array()?;
        let labels = picked
            .iter()
            .map(|&i| options.get(i)?.get("label")?.as_str())
            .collect::<Option<Vec<_>>>()?;
        out.insert(
            q.get("question")?.as_str()?.to_owned(),
            labels.join(", ").into(),
        );
    }
    Some(Value::Object(out))
}

#[cfg(windows)]
fn connect(endpoint: &str, deadline: Instant) -> std::io::Result<std::fs::File> {
    // ERROR_PIPE_BUSY : toutes les instances sont occupées, on réessaie.
    const ERROR_PIPE_BUSY: i32 = 231;
    loop {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(endpoint)
        {
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(pipe) => {
                server_identity::check(&pipe)?;
                return Ok(pipe);
            }
            Err(e) => return Err(e),
        }
    }
}

#[cfg(unix)]
fn connect(endpoint: &str, _deadline: Instant) -> std::io::Result<std::os::unix::net::UnixStream> {
    // Socket et dossier à nous seulement : sinon, ce n'est pas l'app qui écoute.
    let path = std::path::Path::new(endpoint);
    if let Some(dir) = path.parent() {
        ipc::check_private(dir)?;
    }
    ipc::check_private(path)?;
    std::os::unix::net::UnixStream::connect(endpoint)
}

/// Le nom du pipe est public : un autre compte de la machine pourrait le créer
/// avant l'app. On vérifie que le processus qui écoute tourne sous notre compte.
#[cfg(windows)]
mod server_identity {
    use std::os::windows::io::AsRawHandle;

    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{
        GetLengthSid, GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser,
    };
    use windows::Win32::System::Pipes::GetNamedPipeServerProcessId;
    use windows::Win32::System::Threading::{
        GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    pub fn check(pipe: &std::fs::File) -> std::io::Result<()> {
        let mut pid = 0u32;
        // SAFETY: `pipe` est un handle de pipe ouvert ; `pid` est un pointeur valide.
        unsafe { GetNamedPipeServerProcessId(HANDLE(pipe.as_raw_handle()), &mut pid) }?;
        // SAFETY: appel sans pointeur ; le handle est fermé juste après usage.
        let server = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }?;
        let theirs = user_sid(server);
        // SAFETY: handle obtenu ci-dessus, fermé une seule fois.
        unsafe {
            let _ = CloseHandle(server);
        }
        // SAFETY: pseudo-handle du processus courant, rien à fermer.
        let ours = user_sid(unsafe { GetCurrentProcess() })?;
        if theirs? != ours {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "le pipe est tenu par un autre compte Windows",
            ));
        }
        Ok(())
    }

    /// SID (octets) du compte sous lequel tourne `process`.
    fn user_sid(process: HANDLE) -> std::io::Result<Vec<u8>> {
        let mut token = HANDLE::default();
        // SAFETY: `token` est un pointeur valide ; fermé plus bas.
        unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }?;
        let mut len = 0u32;
        // Premier appel : taille nécessaire (échoue volontairement).
        // SAFETY: aucun tampon fourni, `len` est un pointeur valide.
        let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut len) };
        // Tampon de u64 : aligné pour lire TOKEN_USER.
        let mut buf = vec![0u64; (len as usize).div_ceil(8)];
        // SAFETY: `buf` fait au moins `len` octets.
        let result = unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                Some(buf.as_mut_ptr().cast()),
                len,
                &mut len,
            )
        };
        // SAFETY: handle ouvert ci-dessus, fermé une seule fois.
        unsafe {
            let _ = CloseHandle(token);
        }
        result?;
        // SAFETY: GetTokenInformation a rempli `buf` avec un TOKEN_USER valide,
        // dont le SID pointe dans `buf` (vivant jusqu'à la fin de la fonction).
        let sid = unsafe {
            let user = &*buf.as_ptr().cast::<TOKEN_USER>();
            let n = GetLengthSid(user.User.Sid) as usize;
            std::slice::from_raw_parts(user.User.Sid.0.cast::<u8>(), n).to_vec()
        };
        Ok(sid)
    }
}

#[cfg(windows)]
mod process {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, FreeConsole, GetConsoleWindow,
    };
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::GetCurrentProcessId;

    pub fn ancestors() -> Vec<u32> {
        let mut parents = std::collections::HashMap::new();
        // SAFETY: instantané Toolhelp parcouru puis fermé ; `entry` est
        // initialisé avec sa taille comme l'exige l'API.
        unsafe {
            let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
                return Vec::new();
            };
            let mut entry = PROCESSENTRY32W {
                dwSize: size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut ok = Process32FirstW(snap, &mut entry).is_ok();
            while ok {
                parents.insert(entry.th32ProcessID, entry.th32ParentProcessID);
                ok = Process32NextW(snap, &mut entry).is_ok();
            }
            let _ = CloseHandle(snap);
        }
        // SAFETY: appel sans argument.
        let mut pid = unsafe { GetCurrentProcessId() };
        let mut chain = Vec::new();
        while let Some(&parent) = parents.get(&pid) {
            if parent == 0 || chain.contains(&parent) || chain.len() >= 12 {
                break;
            }
            chain.push(parent);
            pid = parent;
        }
        chain
    }

    pub fn console_window() -> Option<i64> {
        // SAFETY: appels console sans pointeur ; la console empruntée au
        // parent est libérée aussitôt (les handles stdin/stdout ne changent pas).
        unsafe {
            let mut hwnd = GetConsoleWindow();
            if hwnd.is_invalid() && AttachConsole(ATTACH_PARENT_PROCESS).is_ok() {
                hwnd = GetConsoleWindow();
                let _ = FreeConsole();
            }
            (!hwnd.is_invalid()).then_some(hwnd.0 as i64)
        }
    }
}

#[cfg(unix)]
mod process {
    pub fn ancestors() -> Vec<u32> {
        let mut chain = vec![std::os::unix::process::parent_id()];
        // Linux : on remonte via /proc ; macOS : via proc_pidinfo.
        while chain.len() < 12 {
            let pid = *chain.last().unwrap_or(&0);
            let Some(parent) = parent_of(pid) else {
                break;
            };
            if parent <= 1 || chain.contains(&parent) {
                break;
            }
            chain.push(parent);
        }
        chain
    }

    #[cfg(target_os = "macos")]
    fn parent_of(pid: u32) -> Option<u32> {
        let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
        let size = size_of::<libc::proc_bsdinfo>() as libc::c_int;
        // SAFETY: `info` est un tampon de la taille annoncée ; le noyau le
        // remplit et renvoie le nombre d'octets écrits.
        let written = unsafe {
            libc::proc_pidinfo(
                i32::try_from(pid).ok()?,
                libc::PROC_PIDTBSDINFO,
                0,
                info.as_mut_ptr().cast(),
                size,
            )
        };
        if written != size {
            return None;
        }
        // SAFETY: entièrement rempli par proc_pidinfo (vérifié ci-dessus).
        Some(unsafe { info.assume_init() }.pbi_ppid)
    }

    #[cfg(not(target_os = "macos"))]
    fn parent_of(pid: u32) -> Option<u32> {
        linux_parent(pid)
    }

    #[cfg(not(target_os = "macos"))]
    fn linux_parent(pid: u32) -> Option<u32> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // Le nom du processus est entre parenthèses et peut contenir des espaces.
        stat.rsplit_once(')')?
            .1
            .split_whitespace()
            .nth(1)?
            .parse()
            .ok()
    }

    pub fn console_window() -> Option<i64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question_input() -> Value {
        serde_json::json!({
            "session_id": "s",
            "hook_event_name": "PreToolUse",
            "tool_name": "AskUserQuestion",
            "tool_input": {
                "questions": [
                    { "question": "Which framework?", "header": "Framework", "multiSelect": false,
                      "options": [{ "label": "React" }, { "label": "Vue" }] },
                    { "question": "Extras?", "header": "Extras", "multiSelect": true,
                      "options": [{ "label": "Tests" }, { "label": "CI" }, { "label": "Docs" }] }
                ]
            }
        })
    }

    fn answer(picks: Vec<Vec<usize>>) -> Reply {
        Reply {
            decision: Decision::Answer,
            answers: picks,
        }
    }

    #[test]
    fn answers_echo_the_questions() {
        let input = question_input();
        let out = hook_output(&answer(vec![vec![1], vec![0, 2]]), &input).unwrap();
        let specific = &out["hookSpecificOutput"];
        assert_eq!(specific["hookEventName"], "PreToolUse");
        assert_eq!(specific["permissionDecision"], "allow");
        let updated = &specific["updatedInput"];
        assert_eq!(updated["questions"], input["tool_input"]["questions"]);
        assert_eq!(updated["answers"]["Which framework?"], "Vue");
        assert_eq!(updated["answers"]["Extras?"], "Tests, Docs");
    }

    #[test]
    fn bad_answers_fall_back_to_the_terminal() {
        let input = question_input();
        for picks in [
            vec![vec![0]],                  // une question sans réponse
            vec![vec![5], vec![0]],         // option inexistante
            vec![vec![0, 1], vec![0]],      // deux choix pour une question simple
            vec![vec![0], vec![]],          // aucun choix
            vec![vec![0], vec![0], vec![]], // question en trop
        ] {
            assert_eq!(
                hook_output(&answer(picks.clone()), &input),
                None,
                "{picks:?}"
            );
        }
        // Une réponse n'a de sens que pour la question retenue en PreToolUse.
        let mut permission = input;
        permission["hook_event_name"] = "PermissionRequest".into();
        assert_eq!(
            hook_output(&answer(vec![vec![0], vec![0]]), &permission),
            None
        );
    }

    #[test]
    fn allow_always_applies_only_shown_suggestions() {
        let mut input = serde_json::json!({
            "session_id": "s",
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": { "command": "cargo test" },
            "permission_suggestions": [
                { "type": "addRules", "rules": [{ "toolName": "Bash", "ruleContent": "cargo test:*" }],
                  "behavior": "allow", "destination": "localSettings" }
            ]
        });
        let always = Reply::new(Decision::AllowAlways);
        let out = hook_output(&always, &input).unwrap();
        let decision = &out["hookSpecificOutput"]["decision"];
        assert_eq!(decision["behavior"], "allow");
        assert_eq!(
            decision["updatedPermissions"],
            input["permission_suggestions"]
        );

        // Une règle que l'île ne sait pas résumer : simple autorisation.
        input["permission_suggestions"][0]["type"] = "replaceRules".into();
        let out = hook_output(&always, &input).unwrap();
        assert_eq!(out["hookSpecificOutput"]["decision"]["behavior"], "allow");
        assert!(
            out["hookSpecificOutput"]["decision"]
                .get("updatedPermissions")
                .is_none()
        );
    }

    #[test]
    fn permission_decisions() {
        let input = serde_json::json!({
            "session_id": "s", "hook_event_name": "PermissionRequest", "tool_name": "Bash"
        });
        let allow = hook_output(&Reply::new(Decision::Allow), &input).unwrap();
        assert_eq!(allow["hookSpecificOutput"]["decision"]["behavior"], "allow");
        let deny = hook_output(&Reply::new(Decision::Deny), &input).unwrap();
        assert_eq!(deny["hookSpecificOutput"]["decision"]["behavior"], "deny");
        assert!(deny["hookSpecificOutput"]["decision"]["message"].is_string());
        assert_eq!(hook_output(&Reply::new(Decision::Ask), &input), None);
        // Autoriser une question sans réponse ne suffit pas à Claude : rien.
        assert_eq!(
            hook_output(&Reply::new(Decision::Allow), &question_input()),
            None
        );
    }

    #[test]
    fn only_displayable_questions_are_held() {
        let e = HookEvent::from_value(&question_input()).unwrap();
        assert!(is_island_question(&e));
        let mut other = question_input();
        other["tool_input"]["questions"][0]["options"] = serde_json::json!([]);
        assert!(!is_island_question(&HookEvent::from_value(&other).unwrap()));
    }
}

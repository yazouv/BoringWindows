//! Le relais exécuté par Claude Code (`boringwindows hook`).
//!
//! Règle d'or : ne jamais bloquer ni casser une session Claude. Toute erreur
//! (app fermée, lente, plantée, entrée illisible) se termine par une sortie
//! silencieuse avec le code 0 ; Claude continue alors comme sans hook.

use std::io::{BufRead, BufReader, Read, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::event::HookEvent;
use crate::ipc::{self, Decision, Message, Reply};

/// Délai pour joindre l'app et lui remettre l'événement.
pub const CONNECT_BUDGET: Duration = Duration::from_millis(300);
/// Attente maximale d'une décision (sous le timeout de 300 s posé dans settings.json).
pub const REPLY_CAP: Duration = Duration::from_secs(290);

enum Step {
    Sent,
    Reply(Decision),
}

/// Point d'entrée : lit stdin, relaie, écrit éventuellement la décision sur
/// stdout. Retourne le code de sortie (toujours 0).
pub fn run() -> i32 {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return 0;
    }
    let Some(event) = HookEvent::from_hook_input(&input) else {
        return 0;
    };
    let wants_reply = event.kind == "PermissionRequest";
    let message = Message {
        v: ipc::PROTOCOL_VERSION,
        wants_reply,
        ancestors: process::ancestors(),
        console_window: process::console_window(),
        event,
    };

    if let Some(decision) = relay(&ipc::endpoint(), &message) {
        print_decision(decision);
    }
    0
}

/// Envoie `message` et attend la décision si besoin. `None` = rien à dire à Claude.
pub fn relay(endpoint: &str, message: &Message) -> Option<Decision> {
    let (tx, rx) = mpsc::channel();
    let endpoint = endpoint.to_owned();
    let line = serde_json::to_string(message).ok()? + "\n";
    let wants_reply = message.wants_reply;

    // Le travail bloquant se fait dans un thread : le thread principal garde
    // la maîtrise des délais et peut sortir même si le pipe est figé.
    std::thread::spawn(move || {
        let deadline = Instant::now() + CONNECT_BUDGET;
        let Some(mut stream) = connect(&endpoint, deadline) else {
            return;
        };
        if stream
            .write_all(line.as_bytes())
            .and_then(|()| stream.flush())
            .is_err()
        {
            return;
        }
        let _ = tx.send(Step::Sent);
        if !wants_reply {
            return;
        }
        let mut reply = String::new();
        let mut reader = BufReader::new(stream.take(ipc::MAX_LINE));
        if reader.read_line(&mut reply).is_ok()
            && let Ok(reply) = serde_json::from_str::<Reply>(&reply)
        {
            let _ = tx.send(Step::Reply(reply.decision));
        }
    });

    match rx.recv_timeout(CONNECT_BUDGET) {
        Ok(Step::Sent) => {}
        _ => return None,
    }
    if !wants_reply {
        return None;
    }
    match rx.recv_timeout(REPLY_CAP) {
        Ok(Step::Reply(decision)) => Some(decision),
        _ => None,
    }
}

fn print_decision(decision: Decision) {
    let behavior = match decision {
        Decision::Allow => "allow",
        Decision::Deny => "deny",
        Decision::Ask => return,
    };
    let mut out = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": { "behavior": behavior }
        }
    });
    if decision == Decision::Deny {
        out["hookSpecificOutput"]["decision"]["message"] = "Refusé depuis BoringWindows".into();
    }
    println!("{out}");
}

#[cfg(windows)]
fn connect(endpoint: &str, deadline: Instant) -> Option<std::fs::File> {
    // ERROR_PIPE_BUSY : toutes les instances sont occupées, on réessaie.
    const ERROR_PIPE_BUSY: i32 = 231;
    loop {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(endpoint)
        {
            Ok(f) => return Some(f),
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => return None,
        }
    }
}

#[cfg(unix)]
fn connect(endpoint: &str, _deadline: Instant) -> Option<std::os::unix::net::UnixStream> {
    std::os::unix::net::UnixStream::connect(endpoint).ok()
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
        // Linux : on remonte via /proc ; ailleurs, le parent direct suffit.
        while chain.len() < 12 {
            let pid = *chain.last().unwrap_or(&0);
            let Some(parent) = linux_parent(pid) else {
                break;
            };
            if parent <= 1 || chain.contains(&parent) {
                break;
            }
            chain.push(parent);
        }
        chain
    }

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

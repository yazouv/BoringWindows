//! Module « claude » : écoute le relais, suit les sessions, publie l'état et
//! répond aux demandes de permission.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use bw_core::{Module, ModuleCtx};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::sync::oneshot;

use crate::config::ClaudeConfig;
use crate::ipc::{self, Decision, Listener, Message, Reply};
use crate::tracker::{Snapshot, Tracker};

pub const MODULE_ID: &str = "claude";

static NEXT_PROMPT_ID: AtomicU64 = AtomicU64::new(1);

enum Cmd {
    Event {
        message: Box<Message>,
        prompt: Option<(u64, oneshot::Sender<Reply>)>,
    },
    Decide {
        id: u64,
        reply: Reply,
    },
    /// Le relais est parti avant la réponse (Claude l'a interrompu).
    Cancel {
        id: u64,
    },
}

pub struct ClaudeModule {
    config: ClaudeConfig,
    endpoint: String,
    commands: Option<UnboundedSender<Cmd>>,
}

impl ClaudeModule {
    pub fn new(config: ClaudeConfig, endpoint: String) -> Self {
        Self {
            config,
            endpoint,
            commands: None,
        }
    }
}

impl Module for ClaudeModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        let listener = Listener::bind(&self.endpoint).map_err(|e| {
            anyhow::anyhow!(bw_i18n::tr!(
                "cannot listen on {}: {e}",
                "impossible d'écouter sur {} : {e}",
                self.endpoint
            ))
        })?;
        log::info!("claude : en écoute sur {}", self.endpoint);

        let (tx, rx) = unbounded_channel();
        self.commands = Some(tx.clone());
        let island = Island {
            permissions: self.config.permissions,
            questions: self.config.questions,
        };
        ctx.spawn(accept_loop(listener, tx, island));
        ctx.spawn(state_loop(rx, ctx.clone(), self.config.clone()));
        Ok(())
    }

    /// Actions : `allow:<id>`, `always:<id>`, `deny:<id>`, `ask:<id>`, et
    /// `answer:<id>:<choix>` (voir `parse_action`).
    fn on_action(&mut self, action: &str) {
        if let (Some((id, reply)), Some(tx)) = (parse_action(action), &self.commands) {
            let _ = tx.send(Cmd::Decide { id, reply });
        }
    }
}

/// Ce à quoi l'île répond ; le reste part au terminal sans attendre.
#[derive(Debug, Clone, Copy)]
struct Island {
    permissions: bool,
    questions: bool,
}

impl Island {
    fn handles(self, message: &Message) -> bool {
        if crate::hook::is_island_question(&message.event) {
            self.questions
        } else {
            self.permissions
        }
    }
}

/// `answer:<id>:<choix>` : les rangs des options choisies, `,` entre les
/// options d'une question, `;` entre les questions (`answer:7:1;0,2`).
fn parse_action(action: &str) -> Option<(u64, Reply)> {
    let (verb, rest) = action.split_once(':')?;
    let decision = match verb {
        "allow" => Decision::Allow,
        "always" => Decision::AllowAlways,
        "deny" => Decision::Deny,
        "ask" => Decision::Ask,
        "answer" => {
            let (id, picks) = rest.split_once(':')?;
            let answers = picks
                .split(';')
                .map(|q| q.split(',').map(|i| i.parse().ok()).collect())
                .collect::<Option<Vec<Vec<usize>>>>()?;
            return Some((
                id.parse().ok()?,
                Reply {
                    decision: Decision::Answer,
                    answers,
                },
            ));
        }
        _ => return None,
    };
    Some((rest.parse().ok()?, Reply::new(decision)))
}

async fn accept_loop(mut listener: Listener, tx: UnboundedSender<Cmd>, island: Island) {
    loop {
        match listener.accept().await {
            Ok(stream) => {
                tokio::spawn(handle_client(stream, tx.clone(), island));
            }
            Err(e) => {
                log::warn!("claude : connexion refusée : {e}");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        if tx.is_closed() {
            break;
        }
    }
}

async fn handle_client<S>(stream: S, tx: UnboundedSender<Cmd>, island: Island)
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let (read, mut write) = tokio::io::split(stream);
    let mut reader = BufReader::new(read.take(ipc::MAX_LINE));
    let mut line = String::new();
    if reader.read_line(&mut line).await.is_err() {
        return;
    }
    let message: Message = match serde_json::from_str(&line) {
        Ok(m) => m,
        Err(e) => {
            log::warn!("claude : message illisible : {e}");
            return;
        }
    };

    if !message.wants_reply {
        let _ = tx.send(Cmd::Event {
            message: Box::new(message),
            prompt: None,
        });
        return;
    }

    let reply = if island.handles(&message) {
        let id = NEXT_PROMPT_ID.fetch_add(1, Ordering::Relaxed);
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = tx.send(Cmd::Event {
            message: Box::new(message),
            prompt: Some((id, reply_tx)),
        });
        tokio::select! {
            r = reply_rx => r.unwrap_or_default(),
            () = wait_eof(&mut reader) => {
                let _ = tx.send(Cmd::Cancel { id });
                return;
            }
        }
    } else {
        let _ = tx.send(Cmd::Event {
            message: Box::new(message),
            prompt: None,
        });
        Reply::new(Decision::Ask)
    };

    let Ok(reply) = serde_json::to_string(&reply) else {
        return;
    };
    if write
        .write_all(format!("{reply}\n").as_bytes())
        .await
        .is_ok()
    {
        let _ = write.flush().await;
        // Laisser le relais lire la réponse avant de fermer : fermer un named
        // pipe peut jeter les données non lues.
        let _ = tokio::time::timeout(Duration::from_secs(2), wait_eof(&mut reader)).await;
    }
}

async fn wait_eof<R: AsyncRead + Unpin>(reader: &mut R) {
    let mut buf = [0u8; 64];
    while matches!(reader.read(&mut buf).await, Ok(n) if n > 0) {}
}

async fn state_loop(mut rx: UnboundedReceiver<Cmd>, ctx: ModuleCtx, config: ClaudeConfig) {
    let mut tracker = Tracker::new(config.done_for());
    let mut replies: HashMap<u64, oneshot::Sender<Reply>> = HashMap::new();
    let mut last: Option<Snapshot> = None;

    loop {
        let deadline = tracker.next_deadline();
        let cmd = tokio::select! {
            cmd = rx.recv() => match cmd {
                Some(cmd) => Some(cmd),
                None => break,
            },
            () = sleep_until(deadline) => None,
        };

        let now = Instant::now();
        match cmd {
            Some(Cmd::Event { message, prompt }) => {
                let settled = tracker.on_event(
                    &message.event,
                    &message.ancestors,
                    message.console_window,
                    now,
                );
                // Réglées dans le terminal : on libère les relais en attente.
                for id in settled {
                    if let Some(reply) = replies.remove(&id) {
                        let _ = reply.send(Reply::new(Decision::Ask));
                    }
                }
                if let Some((id, reply)) = prompt {
                    tracker.add_prompt(id, &message.event, now + config.permission_wait());
                    replies.insert(id, reply);
                }
            }
            Some(Cmd::Decide { id, reply }) => {
                if let Some(sender) = replies.remove(&id) {
                    tracker.resolve_prompt(id, reply.decision);
                    let _ = sender.send(reply);
                }
            }
            Some(Cmd::Cancel { id }) => {
                replies.remove(&id);
                tracker.resolve_prompt(id, Decision::Ask);
            }
            None => {}
        }

        let (expired, _) = tracker.tick(now);
        for id in expired {
            if let Some(reply) = replies.remove(&id) {
                let _ = reply.send(Reply::new(Decision::Ask));
            }
        }

        let snapshot = tracker.snapshot();
        if last.as_ref() != Some(&snapshot) {
            ctx.set_attention(snapshot.attention, snapshot.summary.clone());
            ctx.set_state(snapshot.clone());
            last = Some(snapshot);
        }
    }
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(d) => tokio::time::sleep_until(d.into()).await,
        None => std::future::pending().await,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::sync::mpsc;

    use bw_core::{Action, Attention, ModuleEventKind, ModuleHost};

    use super::*;
    use crate::event::HookEvent;
    use crate::hook;

    fn message(kind: &str, wants_reply: bool) -> Message {
        Message {
            v: ipc::PROTOCOL_VERSION,
            wants_reply,
            ancestors: vec![1234],
            console_window: None,
            event: HookEvent {
                session_id: "s1".into(),
                cwd: "/dev/boring".into(),
                kind: kind.into(),
                tool_name: Some("Bash".into()),
                tool_detail: Some("cargo test".into()),
                ..HookEvent::default()
            },
        }
    }

    /// Bout en bout : relais → socket → module → décision depuis « l'UI ».
    #[test]
    fn relay_round_trip_with_permission() {
        let dir = tempfile::tempdir().unwrap();
        let endpoint = dir.path().join("bw.sock").to_string_lossy().into_owned();

        let (tx, rx) = mpsc::channel();
        let module = ClaudeModule::new(ClaudeConfig::default(), endpoint.clone());
        let host = ModuleHost::spawn(vec![Box::new(module)], move |e| {
            let _ = tx.send(e);
        })
        .unwrap();
        // Laisser le module ouvrir la socket.
        while !std::path::Path::new(&endpoint).exists() {
            std::thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(
            hook::relay(&endpoint, &message("UserPromptSubmit", false)),
            hook::Outcome::Sent
        );

        let ep = endpoint.clone();
        let pending =
            std::thread::spawn(move || hook::relay(&ep, &message("PermissionRequest", true)));

        // Attendre que la demande apparaisse dans l'état publié, puis répondre.
        let prompt_id = loop {
            let event = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            if let ModuleEventKind::State(state) = event.kind
                && let Some(prompt) = &state.downcast_ref::<Snapshot>().unwrap().prompt
            {
                assert_eq!(prompt.detail, "cargo test");
                break prompt.id;
            }
        };
        host.send_action(Action {
            module: MODULE_ID.into(),
            name: format!("allow:{prompt_id}"),
        });
        assert_eq!(
            pending.join().unwrap(),
            hook::Outcome::Decided(Reply::new(Decision::Allow))
        );

        // L'attention retombe sur « travaille ».
        let level = loop {
            let event = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            if let ModuleEventKind::Attention { level, .. } = event.kind {
                break level;
            }
        };
        assert_eq!(level, Attention::Low);
        host.shutdown();
    }

    #[test]
    fn relay_gives_up_quickly_without_app() {
        let dir = tempfile::tempdir().unwrap();
        let endpoint = dir
            .path()
            .join("absent.sock")
            .to_string_lossy()
            .into_owned();
        let start = Instant::now();
        assert!(matches!(
            hook::relay(&endpoint, &message("PermissionRequest", true)),
            hook::Outcome::Unreachable(_)
        ));
        assert!(start.elapsed() < hook::CONNECT_BUDGET + Duration::from_millis(100));
    }
}

#[cfg(test)]
mod action_tests {
    use super::*;

    #[test]
    fn parses_island_actions() {
        assert_eq!(
            parse_action("allow:3"),
            Some((3, Reply::new(Decision::Allow)))
        );
        assert_eq!(
            parse_action("always:4"),
            Some((4, Reply::new(Decision::AllowAlways)))
        );
        assert_eq!(
            parse_action("answer:7:1;0,2"),
            Some((
                7,
                Reply {
                    decision: Decision::Answer,
                    answers: vec![vec![1], vec![0, 2]],
                }
            ))
        );
        assert_eq!(parse_action("answer:7:"), None);
        assert_eq!(parse_action("answer:7:a"), None);
        assert_eq!(parse_action("allow:x"), None);
        assert_eq!(parse_action("nope:1"), None);
    }
}

#[cfg(test)]
mod restart_tests {
    use std::sync::mpsc;

    use bw_core::{Action, ModuleEvent, ModuleEventKind, ModuleHost};

    use super::*;
    use crate::event::HookEvent;
    use crate::hook;

    fn permission_request() -> Message {
        Message {
            v: ipc::PROTOCOL_VERSION,
            wants_reply: true,
            ancestors: vec![],
            console_window: None,
            event: HookEvent {
                session_id: "s1".into(),
                cwd: "/dev/boring".into(),
                kind: "PermissionRequest".into(),
                tool_name: Some("Bash".into()),
                tool_detail: Some("cargo test".into()),
                ..HookEvent::default()
            },
        }
    }

    fn start(endpoint: &str) -> (ModuleHost, mpsc::Receiver<ModuleEvent>) {
        let (tx, rx) = mpsc::channel();
        let module = ClaudeModule::new(ClaudeConfig::default(), endpoint.to_owned());
        let host = ModuleHost::spawn(vec![Box::new(module)], move |e| {
            let _ = tx.send(e);
        })
        .unwrap();
        (host, rx)
    }

    /// Relaie une demande depuis un autre thread. Le module démarre en
    /// arrière-plan : on réessaie tant qu'il n'écoute pas (5 s au plus).
    fn relay_soon(endpoint: &str) -> std::thread::JoinHandle<hook::Outcome> {
        let endpoint = endpoint.to_owned();
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                match hook::relay(&endpoint, &permission_request()) {
                    hook::Outcome::Unreachable(_) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    outcome => break outcome,
                }
            }
        })
    }

    fn wait_prompt(rx: &mpsc::Receiver<ModuleEvent>) -> u64 {
        loop {
            let event = rx.recv_timeout(Duration::from_secs(10)).unwrap();
            if let ModuleEventKind::State(state) = event.kind
                && let Some(prompt) = &state.downcast_ref::<Snapshot>().unwrap().prompt
            {
                break prompt.id;
            }
        }
    }

    /// La config change pendant qu'une demande attend : le module redémarre
    /// et écoute de nouveau, la demande en attente repart au terminal.
    #[test]
    fn restart_while_a_permission_waits() {
        #[cfg(unix)]
        let dir = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        let endpoint = dir.path().join("bw.sock").to_string_lossy().into_owned();
        #[cfg(windows)]
        let endpoint = format!(
            r"\\.\pipe\boringwindows-test-restart-{}",
            std::process::id()
        );

        let (host, rx) = start(&endpoint);
        let pending = relay_soon(&endpoint);
        wait_prompt(&rx);

        // Ce que fait `Controller::restart_modules`.
        host.shutdown();
        let (host, rx) = start(&endpoint);
        assert_eq!(pending.join().unwrap(), hook::Outcome::NoReply);

        // Le nouveau module reçoit les demandes et y répond.
        let next = relay_soon(&endpoint);
        let id = wait_prompt(&rx);
        host.send_action(Action {
            module: MODULE_ID.into(),
            name: format!("allow:{id}"),
        });
        assert_eq!(
            next.join().unwrap(),
            hook::Outcome::Decided(Reply::new(Decision::Allow))
        );
        host.shutdown();
    }
}

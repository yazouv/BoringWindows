use std::future::Future;
use std::thread::JoinHandle;

use tokio::runtime::Handle;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::Attention;

/// Un module de l'île (Claude, musique, calendrier…).
///
/// `start` est appelé une fois sur le thread des modules ; il doit rendre la
/// main vite et lancer son travail via [`ModuleCtx::spawn`]. Tout doit être
/// événementiel : pas de boucle de polling.
pub trait Module: Send + 'static {
    fn id(&self) -> &'static str;

    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()>;

    /// Action déclenchée depuis l'UI (« next », « allow »…).
    fn on_action(&mut self, _action: &str) {}
}

/// Ce qu'un module publie vers l'UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleEvent {
    pub module: &'static str,
    pub kind: ModuleEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleEventKind {
    Attention {
        level: Attention,
        summary: Option<String>,
    },
}

/// Action de l'UI adressée à un module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub module: String,
    pub name: String,
}

/// Poignée donnée à un module au démarrage.
#[derive(Clone)]
pub struct ModuleCtx {
    id: &'static str,
    events: UnboundedSender<ModuleEvent>,
    runtime: Handle,
}

impl ModuleCtx {
    pub fn set_attention(&self, level: Attention, summary: Option<String>) {
        self.emit(ModuleEventKind::Attention { level, summary });
    }

    pub fn emit(&self, kind: ModuleEventKind) {
        // Le récepteur ne disparaît qu'à l'arrêt : rien à faire dans ce cas.
        let _ = self.events.send(ModuleEvent {
            module: self.id,
            kind,
        });
    }

    pub fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.runtime.spawn(fut);
    }
}

/// Fait tourner les modules sur un thread dédié (runtime tokio mono-thread).
///
/// Sans module actif, aucun thread n'est créé : coût nul.
pub struct ModuleHost {
    actions: Option<UnboundedSender<Action>>,
    thread: Option<JoinHandle<()>>,
}

impl ModuleHost {
    /// Démarre les modules. `sink` reçoit chaque événement sur le thread des
    /// modules : c'est à l'appelant de le renvoyer vers le thread UI.
    pub fn spawn<S>(modules: Vec<Box<dyn Module>>, sink: S) -> anyhow::Result<Self>
    where
        S: Fn(ModuleEvent) + Send + 'static,
    {
        if modules.is_empty() {
            return Ok(Self {
                actions: None,
                thread: None,
            });
        }

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()?;
        let (action_tx, action_rx) = unbounded_channel();

        let thread = std::thread::Builder::new()
            .name("bw-modules".into())
            .spawn(move || runtime.block_on(run(modules, sink, action_rx)))?;

        Ok(Self {
            actions: Some(action_tx),
            thread: Some(thread),
        })
    }

    pub fn send_action(&self, action: Action) {
        if let Some(tx) = &self.actions {
            let _ = tx.send(action);
        }
    }

    /// Arrête les modules et attend la fin du thread.
    pub fn shutdown(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        self.actions = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for ModuleHost {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn run<S>(mut modules: Vec<Box<dyn Module>>, sink: S, mut actions: UnboundedReceiver<Action>)
where
    S: Fn(ModuleEvent),
{
    let (event_tx, mut events) = unbounded_channel();
    let runtime = Handle::current();

    modules.retain_mut(|module| {
        let ctx = ModuleCtx {
            id: module.id(),
            events: event_tx.clone(),
            runtime: runtime.clone(),
        };
        match module.start(ctx) {
            Ok(()) => true,
            Err(err) => {
                log::error!("module {} : échec du démarrage : {err:#}", module.id());
                false
            }
        }
    });
    drop(event_tx);

    loop {
        tokio::select! {
            Some(event) = events.recv() => sink(event),
            action = actions.recv() => match action {
                Some(action) => {
                    match modules.iter_mut().find(|m| m.id() == action.module) {
                        Some(module) => module.on_action(&action.name),
                        None => log::warn!("action pour un module inconnu : {}", action.module),
                    }
                }
                // L'hôte a été arrêté.
                None => break,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    struct Echo(mpsc::Sender<String>);

    impl Module for Echo {
        fn id(&self) -> &'static str {
            "echo"
        }

        fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
            ctx.set_attention(Attention::Low, Some("ready".into()));
            Ok(())
        }

        fn on_action(&mut self, action: &str) {
            self.0.send(action.to_owned()).unwrap();
        }
    }

    struct Broken;

    impl Module for Broken {
        fn id(&self) -> &'static str {
            "broken"
        }

        fn start(&mut self, _ctx: ModuleCtx) -> anyhow::Result<()> {
            anyhow::bail!("nope")
        }
    }

    struct Ticker;

    impl Module for Ticker {
        fn id(&self) -> &'static str {
            "ticker"
        }

        fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
            let task_ctx = ctx.clone();
            ctx.spawn(async move {
                tokio::time::sleep(Duration::from_millis(5)).await;
                task_ctx.set_attention(Attention::High, None);
            });
            Ok(())
        }
    }

    fn recv(rx: &mpsc::Receiver<ModuleEvent>) -> ModuleEvent {
        rx.recv_timeout(Duration::from_secs(5))
            .expect("événement attendu")
    }

    #[test]
    fn no_modules_spawns_nothing() {
        let host = ModuleHost::spawn(Vec::new(), |_| {}).unwrap();
        assert!(host.thread.is_none());
        host.send_action(Action {
            module: "x".into(),
            name: "y".into(),
        });
    }

    #[test]
    fn events_reach_the_sink_and_broken_modules_are_skipped() {
        let (tx, rx) = mpsc::channel();
        let (action_tx, action_rx) = mpsc::channel();
        let modules: Vec<Box<dyn Module>> = vec![
            Box::new(Broken),
            Box::new(Echo(action_tx)),
            Box::new(Ticker),
        ];
        let host = ModuleHost::spawn(modules, move |e| tx.send(e).unwrap()).unwrap();

        assert_eq!(
            recv(&rx),
            ModuleEvent {
                module: "echo",
                kind: ModuleEventKind::Attention {
                    level: Attention::Low,
                    summary: Some("ready".into()),
                },
            }
        );
        assert_eq!(recv(&rx).module, "ticker");

        host.send_action(Action {
            module: "echo".into(),
            name: "ping".into(),
        });
        assert_eq!(
            action_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            "ping"
        );
        host.shutdown();
    }
}

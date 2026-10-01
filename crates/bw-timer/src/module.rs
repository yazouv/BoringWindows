use std::time::Instant;

use bw_core::{Attention, Module, ModuleCtx};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::config::TimerConfig;
use crate::machine::{Machine, Phase};

pub const MODULE_ID: &str = "timer";

pub struct TimerModule {
    config: TimerConfig,
    actions: Option<UnboundedSender<String>>,
}

impl TimerModule {
    pub fn new(config: TimerConfig) -> Self {
        Self {
            config,
            actions: None,
        }
    }
}

impl Module for TimerModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        let (tx, mut rx) = unbounded_channel::<String>();
        self.actions = Some(tx);
        let mut machine = Machine::new(self.config.presets.clone(), self.config.done_for());
        let presets = self.config.presets.clone();

        ctx.clone().spawn(async move {
            publish(&ctx, &machine, Instant::now());
            loop {
                let now = Instant::now();
                let wake = machine.next_wake(now);
                let sleep = async {
                    match wake {
                        Some(at) => tokio::time::sleep_until(tokio::time::Instant::from_std(at)).await,
                        None => std::future::pending().await,
                    }
                };
                tokio::select! {
                    () = sleep => {
                        machine.tick(Instant::now());
                    }
                    action = rx.recv() => {
                        let Some(action) = action else { break };
                        let now = Instant::now();
                        match action.split_once(':') {
                            Some(("start", minutes)) => {
                                // Seulement une durée proposée : l'UI ne choisit pas librement.
                                if let Ok(m) = minutes.parse::<u32>()
                                    && presets.contains(&m)
                                {
                                    machine.start(m, now);
                                }
                            }
                            _ => match action.as_str() {
                                "pause" => machine.pause(now),
                                "resume" => machine.resume(now),
                                "reset" => machine.reset(),
                                _ => {}
                            },
                        }
                    }
                }
                publish(&ctx, &machine, Instant::now());
            }
        });
        Ok(())
    }

    /// Actions : `start:<minutes>`, `pause`, `resume`, `reset`.
    fn on_action(&mut self, action: &str) {
        if let Some(tx) = &self.actions {
            let _ = tx.send(action.to_owned());
        }
    }
}

fn publish(ctx: &ModuleCtx, machine: &Machine, now: Instant) {
    let (level, summary) = match machine.phase() {
        Phase::Idle => (Attention::None, None),
        Phase::Running | Phase::Paused => (
            Attention::Low,
            Some(bw_i18n::tr!(
                "Timer · {} min",
                "Minuteur · {} min",
                machine.minutes_left(now)
            )),
        ),
        Phase::Done => (
            Attention::High,
            Some(bw_i18n::tr!("Timer done", "Minuteur terminé")),
        ),
    };
    ctx.set_attention(level, summary);
    ctx.set_state(machine.snapshot(now));
}

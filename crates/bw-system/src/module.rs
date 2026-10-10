//! Module « system » : relève le processeur et la mémoire pendant que l'île
//! est ouverte (actions `open` / `close` du contrôleur), et en continu,
//! moins souvent, si une alerte est réglée.

use std::time::{Duration, Instant};

use bw_core::{Attention, Module, ModuleCtx};
use bw_i18n::tr;
use sysinfo::{MINIMUM_CPU_UPDATE_INTERVAL, ProcessRefreshKind, ProcessesToUpdate, System};
use tokio::sync::watch;

use crate::{SystemConfig, SystemSnapshot, Watch};

pub const MODULE_ID: &str = "system";
/// Relevé île fermée, quand une alerte est réglée.
const BACKGROUND: Duration = Duration::from_secs(5);
/// Durée d'affichage d'une alerte dans la pilule.
const SHOW_ALERT: Duration = Duration::from_secs(6);

pub struct SystemModule {
    config: SystemConfig,
    open: watch::Sender<bool>,
}

impl SystemModule {
    pub fn new(config: SystemConfig) -> Self {
        Self {
            config,
            open: watch::Sender::new(false),
        }
    }

    pub fn is_supported() -> bool {
        sysinfo::IS_SUPPORTED_SYSTEM
    }
}

impl Module for SystemModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        let config = self.config.clone();
        let mut open = self.open.subscribe();
        let task_ctx = ctx.clone();
        ctx.spawn(async move {
            let ctx = task_ctx;
            let mut sys = System::new();
            let mut cpu_watch = Watch::new(config.cpu_alert_percent, config.alert_after());
            let mut ram_watch = Watch::new(config.ram_alert_percent, config.alert_after());
            let mut last: Option<Instant> = None;
            loop {
                let is_open = *open.borrow_and_update();
                if !is_open && !config.alerts() {
                    // Rien à mesurer : on attend l'ouverture de l'île.
                    if open.changed().await.is_err() {
                        break;
                    }
                    continue;
                }
                let period = if is_open {
                    config.refresh()
                } else {
                    BACKGROUND
                };
                // L'usage du processeur se calcule entre deux relevés : sans
                // relevé récent, on en fait un juste avant.
                if last.is_none_or(|t| t.elapsed() > period * 2) {
                    sys.refresh_cpu_usage();
                    tokio::time::sleep(MINIMUM_CPU_UPDATE_INTERVAL + Duration::from_millis(50))
                        .await;
                }
                sys.refresh_cpu_usage();
                sys.refresh_memory();
                last = Some(Instant::now());
                let snapshot = SystemSnapshot {
                    cpu: sys.global_cpu_usage(),
                    memory_used: sys.used_memory(),
                    memory_total: sys.total_memory(),
                };
                if is_open {
                    ctx.set_state(snapshot);
                }

                let now = Instant::now();
                if cpu_watch.feed(snapshot.cpu, now) {
                    let top = top_process(&mut sys, true).await;
                    alert(&ctx, Load::Cpu, snapshot.cpu, top);
                }
                if ram_watch.feed(snapshot.ram_percent(), now) {
                    let top = top_process(&mut sys, false).await;
                    alert(&ctx, Load::Memory, snapshot.ram_percent(), top);
                }

                tokio::select! {
                    () = tokio::time::sleep(period) => {}
                    changed = open.changed() => if changed.is_err() { break },
                }
            }
        });
        Ok(())
    }

    /// Actions : `open` / `close` (l'île s'ouvre ou se referme).
    fn on_action(&mut self, action: &str) {
        match action {
            "open" => self.open.send_replace(true),
            "close" => self.open.send_replace(false),
            _ => return,
        };
    }
}

#[derive(Clone, Copy)]
enum Load {
    Cpu,
    Memory,
}

/// Annonce dans la pilule, retirée après quelques secondes.
fn alert(ctx: &ModuleCtx, load: Load, value: f32, top: Option<String>) {
    let value = value.clamp(0.0, 100.0).round() as u32;
    let mut text = match load {
        Load::Cpu => tr!("CPU at {value} %", "Processeur à {value} %"),
        Load::Memory => tr!("Memory at {value} %", "Mémoire à {value} %"),
    };
    if let Some(top) = top {
        text = format!("{text} · {top}");
    }
    log::info!("système : {text}");
    ctx.set_attention(Attention::High, Some(text));
    let clear = ctx.clone();
    ctx.spawn(async move {
        tokio::time::sleep(SHOW_ALERT).await;
        clear.set_attention(Attention::None, None);
    });
}

/// Processus qui consomme le plus (processeur ou mémoire), relevé seulement
/// au moment d'une alerte.
async fn top_process(sys: &mut System, by_cpu: bool) -> Option<String> {
    let kind = if by_cpu {
        ProcessRefreshKind::nothing().with_cpu()
    } else {
        ProcessRefreshKind::nothing().with_memory()
    };
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
    if by_cpu {
        // Même principe que pour le processeur entier : deux relevés.
        tokio::time::sleep(MINIMUM_CPU_UPDATE_INTERVAL + Duration::from_millis(50)).await;
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
    }
    let weight = |p: &sysinfo::Process| {
        if by_cpu {
            f64::from(p.cpu_usage())
        } else {
            p.memory() as f64
        }
    };
    let top = sys
        .processes()
        .values()
        // « System Idle Process » de Windows : le temps libre, pas un coupable.
        .filter(|p| p.pid().as_u32() != 0)
        .max_by(|a, b| weight(a).total_cmp(&weight(b)))?;
    Some(crate::process_label(&top.name().to_string_lossy()).to_owned())
}

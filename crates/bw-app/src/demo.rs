//! Module de démonstration (`[modules.demo] enabled = true`) : fait tourner
//! les niveaux d'attention pour tester les animations sans vrais modules.

use std::time::Duration;

use bw_core::{Attention, Module, ModuleCtx};

pub struct Demo;

const STEPS: [(Attention, Option<&str>); 4] = [
    (Attention::Low, Some("♪ Daft Punk — Digital Love")),
    (Attention::High, Some("Réunion dans 5 min")),
    (Attention::Urgent, Some("Claude attend ta réponse")),
    (Attention::None, None),
];

impl Module for Demo {
    fn id(&self) -> &'static str {
        "demo"
    }

    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        let task_ctx = ctx.clone();
        ctx.spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(4));
            for (level, summary) in STEPS.iter().cycle() {
                tick.tick().await;
                task_ctx.set_attention(*level, summary.map(str::to_owned));
            }
        });
        Ok(())
    }
}

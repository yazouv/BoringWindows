use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bw_core::{Attention, Module, ModuleCtx};

use crate::config::PluginsConfig;
use crate::manifest::{PluginSpec, discover};
use crate::runtime::{Output, Plugin};

pub const MODULE_ID: &str = "plugins";

/// Après ce nombre d'erreurs de suite, un plugin est arrêté.
const MAX_FAILURES: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginItem {
    pub name: String,
    pub text: String,
    pub attention: u8,
}

/// Ce que l'île affiche : une ligne par plugin qui a du texte.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PluginsSnapshot {
    pub items: Vec<PluginItem>,
}

pub struct PluginsModule {
    config: PluginsConfig,
    dir: PathBuf,
}

impl PluginsModule {
    pub fn new(config: PluginsConfig, dir: PathBuf) -> Self {
        Self { config, dir }
    }
}

impl Module for PluginsModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn start(&mut self, ctx: ModuleCtx) -> anyhow::Result<()> {
        let mut specs = Vec::new();
        let mut errors = Vec::new();
        for found in discover(&self.dir) {
            match found {
                Ok(spec) if self.config.allows(&spec.id) => specs.push(spec),
                Ok(_) => {}
                Err(e) => errors.push(e),
            }
        }
        for e in &errors {
            log::warn!("{e}");
        }
        if specs.is_empty() {
            log::info!("plugins : aucun plugin dans {}", self.dir.display());
            return Ok(());
        }

        // Une ligne par plugin, dans l'ordre des dossiers.
        let state = Arc::new(Mutex::new(vec![None::<PluginItem>; specs.len()]));
        for (index, spec) in specs.into_iter().enumerate() {
            let state = state.clone();
            let ctx = ctx.clone();
            ctx.clone().spawn(run_plugin(spec, index, state, ctx));
        }
        Ok(())
    }
}

async fn run_plugin(
    spec: PluginSpec,
    index: usize,
    state: Arc<Mutex<Vec<Option<PluginItem>>>>,
    ctx: ModuleCtx,
) {
    let mut plugin = match std::fs::read(&spec.wasm_path)
        .map_err(anyhow::Error::from)
        .and_then(|bytes| Plugin::load(&spec.id, &bytes))
    {
        Ok(p) => p,
        Err(e) => {
            log::warn!("plugin {} : {e:#}", spec.id);
            return;
        }
    };
    log::info!(
        "plugin {} chargé (toutes les {} s)",
        spec.id,
        spec.interval.as_secs()
    );

    let mut failures = 0;
    loop {
        let item = match plugin.update() {
            Ok(Output { text, attention }) => {
                failures = 0;
                (!text.is_empty()).then(|| PluginItem {
                    name: spec.name.clone(),
                    text,
                    attention,
                })
            }
            Err(e) => {
                failures += 1;
                log::warn!("plugin {} : {e:#}", spec.id);
                Some(PluginItem {
                    name: spec.name.clone(),
                    text: "⚠ erreur".into(),
                    attention: 0,
                })
            }
        };
        publish(&state, index, item, &ctx);
        if failures >= MAX_FAILURES {
            log::warn!("plugin {} arrêté après {MAX_FAILURES} erreurs", spec.id);
            return;
        }
        tokio::time::sleep(spec.interval).await;
    }
}

/// Met à jour la ligne du plugin et republie l'ensemble si quelque chose a changé.
fn publish(
    state: &Mutex<Vec<Option<PluginItem>>>,
    index: usize,
    item: Option<PluginItem>,
    ctx: &ModuleCtx,
) {
    let Ok(mut all) = state.lock() else { return };
    if all[index] == item {
        return;
    }
    all[index] = item;
    let items: Vec<PluginItem> = all.iter().flatten().cloned().collect();
    let top = items.iter().max_by_key(|i| i.attention);
    match top {
        Some(i) if i.attention > 0 => ctx.set_attention(
            if i.attention >= 2 {
                Attention::High
            } else {
                Attention::Low
            },
            Some(format!("{} : {}", i.name, i.text)),
        ),
        _ => ctx.set_attention(Attention::None, None),
    }
    ctx.set_state(PluginsSnapshot { items });
}

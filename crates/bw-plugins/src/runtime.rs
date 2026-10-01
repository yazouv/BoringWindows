//! Exécution d'un plugin WASM dans un bac à sable (wasmi) : aucun accès au
//! système, seulement quatre fonctions fournies par l'hôte, un budget de calcul
//! par appel et une mémoire plafonnée.

use anyhow::{Context, anyhow};
use wasmi::{
    Caller, Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc,
};

/// Calcul autorisé par appel de `bw_update` (unités de « carburant » de wasmi).
const FUEL_PER_CALL: u64 = 20_000_000;
/// Mémoire maximale d'un plugin.
const MAX_MEMORY_BYTES: usize = 16 * 1024 * 1024;
/// Longueur maximale du texte affiché (en caractères).
pub const MAX_TEXT_CHARS: usize = 120;

/// Ce qu'un plugin a publié lors de son dernier appel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Output {
    pub text: String,
    /// 0 rien, 1 discret, 2 à regarder (jamais « urgent » : réservé à l'app).
    pub attention: u8,
}

struct Host {
    output: Output,
    limits: StoreLimits,
    name: String,
}

pub struct Plugin {
    store: Store<Host>,
    update: TypedFunc<(), ()>,
}

impl Plugin {
    /// Compile et instancie `wasm` ; `name` sert aux messages de log.
    pub fn load(name: &str, wasm: &[u8]) -> anyhow::Result<Self> {
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, wasm).context("module WASM invalide")?;

        let mut store = Store::new(
            &engine,
            Host {
                output: Output::default(),
                limits: StoreLimitsBuilder::new()
                    .memory_size(MAX_MEMORY_BYTES)
                    .memories(1)
                    .instances(1)
                    .build(),
                name: name.to_owned(),
            },
        );
        store.limiter(|host| &mut host.limits);
        store.set_fuel(FUEL_PER_CALL)?;

        let mut linker = <Linker<Host>>::new(&engine);
        linker.func_wrap(
            "bw",
            "set_text",
            |mut caller: Caller<Host>, ptr: i32, len: i32| {
                let text = read_str(&mut caller, ptr, len)?;
                caller.data_mut().output.text = text.chars().take(MAX_TEXT_CHARS).collect();
                Ok(())
            },
        )?;
        linker.func_wrap(
            "bw",
            "set_attention",
            |mut caller: Caller<Host>, level: i32| {
                caller.data_mut().output.attention = level.clamp(0, 2) as u8;
            },
        )?;
        linker.func_wrap(
            "bw",
            "log",
            |mut caller: Caller<Host>, ptr: i32, len: i32| {
                let text = read_str(&mut caller, ptr, len)?;
                log::info!(
                    "plugin {} : {}",
                    caller.data().name,
                    text.chars().take(300).collect::<String>()
                );
                Ok(())
            },
        )?;
        linker.func_wrap("bw", "now_unix", |_: Caller<Host>| -> i64 {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64)
        })?;

        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .context("instanciation")?;
        let update = instance
            .get_typed_func::<(), ()>(&store, "bw_update")
            .context("le plugin doit exporter `bw_update` (sans paramètre ni résultat)")?;
        Ok(Self { store, update })
    }

    /// Appelle `bw_update` ; en cas d'erreur (piège, carburant épuisé…), le plugin reste utilisable.
    pub fn update(&mut self) -> anyhow::Result<Output> {
        self.store.set_fuel(FUEL_PER_CALL)?;
        self.update
            .call(&mut self.store, ())
            .map_err(|e| anyhow!("{e}"))?;
        Ok(self.store.data().output.clone())
    }
}

fn read_str(caller: &mut Caller<Host>, ptr: i32, len: i32) -> Result<String, wasmi::Error> {
    let bad = |_| wasmi::Error::new("pointeur ou longueur négatif");
    let (ptr, len) = (
        usize::try_from(ptr).map_err(bad)?,
        usize::try_from(len).map_err(bad)?,
    );
    if len > 4096 {
        return Err(wasmi::Error::new("texte trop long"));
    }
    let memory = caller
        .get_export("memory")
        .and_then(wasmi::Extern::into_memory)
        .ok_or_else(|| wasmi::Error::new("le plugin n'exporte pas `memory`"))?;
    let mut buf = vec![0; len];
    memory
        .read(&caller, ptr, &mut buf)
        .map_err(|_| wasmi::Error::new("lecture hors de la mémoire du plugin"))?;
    String::from_utf8(buf).map_err(|_| wasmi::Error::new("texte non UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin(wat: &str) -> anyhow::Result<Plugin> {
        Plugin::load("test", &wat::parse_str(wat).unwrap())
    }

    const HELLO: &str = r#"(module
        (import "bw" "set_text" (func $set_text (param i32 i32)))
        (import "bw" "set_attention" (func $att (param i32)))
        (memory (export "memory") 1)
        (data (i32.const 16) "Hello plugin")
        (func (export "bw_update")
            (call $set_text (i32.const 16) (i32.const 12))
            (call $att (i32.const 2))))"#;

    #[test]
    fn shipped_example_plugin_works() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/plugins/hello");
        let spec = crate::manifest::discover(dir.parent().unwrap())
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("manifeste valide");
        assert_eq!(spec[0].name, "Exemple");
        let mut p = Plugin::load("hello", &std::fs::read(&spec[0].wasm_path).unwrap()).unwrap();
        let out = p.update().unwrap();
        assert_eq!(
            out,
            Output {
                text: "Bonjour depuis un plugin WASM".into(),
                attention: 1
            }
        );
    }

    #[test]
    fn publishes_text_and_attention() {
        let mut p = plugin(HELLO).unwrap();
        let out = p.update().unwrap();
        assert_eq!(
            out,
            Output {
                text: "Hello plugin".into(),
                attention: 2
            }
        );
        // Rappelé : même résultat.
        assert_eq!(p.update().unwrap(), out);
    }

    #[test]
    fn attention_is_clamped_and_text_truncated() {
        let long = "x".repeat(300);
        let wat = format!(
            r#"(module
            (import "bw" "set_text" (func $t (param i32 i32)))
            (import "bw" "set_attention" (func $a (param i32)))
            (memory (export "memory") 1)
            (data (i32.const 0) "{long}")
            (func (export "bw_update") (call $t (i32.const 0) (i32.const 300)) (call $a (i32.const 99))))"#
        );
        let out = plugin(&wat).unwrap().update().unwrap();
        assert_eq!(out.text.chars().count(), MAX_TEXT_CHARS);
        assert_eq!(out.attention, 2);
    }

    #[test]
    fn endless_loop_runs_out_of_fuel_and_plugin_survives() {
        let mut p = plugin(
            r#"(module (memory (export "memory") 1)
                (func (export "bw_update") (loop $l (br $l))))"#,
        )
        .unwrap();
        let err = p.update().unwrap_err().to_string();
        assert!(err.to_lowercase().contains("fuel"), "{err}");
        assert!(p.update().is_err());
    }

    #[test]
    fn memory_is_capped() {
        // Demande 1 Go : l'instanciation échoue.
        let r = plugin(r#"(module (memory (export "memory") 16384) (func (export "bw_update")))"#);
        assert!(r.is_err());
        // Croissance dynamique refusée : `memory.grow` renvoie -1.
        let mut p = plugin(
            r#"(module
                (import "bw" "set_attention" (func $a (param i32)))
                (memory (export "memory") 1)
                (func (export "bw_update")
                    (if (i32.eq (memory.grow (i32.const 4000)) (i32.const -1))
                        (then (call $a (i32.const 1))))))"#,
        )
        .unwrap();
        assert_eq!(p.update().unwrap().attention, 1);
    }

    #[test]
    fn host_calls_with_bad_pointers_trap_instead_of_crashing() {
        let mut p = plugin(
            r#"(module
                (import "bw" "set_text" (func $t (param i32 i32)))
                (memory (export "memory") 1)
                (func (export "bw_update") (call $t (i32.const 65530) (i32.const 100))))"#,
        )
        .unwrap();
        assert!(p.update().is_err());
        let mut p = plugin(
            r#"(module
                (import "bw" "set_text" (func $t (param i32 i32)))
                (memory (export "memory") 1)
                (data (i32.const 0) "\ff\fe")
                (func (export "bw_update") (call $t (i32.const 0) (i32.const 2))))"#,
        )
        .unwrap();
        assert!(p.update().is_err());
    }

    #[test]
    fn rejects_unknown_imports_and_missing_exports() {
        // Aucune fonction système (WASI…) : l'instanciation échoue.
        assert!(plugin(
            r#"(module (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))
                (memory (export "memory") 1) (func (export "bw_update")))"#
        )
        .is_err());
        assert!(plugin(r#"(module (memory (export "memory") 1))"#).is_err());
        assert!(Plugin::load("x", b"pas du wasm").is_err());
    }
}

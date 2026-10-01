//! Vue personnelle de l'île ouverte : `layouts/<nom>.slint`, compilée à
//! l'exécution (slint-interpreter) et affichée dans l'île à la place de la
//! vue fournie. L'API de données (propriétés et callbacks que le fichier peut
//! déclarer) est documentée dans docs/*/src/layouts.md.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;

use slint::{ComponentFactory, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use slint_interpreter::{
    Compiler, ComponentDefinition, ComponentInstance, DiagnosticLevel, Struct, Value,
};

use super::Controller;
use crate::Island;

/// Vue compilée et état de son instance (créée par l'île à son premier affichage).
pub(super) struct CustomView {
    instance: Rc<RefCell<Option<ComponentInstance>>>,
    properties: HashSet<String>,
}

pub(super) struct Compiled {
    pub definition: ComponentDefinition,
    pub properties: HashSet<String>,
    pub callbacks: HashSet<String>,
}

/// Compile `path` ; l'erreur est un message lisible pour l'île.
pub(super) fn compile(path: &Path) -> Result<Compiled, String> {
    let source = std::fs::read_to_string(path).map_err(|e| format!("{} : {e}", path.display()))?;
    let mut compiler = Compiler::default();
    compiler.set_style("fluent".into());
    let result = block_on(compiler.build_from_source(source, path.to_path_buf()))
        .ok_or_else(|| "compilation sans fin".to_owned())?;
    let file = path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let errors: Vec<String> = result
        .diagnostics()
        .filter(|d| d.level() == DiagnosticLevel::Error)
        .take(3)
        .map(|d| format!("{file}:{} {}", d.line_column().0, d.message()))
        .collect();
    if !errors.is_empty() {
        return Err(errors.join(" · "));
    }
    let names: Vec<&str> = result.component_names().collect();
    let name = if names.contains(&"View") {
        "View"
    } else {
        *names.first().ok_or_else(|| {
            bw_i18n::tr!(
                "{file}: no exported component (write `export component View inherits Window`)",
                "{file} : aucun composant exporté (écris `export component View inherits Window`)"
            )
        })?
    };
    let definition = result
        .component(name)
        .ok_or_else(|| format!("{file} : composant {name} introuvable"))?;
    Ok(Compiled {
        properties: definition.properties().map(|(n, _)| n).collect(),
        callbacks: definition.callbacks().collect(),
        definition,
    })
}

/// Attend un futur qui n'a rien à attendre réellement (lecture de fichiers locaux).
fn block_on<F: std::future::Future>(future: F) -> Option<F::Output> {
    use std::task::{Context, Poll, Waker};
    let mut cx = Context::from_waker(Waker::noop());
    let mut future = Box::pin(future);
    for _ in 0..100_000 {
        if let Poll::Ready(v) = future.as_mut().poll(&mut cx) {
            return Some(v);
        }
    }
    None
}

fn s(text: impl AsRef<str>) -> Value {
    Value::String(SharedString::from(text.as_ref()))
}

fn color(c: slint::Color) -> Value {
    Value::Brush(slint::Brush::SolidColor(c))
}

fn model(rows: Vec<Value>) -> Value {
    Value::Model(ModelRc::new(VecModel::from(rows)))
}

fn row(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Struct(Struct::from_iter(
        fields.into_iter().map(|(k, v)| (k.to_owned(), v)),
    ))
}

type Handler = Box<dyn Fn(&Island, &[Value])>;

fn text_arg(args: &[Value], i: usize) -> SharedString {
    match args.get(i) {
        Some(Value::String(t)) => t.clone(),
        _ => SharedString::new(),
    }
}

impl Controller {
    /// (Re)lit `layout.view` : compile le fichier et l'affiche, ou retombe sur
    /// la vue fournie en signalant l'erreur.
    pub(super) fn reload_custom_view(&self) {
        let (name, dir) = {
            let config = self.config.borrow();
            (
                config.layout.view.clone(),
                self.path.parent().map(Path::to_path_buf).unwrap_or_default(),
            )
        };
        if name.is_empty() {
            *self.custom_view.borrow_mut() = None;
            self.ui.set_custom_view(false);
            return;
        }
        let path = bw_config::layout_path(&dir, &name);
        match compile(&path) {
            Ok(compiled) => self.show_custom_view(compiled),
            Err(message) => {
                log::error!("vue personnelle : {message}");
                *self.custom_view.borrow_mut() = None;
                self.ui.set_custom_view(false);
                self.arbiter.borrow_mut().claim(
                    super::CONFIG_ERROR,
                    bw_core::Attention::High,
                    Some(format!("⚠ {message}")),
                );
                self.refresh_shape();
            }
        }
    }

    fn show_custom_view(&self, compiled: Compiled) {
        let instance: Rc<RefCell<Option<ComponentInstance>>> = Rc::default();
        let slot = instance.clone();
        let ui = self.ui.as_weak();
        let callbacks = compiled.callbacks;
        let definition = compiled.definition;
        let factory = ComponentFactory::new(move |ctx| {
            let inst = definition.create_embedded(ctx).ok()?;
            wire_callbacks(&inst, &callbacks, &ui);
            *slot.borrow_mut() = Some(inst.clone_strong());
            // Premières données dès que l'instance existe.
            super::post(|c| c.sync_custom_view());
            Some(inst)
        });
        *self.custom_view.borrow_mut() = Some(CustomView {
            instance,
            properties: compiled.properties,
        });
        self.ui.set_custom_view_factory(factory);
        self.ui.set_custom_view(true);
    }

    /// Pousse l'état de l'île dans la vue personnelle (sans effet si absente).
    pub(super) fn sync_custom_view(&self) {
        let view = self.custom_view.borrow();
        let Some(view) = view.as_ref() else { return };
        let instance = view.instance.borrow();
        let Some(instance) = instance.as_ref() else { return };
        let ui = &self.ui;
        let set = |name: &str, value: Value| {
            if view.properties.contains(name) {
                let _ = instance.set_property(name, value);
            }
        };

        set("expanded", Value::Bool(ui.get_expanded()));
        set("time-text", s(ui.get_time_text()));
        set("date-text", s(ui.get_date_text()));
        set("accent", color(ui.get_accent()));
        set("foreground", color(ui.get_fg()));
        set("background", color(ui.get_bg()));

        let media = ui.get_media();
        set("has-media", Value::Bool(ui.get_has_media()));
        set("media-title", s(&media.title));
        set("media-artist", s(&media.artist));
        set("media-source", s(&media.source));
        set("media-playing", Value::Bool(media.playing));
        set("media-position", s(&media.position));
        set("media-duration", s(&media.duration));
        set("media-progress", Value::Number(f64::from(media.progress)));
        set("media-multi-source", Value::Bool(media.multi_source));
        set("media-can-previous", Value::Bool(media.can_previous));
        set("media-can-next", Value::Bool(media.can_next));
        set("media-can-toggle", Value::Bool(media.can_toggle));
        set("media-can-seek", Value::Bool(media.can_seek));
        set(
            "viz-bars",
            model(
                ui.get_viz_bars()
                    .iter()
                    .map(|v| Value::Number(f64::from(v)))
                    .collect(),
            ),
        );
        set("has-media-art", Value::Bool(media.has_art));
        set("media-art", Value::Image(media.art));

        let prompt = ui.get_prompt();
        set("has-prompt", Value::Bool(ui.get_has_prompt()));
        set("prompt-id", s(&prompt.id));
        set("prompt-project", s(&prompt.project));
        set("prompt-tool", s(&prompt.tool));
        set("prompt-detail", s(&prompt.detail));

        set(
            "claude-rows",
            model(
                ui.get_claude_rows()
                    .iter()
                    .map(|r| {
                        row([
                            ("id", s(&r.id)),
                            ("project", s(&r.project)),
                            ("status", s(&r.status)),
                            ("urgent", Value::Bool(r.urgent)),
                            ("active", Value::Bool(r.active)),
                        ])
                    })
                    .collect(),
            ),
        );
        set(
            "agenda-rows",
            model(
                ui.get_agenda_rows()
                    .iter()
                    .map(|r| {
                        row([
                            ("title", s(&r.title)),
                            ("time", s(&r.time)),
                            ("location", s(&r.location)),
                            ("relative", s(&r.relative)),
                            ("join-url", s(&r.join_url)),
                            ("has-join", Value::Bool(r.has_join)),
                            ("soon", Value::Bool(r.soon)),
                        ])
                    })
                    .collect(),
            ),
        );

        set(
            "shelf-rows",
            model(
                ui.get_shelf_rows()
                    .iter()
                    .map(|r| row([("name", s(&r.name)), ("path", s(&r.path))]))
                    .collect(),
            ),
        );
        set("shelf-more", s(ui.get_shelf_more()));

        let timer = ui.get_timer();
        set("has-timer", Value::Bool(ui.get_has_timer()));
        set("timer-phase", Value::Number(f64::from(timer.phase)));
        set("timer-time", s(&timer.time));
        set("timer-progress", Value::Number(f64::from(timer.progress)));
        set(
            "timer-presets",
            model(timer.presets.iter().map(s).collect()),
        );
    }
}

/// Relie les callbacks que la vue déclare à ceux de l'île (mêmes actions que
/// la vue fournie).
fn wire_callbacks(inst: &ComponentInstance, declared: &HashSet<String>, ui: &slint::Weak<Island>) {
    let wire = |name: &str, f: Handler| {
        if declared.contains(name) {
            let ui = ui.clone();
            let _ = inst.set_callback(name, move |args| {
                if let Some(ui) = ui.upgrade() {
                    f(&ui, args);
                }
                Value::Void
            });
        }
    };
    wire(
        "media-action",
        Box::new(|ui, a| ui.invoke_media_action(text_arg(a, 0))),
    );
    wire(
        "media-seek",
        Box::new(|ui, a| {
            if let Some(Value::Number(n)) = a.first() {
                ui.invoke_media_seek(*n as f32);
            }
        }),
    );
    wire(
        "claude-decide",
        Box::new(|ui, a| ui.invoke_claude_decide(text_arg(a, 0), text_arg(a, 1))),
    );
    wire(
        "claude-focus",
        Box::new(|ui, a| ui.invoke_claude_focus(text_arg(a, 0))),
    );
    wire(
        "open-url",
        Box::new(|ui, a| ui.invoke_open_url(text_arg(a, 0))),
    );
    wire(
        "shelf-open",
        Box::new(|ui, a| {
            if let Some(Value::Number(n)) = a.first() {
                ui.invoke_shelf_open(*n as i32);
            }
        }),
    );
    wire(
        "shelf-remove",
        Box::new(|ui, a| {
            if let Some(Value::Number(n)) = a.first() {
                ui.invoke_shelf_remove(*n as i32);
            }
        }),
    );
    wire(
        "timer-action",
        Box::new(|ui, a| ui.invoke_timer_action(text_arg(a, 0))),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, content: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("bw-view-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("view.slint");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn compiles_a_valid_view_and_lists_its_api() {
        let path = temp_file(
            "ok",
            r#"
export component View inherits Window {
    in property <string> media-title;
    in property <[{ title: string, time: string }]> agenda-rows;
    callback media-action(string);
    Text { text: root.media-title; }
}"#,
        );
        let compiled = compile(&path).expect("compile");
        assert!(compiled.properties.contains("media-title"));
        assert!(compiled.properties.contains("agenda-rows"));
        assert!(compiled.callbacks.contains("media-action"));
    }

    #[test]
    fn reports_errors_with_file_and_line() {
        let path = temp_file("bad", "export component View inherits Window {\n  Foo { }\n}");
        let err = compile(&path).err().expect("erreur attendue");
        assert!(err.starts_with("view.slint:2"), "{err}");

        assert!(compile(Path::new("C:/n/existe/pas.slint")).is_err());
        let empty = temp_file("empty", "");
        assert!(compile(&empty).is_err());
    }
}

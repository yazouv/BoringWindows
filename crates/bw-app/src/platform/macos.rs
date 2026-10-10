//! Intégration macOS : l'île est un panneau non activant posé au-dessus de la
//! barre de menus, centré en haut de l'écran (autour de l'encoche s'il y en
//! a une). Les clics hors de la pilule traversent la fenêtre, le démarrage à
//! la connexion passe par un LaunchAgent.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;

use block2::RcBlock;
use bw_config::MonitorChoice;
use objc2::rc::Retained;
use objc2::runtime::{
    AnyClass, AnyObject, Bool, ClassBuilder, NSObjectProtocol, ProtocolObject, Sel,
};
use objc2::{AnyThread, ClassType, MainThreadMarker, sel};
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSApplication, NSApplicationActivationPolicy,
    NSApplicationDidChangeScreenParametersNotification, NSColor, NSColorSpace, NSEvent,
    NSEventMask, NSModalResponseOK, NSOpenPanel, NSPanel, NSRunningApplication, NSScreen, NSSound,
    NSTrackingArea, NSTrackingAreaOptions, NSView, NSWindow, NSWindowCollectionBehavior,
    NSWindowSharingType, NSWindowStyleMask,
};
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotification, NSNotificationCenter, NSPoint, NSRect,
    NSString, NSUserDefaults,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::BackendSelector;
use slint::winit_030::winit::platform::macos::WindowAttributesExtMacOS;
use slint::winit_030::winit::window::{Window, WindowAttributes};

use super::{PlatformEvent, SystemLook};
use crate::geometry::{PhysRect, RoundRect};

/// Niveau de l'île : au-dessus de la barre de menus (`NSMainMenuWindowLevel`
/// vaut 24), sous les menus déroulants.
const ISLAND_LEVEL: isize = 24 + 3;
/// LaunchAgent du démarrage à la connexion.
const AGENT_LABEL: &str = "io.github.yazouv.boringwindows";

// ---------------------------------------------------------------------------
// Instance unique

/// Verrou sur un fichier du dossier de données, gardé jusqu'à la sortie.
pub struct SingleInstance(#[allow(dead_code)] Option<std::fs::File>);

/// `None` si BoringWindows tourne déjà pour cet utilisateur.
pub fn single_instance() -> Option<SingleInstance> {
    let dir = bw_claude::install::data_dir();
    let file = std::fs::create_dir_all(&dir).and_then(|()| {
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join("instance.lock"))
    });
    match file {
        Ok(file) => match file.try_lock() {
            Ok(()) => Some(SingleInstance(Some(file))),
            Err(std::fs::TryLockError::WouldBlock) => None,
            Err(e) => {
                log::warn!("verrou d'instance unique indisponible : {e:?}");
                Some(SingleInstance(None))
            }
        },
        Err(e) => {
            // Pas de verrou possible : on démarre quand même.
            log::warn!("verrou d'instance unique indisponible : {e}");
            Some(SingleInstance(None))
        }
    }
}

// ---------------------------------------------------------------------------
// Fenêtre de l'île

type EventCallback = Box<dyn Fn(PlatformEvent)>;

thread_local! {
    static CREATING_ISLAND: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ISLAND: RefCell<Option<IslandState>> = const { RefCell::new(None) };
    static ON_EVENT: RefCell<Option<EventCallback>> = const { RefCell::new(None) };
    /// Moniteur posé par `watch_user_return`, retiré à la première saisie.
    static RETURN_MONITOR: RefCell<Option<Retained<AnyObject>>> = const { RefCell::new(None) };
}

/// Crée l'île : seules les fenêtres créées dans `f` reçoivent les attributs
/// de l'île (Slint applique le hook à toutes les fenêtres).
pub fn creating_island<R>(f: impl FnOnce() -> R) -> R {
    CREATING_ISLAND.set(true);
    let result = f();
    CREATING_ISLAND.set(false);
    result
}

/// Application « accessoire » : pas d'icône dans le Dock ni dans Cmd+Tab.
pub fn configure_backend(selector: BackendSelector) -> BackendSelector {
    use slint::winit_030::SlintEvent;
    use slint::winit_030::winit::event_loop::EventLoop;
    use slint::winit_030::winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

    let mut builder = EventLoop::<SlintEvent>::with_user_event();
    builder.with_activation_policy(ActivationPolicy::Accessory);
    selector
        .with_winit_event_loop_builder(builder)
        .with_winit_window_attributes_hook(window_attributes)
}

fn window_attributes(attrs: WindowAttributes) -> WindowAttributes {
    if !CREATING_ISLAND.get() {
        return attrs;
    }
    attrs
        .with_active(false)
        .with_transparent(true)
        .with_decorations(false)
        .with_resizable(false)
        .with_has_shadow(false)
}

/// Écran choisi : celui de la barre de menus, ou celui sous la souris.
fn target_screen(mtm: MainThreadMarker, choice: MonitorChoice) -> Option<Retained<NSScreen>> {
    let screens = NSScreen::screens(mtm);
    if choice == MonitorChoice::Cursor {
        let mouse = NSEvent::mouseLocation();
        if let Some(screen) = screens.iter().find(|s| contains(s.frame(), mouse)) {
            return Some(screen);
        }
    }
    screens.firstObject()
}

fn contains(r: NSRect, p: NSPoint) -> bool {
    p.x >= r.origin.x
        && p.x < r.origin.x + r.size.width
        && p.y >= r.origin.y
        && p.y < r.origin.y + r.size.height
}

/// Position physique (repère de winit : origine en haut à gauche de l'écran
/// principal) pour centrer une fenêtre de `size` (logique) en haut de l'écran.
pub fn initial_position(
    choice: MonitorChoice,
    size: (f32, f32),
) -> Option<slint::PhysicalPosition> {
    let mtm = MainThreadMarker::new()?;
    let screen = target_screen(mtm, choice)?;
    let primary = NSScreen::screens(mtm).firstObject()?.frame();
    let frame = screen.frame();
    let scale = screen.backingScaleFactor();
    let x = frame.origin.x + (frame.size.width - f64::from(size.0)) / 2.0;
    // Cocoa compte depuis le bas de l'écran principal, winit depuis le haut.
    let y = primary.size.height - (frame.origin.y + frame.size.height);
    Some(slint::PhysicalPosition::new(
        (x * scale).round() as i32,
        (y * scale).round() as i32,
    ))
}

/// État de l'île, partagé avec les moniteurs d'événements (thread UI).
struct IslandState {
    window: Retained<NSWindow>,
    /// Zone cliquable : pixels physiques depuis le coin haut gauche.
    hit: PhysRect,
    /// La souris est sur la pilule (la fenêtre reçoit les clics).
    inside: bool,
}

pub struct Platform {
    window: Retained<NSWindow>,
    /// Moniteurs des déplacements de la souris (ici et dans les autres apps).
    monitors: Vec<Retained<AnyObject>>,
    /// Abonnement aux changements d'écrans.
    screens_observer: Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    mtm: MainThreadMarker,
}

impl Platform {
    /// À appeler une fois la fenêtre native créée (boucle d'événements active).
    pub fn attach(
        window: &Window,
        on_event: impl Fn(PlatformEvent) + 'static,
    ) -> anyhow::Result<Self> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| anyhow::anyhow!("intégration hors du thread principal"))?;
        let view = match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::AppKit(h)) => h.ns_view,
            _ => anyhow::bail!("fenêtre native introuvable"),
        };
        // SAFETY: la vue de winit vit aussi longtemps que la fenêtre Slint.
        let view: Retained<NSView> = unsafe { Retained::retain(view.as_ptr().cast()) }
            .ok_or_else(|| anyhow::anyhow!("vue native introuvable"))?;
        let ns_window = view
            .window()
            .ok_or_else(|| anyhow::anyhow!("fenêtre native introuvable"))?;

        make_panel(&ns_window);
        ns_window.setLevel(ISLAND_LEVEL);
        ns_window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        ns_window.setHasShadow(false);
        ns_window.setIgnoresMouseEvents(true);

        // Survol sans être la fenêtre active : winit ne reçoit les
        // déplacements que par une zone de suivi « toujours active ».
        // SAFETY: la vue est le propriétaire de la zone et la garde.
        unsafe {
            let area = NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                NSTrackingAreaOptions::MouseEnteredAndExited
                    | NSTrackingAreaOptions::MouseMoved
                    | NSTrackingAreaOptions::ActiveAlways
                    | NSTrackingAreaOptions::InVisibleRect,
                Some(&view),
                None,
            );
            view.addTrackingArea(&area);
        }

        ON_EVENT.with(|cb| *cb.borrow_mut() = Some(Box::new(on_event)));
        ISLAND.with(|s| {
            *s.borrow_mut() = Some(IslandState {
                window: ns_window.clone(),
                hit: PhysRect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
                inside: false,
            });
        });

        let mask = NSEventMask::MouseMoved
            | NSEventMask::LeftMouseDragged
            | NSEventMask::RightMouseDragged
            | NSEventMask::OtherMouseDragged;
        let mut monitors = Vec::new();
        let global = RcBlock::new(|_: NonNull<NSEvent>| update_passthrough());
        if let Some(m) = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &global) {
            monitors.push(m);
        }
        let local = RcBlock::new(|event: NonNull<NSEvent>| {
            update_passthrough();
            event.as_ptr()
        });
        // SAFETY: le bloc rend l'événement tel quel.
        if let Some(m) =
            unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &local) }
        {
            monitors.push(m);
        }

        let changed =
            RcBlock::new(|_: NonNull<NSNotification>| emit(PlatformEvent::DisplayChanged));
        // SAFETY: le bloc n'appelle que le rappel de l'app, sur le thread UI
        // (notification envoyée sur le thread principal).
        let screens_observer = Some(unsafe {
            NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                Some(NSApplicationDidChangeScreenParametersNotification),
                None,
                None,
                &changed,
            )
        });

        Ok(Self {
            window: ns_window,
            monitors,
            screens_observer,
            mtm,
        })
    }

    pub fn place(&self, window: &slint::Window, choice: MonitorChoice, size: (f32, f32)) {
        if let Some(pos) = initial_position(choice, size) {
            window.set_position(pos);
        }
    }

    pub fn set_hit_region(&self, rect: PhysRect) {
        ISLAND.with(|s| {
            if let Some(state) = s.borrow_mut().as_mut() {
                state.hit = rect;
            }
        });
        update_passthrough();
    }

    /// Pas de région arrondie ici : la zone cliquable reste le rectangle.
    pub fn set_round_region(&self, shape: RoundRect) {
        self.set_hit_region(shape.rect);
    }

    /// Le flou de `theme.blur` n'existe que sous Windows.
    pub fn set_blur(&self, _on: bool) {}

    /// L'île n'apparaît pas dans les captures et partages d'écran.
    pub fn set_capture_excluded(&self, excluded: bool) {
        self.window.setSharingType(if excluded {
            NSWindowSharingType::None
        } else {
            NSWindowSharingType::ReadOnly
        });
    }

    /// Prévient (`PlatformEvent::UserReturned`) au prochain mouvement de la
    /// souris ou clic, une seule fois. (Le clavier demanderait l'autorisation
    /// « Accessibilité ».)
    pub fn watch_user_return(&self) {
        if RETURN_MONITOR.with(|m| m.borrow().is_some()) {
            return;
        }
        let block = RcBlock::new(|_: NonNull<NSEvent>| {
            if let Some(monitor) = RETURN_MONITOR.with(|m| m.borrow_mut().take()) {
                // SAFETY: moniteur obtenu d'addGlobalMonitor, retiré une fois.
                unsafe { NSEvent::removeMonitor(&monitor) };
                emit(PlatformEvent::UserReturned);
            }
        });
        let mask = NSEventMask::MouseMoved
            | NSEventMask::LeftMouseDown
            | NSEventMask::RightMouseDown
            | NSEventMask::ScrollWheel
            | NSEventMask::KeyDown;
        let monitor = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &block);
        RETURN_MONITOR.with(|m| *m.borrow_mut() = monitor);
    }

    pub fn set_visible(&self, visible: bool) {
        if visible {
            self.window.orderFrontRegardless();
        } else {
            self.window.orderOut(None);
        }
    }

    /// macOS met les applications plein écran dans leur propre bureau, où
    /// l'île n'apparaît pas : rien à détecter.
    pub fn fullscreen_now(&self) -> bool {
        false
    }

    pub fn foreground_on_our_monitor(&self) -> bool {
        true
    }

    pub fn foreground_is_capture_tool(&self) -> bool {
        false
    }

    /// Hauteur de la barre de menus de l'écran de l'île (encoche comprise),
    /// pour que la pilule fermée s'y loge. `None` si elle se masque.
    pub fn top_taskbar_height(&self) -> Option<f32> {
        let screen = self
            .window
            .screen()
            .or_else(|| NSScreen::mainScreen(self.mtm))?;
        let frame = screen.frame();
        let visible = screen.visibleFrame();
        let bar = (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
        // `safeAreaInsets` (l'encoche) n'existe que depuis macOS 12.
        let notch = if screen.respondsToSelector(sel!(safeAreaInsets)) {
            screen.safeAreaInsets().top
        } else {
            0.0
        };
        let height = bar.max(notch);
        (height >= 1.0).then_some(height as f32)
    }
}

impl Drop for Platform {
    fn drop(&mut self) {
        for monitor in self.monitors.drain(..) {
            // SAFETY: moniteurs obtenus dans `attach`, retirés une fois.
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
        if let Some(observer) = self.screens_observer.take() {
            // SAFETY: observateur obtenu de ce même centre.
            unsafe { NSNotificationCenter::defaultCenter().removeObserver(observer.as_ref()) };
        }
        if let Some(monitor) = RETURN_MONITOR.with(|m| m.borrow_mut().take()) {
            // SAFETY: moniteur obtenu dans `watch_user_return`.
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
        ISLAND.with(|s| s.borrow_mut().take());
        ON_EVENT.with(|cb| cb.borrow_mut().take());
    }
}

fn emit(event: PlatformEvent) {
    ON_EVENT.with(|cb| {
        if let Some(cb) = cb.borrow().as_ref() {
            cb(event);
        }
    });
}

/// La fenêtre ne reçoit la souris que sur la pilule : ailleurs, les clics
/// passent à l'application en dessous. Appelé à chaque mouvement de la souris.
fn update_passthrough() {
    let left = ISLAND.with(|s| {
        let mut state = s.borrow_mut();
        let state = state.as_mut()?;
        let inside = hit_contains(&state.window, state.hit, NSEvent::mouseLocation());
        if inside == state.inside {
            return None;
        }
        state.inside = inside;
        state.window.setIgnoresMouseEvents(!inside);
        Some(!inside)
    });
    // La fenêtre n'a pas vu la souris partir (elle l'ignorait déjà) : on le
    // dit à l'app pour que l'île se referme.
    if left == Some(true) {
        emit(PlatformEvent::PointerLeft);
    }
}

/// `mouse` (coordonnées d'écran Cocoa) est-il dans `hit` (pixels physiques
/// depuis le coin haut gauche de la fenêtre) ?
fn hit_contains(window: &NSWindow, hit: PhysRect, mouse: NSPoint) -> bool {
    if hit.width <= 0 || hit.height <= 0 || !window.isVisible() {
        return false;
    }
    let frame = window.frame();
    let scale = window.backingScaleFactor();
    let left = frame.origin.x + f64::from(hit.x) / scale;
    let top = frame.origin.y + frame.size.height - f64::from(hit.y) / scale;
    let rect = NSRect::new(
        NSPoint::new(left, top - f64::from(hit.height) / scale),
        objc2_foundation::NSSize::new(f64::from(hit.width) / scale, f64::from(hit.height) / scale),
    );
    contains(rect, mouse)
}

/// Transforme la fenêtre de winit en panneau non activant : un clic sur
/// l'île ne retire pas le focus à l'application en cours.
fn make_panel(window: &NSWindow) {
    let Some(class) = panel_class() else {
        log::warn!("panneau non activant indisponible : l'île prendra le focus au clic");
        return;
    };
    let current = window.class();
    // La fenêtre est allouée avec la taille de sa classe : la nouvelle ne
    // doit pas en demander plus.
    if class.instance_size() > current.instance_size() {
        log::warn!("panneau non activant : classe incompatible, ignoré");
        return;
    }
    // SAFETY: la classe dérive de NSPanel (donc de NSWindow), sans variable
    // d'instance, et n'est pas plus grande que la classe d'origine ; winit
    // n'ajoute à NSWindow que canBecomeKeyWindow/canBecomeMainWindow,
    // redéfinies ici.
    unsafe { AnyObject::set_class(window.as_ref(), class) };
    window.setStyleMask(window.styleMask() | NSWindowStyleMask::NonactivatingPanel);
    if let Some(panel) = window.downcast_ref::<NSPanel>() {
        panel.setFloatingPanel(true);
        panel.setBecomesKeyOnlyIfNeeded(true);
        panel.setWorksWhenModal(true);
    }
    // Un panneau se cache par défaut quand l'app passe en arrière-plan.
    window.setHidesOnDeactivate(false);
}

fn panel_class() -> Option<&'static AnyClass> {
    static CLASS: std::sync::OnceLock<Option<&'static AnyClass>> = std::sync::OnceLock::new();
    *CLASS.get_or_init(|| {
        extern "C-unwind" fn no(_: &AnyObject, _: Sel) -> Bool {
            Bool::NO
        }
        let mut builder = ClassBuilder::new(c"BoringWindowsIslandPanel", NSPanel::class())?;
        // SAFETY: signatures conformes à `- (BOOL)canBecomeKeyWindow` et
        // `- (BOOL)canBecomeMainWindow`.
        unsafe {
            builder.add_method(
                sel!(canBecomeKeyWindow),
                no as extern "C-unwind" fn(_, _) -> _,
            );
            builder.add_method(
                sel!(canBecomeMainWindow),
                no as extern "C-unwind" fn(_, _) -> _,
            );
        }
        Some(builder.register())
    })
}

// ---------------------------------------------------------------------------
// Démarrage à la connexion (LaunchAgent)

fn agent_path() -> Option<PathBuf> {
    Some(
        dirs_home()?
            .join("Library/LaunchAgents")
            .join(format!("{AGENT_LABEL}.plist")),
    )
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub fn autostart_enabled() -> bool {
    agent_path().is_some_and(|p| p.exists())
}

pub fn set_autostart(enabled: bool) -> anyhow::Result<()> {
    let path = agent_path().ok_or_else(|| anyhow::anyhow!("dossier personnel introuvable"))?;
    if !enabled {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        };
    }
    let exe = std::env::current_exe()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, launch_agent(&exe))?;
    Ok(())
}

/// Plist du LaunchAgent qui lance `exe` à l'ouverture de session.
fn launch_agent(exe: &Path) -> String {
    let exe = xml_escape(&exe.to_string_lossy());
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{AGENT_LABEL}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{exe}</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>ProcessType</key>
	<string>Interactive</string>
	<key>LimitLoadToSessionType</key>
	<string>Aqua</string>
</dict>
</plist>
"#
    )
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// Fenêtres et applications

/// Boîte de dialogue OK / Annuler.
pub fn confirm(title: &str, text: &str) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        log::info!("{title} : {text}");
        return true;
    };
    bring_app_forward(mtm);
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(text));
    alert.addButtonWithTitle(&NSString::from_str(&bw_i18n::tr!("OK", "OK")));
    alert.addButtonWithTitle(&NSString::from_str(&bw_i18n::tr!("Cancel", "Annuler")));
    alert.runModal() == NSAlertFirstButtonReturn
}

/// Une application « accessoire » ne passe pas devant toute seule : sans
/// cela, la boîte de dialogue s'ouvrirait derrière la fenêtre en cours.
fn bring_app_forward(mtm: MainThreadMarker) {
    #[allow(deprecated)]
    NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
}

/// Passe l'app au premier plan (fenêtre de réglages).
pub fn activate_app() {
    if let Some(mtm) = MainThreadMarker::new() {
        bring_app_forward(mtm);
    }
}

/// Ramène au premier plan l'application (Terminal, iTerm, VS Code…) dont
/// descend la session Claude.
pub fn focus_terminal(ancestors: &[u32], _console_window: Option<i64>) -> bool {
    for &pid in ancestors {
        let Ok(pid) = i32::try_from(pid) else {
            continue;
        };
        let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else {
            continue;
        };
        if app.activationPolicy() != NSApplicationActivationPolicy::Regular {
            continue;
        }
        // `open -b` passe par LaunchServices : il ramène l'app devant même
        // depuis une app en arrière-plan.
        if let Some(bundle) = app.bundleIdentifier() {
            return std::process::Command::new("/usr/bin/open")
                .args(["-b", &bundle.to_string()])
                .spawn()
                .is_ok();
        }
    }
    false
}

/// Rejouer une notification : Windows uniquement.
pub fn open_notification(_lines: &[String]) -> bool {
    false
}

/// Ouvrir l'application d'une notification : Windows uniquement.
pub fn open_app(_app_id: &str) -> bool {
    false
}

pub fn attach_parent_console() {}

/// Sélecteur de fichier `.ics`.
pub fn pick_ics_file() -> Option<PathBuf> {
    let mtm = MainThreadMarker::new()?;
    bring_app_forward(mtm);
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(false);
    panel.setAllowsMultipleSelection(false);
    if panel.runModal() != NSModalResponseOK {
        return None;
    }
    let url = panel.URL()?;
    Some(PathBuf::from(url.path()?.to_string()))
}

/// Son quand Claude se met à attendre.
pub fn alert_sound() {
    match NSSound::soundNamed(&NSString::from_str("Glass")) {
        Some(sound) => {
            sound.play();
        }
        None => objc2_app_kit::NSBeep(),
    }
}

/// Ouvre un fichier ou un lien avec l'application associée.
pub fn open_path(path: &Path) {
    if let Err(e) = std::process::Command::new("/usr/bin/open")
        .arg(path)
        .spawn()
    {
        log::warn!("impossible d'ouvrir {} : {e}", path.display());
    }
}

/// Rouvre une conversation Claude Code : `claude --resume <id>` dans une
/// nouvelle fenêtre du Terminal, sur le dossier de la session. L'identifiant
/// est validé et le dossier passé entre apostrophes.
pub fn resume_claude_session(cwd: &Path, session_id: &str) -> bool {
    if !bw_claude::is_session_id(session_id) || !cwd.is_dir() {
        log::warn!("reprise de session refusée (id ou dossier invalide)");
        return false;
    }
    let command = format!(
        "cd {} && claude --resume {session_id}",
        shell_quote(&cwd.to_string_lossy())
    );
    let script = format!(
        "tell application \"Terminal\"\n\tactivate\n\tdo script \"{}\"\nend tell",
        applescript_escape(&command)
    );
    std::process::Command::new("/usr/bin/osascript")
        .args(["-e", &script])
        .spawn()
        .map_err(|e| log::warn!("impossible de rouvrir la session : {e}"))
        .is_ok()
}

/// Argument shell entre apostrophes.
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// Texte entre guillemets dans un AppleScript.
fn applescript_escape(text: &str) -> String {
    text.replace('\\', r"\\").replace('"', "\\\"")
}

// ---------------------------------------------------------------------------
// Apparence

/// Mode clair ou sombre, et couleur d'accent du système.
pub fn system_look() -> SystemLook {
    let style = NSUserDefaults::standardUserDefaults()
        .stringForKey(&NSString::from_str("AppleInterfaceStyle"));
    let dark = style.is_some_and(|s| s.to_string().eq_ignore_ascii_case("dark"));
    SystemLook {
        light: !dark,
        accent: accent_color(),
    }
}

fn accent_color() -> Option<[u8; 3]> {
    let color =
        NSColor::controlAccentColor().colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())?;
    let channel = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some([
        channel(color.redComponent()),
        channel(color.greenComponent()),
        channel(color.blueComponent()),
    ])
}

/// Abonnement aux changements de mode (clair/sombre) et d'accent.
pub struct LookWatcher {
    observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
}

impl Drop for LookWatcher {
    fn drop(&mut self) {
        let center = NSDistributedNotificationCenter::defaultCenter();
        for observer in self.observers.drain(..) {
            // SAFETY: observateur obtenu de ce même centre.
            unsafe { center.removeObserver(observer.as_ref()) };
        }
    }
}

pub fn watch_system_look(on_change: impl Fn() + Send + 'static) -> Option<LookWatcher> {
    let on_change = std::rc::Rc::new(on_change);
    let center = NSDistributedNotificationCenter::defaultCenter();
    let observers = [
        "AppleInterfaceThemeChangedNotification",
        "AppleColorPreferencesChangedNotification",
    ]
    .into_iter()
    .map(|name| {
        let on_change = on_change.clone();
        let block = RcBlock::new(move |_: NonNull<NSNotification>| on_change());
        // SAFETY: inscrit depuis le thread UI, dont la boucle livre la
        // notification : le bloc n'en sort pas.
        unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(&NSString::from_str(name)),
                None,
                None,
                &block,
            )
        }
    })
    .collect();
    Some(LookWatcher { observers })
}

/// Temps écoulé depuis la dernière saisie au clavier ou à la souris.
pub fn idle_for() -> std::time::Duration {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    }
    /// `kCGEventSourceStateCombinedSessionState`, `kCGAnyInputEventType`.
    const COMBINED_SESSION: i32 = 0;
    const ANY_INPUT: u32 = u32::MAX;
    // SAFETY: fonction sans pointeur, constantes documentées.
    let secs = unsafe { CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION, ANY_INPUT) };
    if secs.is_finite() && secs >= 0.0 {
        std::time::Duration::from_secs_f64(secs)
    } else {
        std::time::Duration::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_agent_and_quoting() {
        let plist = launch_agent(Path::new("/Applications/Boring & Co/boringwindows"));
        assert!(plist.contains("<string>/Applications/Boring &amp; Co/boringwindows</string>"));
        assert!(plist.contains(AGENT_LABEL));
        assert_eq!(shell_quote("/Users/a/it's"), r"'/Users/a/it'\''s'");
        assert_eq!(applescript_escape(r#"say "hi" \o/"#), r#"say \"hi\" \\o/"#);
    }
}

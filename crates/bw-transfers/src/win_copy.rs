//! Copies, déplacements et suppressions de l'Explorateur.
//!
//! La boîte de progression est une fenêtre `OperationStatusWindow` dont le
//! titre porte le pourcentage. Un hook (WinEvent) sur l'apparition, le
//! changement de titre et la disparition des fenêtres la suit sans rien
//! interroger. Le libellé (« Copie d'un élément de A vers B ») vient de
//! l'arbre UI Automation de la fenêtre, lu une fois par fenêtre.
//!
//! Le hook demande une boucle de messages : thread dédié, arrêté par `WM_QUIT`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::mpsc::channel;
use std::thread::JoinHandle;

use tokio::sync::mpsc::UnboundedSender;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, HWINEVENTHOOK, IUIAutomation, SetWinEventHook, TreeScope_Descendants,
    UnhookWinEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, EnumWindows, GetClassNameW, GetMessageW, GetWindowTextW, IsWindowVisible,
    MSG, PostThreadMessageW, TranslateMessage, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
    WM_QUIT,
};
use windows::core::BOOL;

use crate::{CopyEvent, operation_label, parse_percent};

/// Classe de la fenêtre de progression de l'Explorateur.
const STATUS_CLASS: &str = "OperationStatusWindow";
/// Classe de la « tuile » qui porte le libellé de l'opération.
const TILE_CLASS: &str = "OperationTileDialog";
/// Tentatives de lecture du libellé (la tuile peut arriver après la fenêtre).
const LABEL_TRIES: u32 = 4;

const EVENT_OBJECT_DESTROY: u32 = 0x8001;
const EVENT_OBJECT_SHOW: u32 = 0x8002;
const EVENT_OBJECT_HIDE: u32 = 0x8003;
const EVENT_OBJECT_NAMECHANGE: u32 = 0x800C;
const OBJID_WINDOW: i32 = 0;

/// Destinataire des événements du hook (les fonctions de rappel n'ont pas de
/// contexte). Remplacé à chaque démarrage du module.
static SINK: Mutex<Option<UnboundedSender<CopyEvent>>> = Mutex::new(None);

struct Known {
    label: Option<String>,
    tries: u32,
}

thread_local! {
    /// Fenêtres de progression vues par ce thread.
    static KNOWN: RefCell<HashMap<isize, Known>> = RefCell::new(HashMap::new());
    static UIA: RefCell<Option<IUIAutomation>> = const { RefCell::new(None) };
}

/// Garde le thread du hook en vie ; l'arrête à la destruction.
pub(crate) struct Watcher {
    thread_id: u32,
    join: Option<JoinHandle<()>>,
}

impl Watcher {
    pub(crate) fn spawn(tx: UnboundedSender<CopyEvent>) -> Option<Self> {
        *SINK.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
        let (id_tx, id_rx) = channel();
        let join = std::thread::Builder::new()
            .name("transfers-copy".into())
            .spawn(move || run(&id_tx))
            .ok()?;
        let thread_id = id_rx.recv().ok()?;
        Some(Self {
            thread_id,
            join: Some(join),
        })
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        // SAFETY: message posté au thread du hook, qui sort de sa boucle.
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        *SINK.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

fn run(id_tx: &std::sync::mpsc::Sender<u32>) {
    // SAFETY: initialisation COM du thread (UI Automation), sans effet si déjà faite.
    let uia = unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()
    };
    if uia.is_none() {
        log::info!("transferts : UI Automation indisponible, copies sans libellé");
    }
    UIA.with(|u| *u.borrow_mut() = uia);

    // SAFETY: `hook_proc` est une fonction `extern "system"` valide ; le hook
    // est appelé sur ce thread par sa boucle de messages.
    let hooks: Vec<HWINEVENTHOOK> = unsafe {
        let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
        [
            (EVENT_OBJECT_SHOW, EVENT_OBJECT_HIDE),
            (EVENT_OBJECT_DESTROY, EVENT_OBJECT_DESTROY),
            (EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_NAMECHANGE),
        ]
        .into_iter()
        .map(|(min, max)| SetWinEventHook(min, max, None, Some(hook_proc), 0, 0, flags))
        .filter(|h| !h.is_invalid())
        .collect()
    };
    if hooks.len() < 3 {
        log::warn!("transferts : hook des fenêtres refusé, copies non suivies");
    }

    // Une copie déjà en cours au lancement.
    // SAFETY: rappel appelé de façon synchrone, sans pointeur conservé.
    unsafe {
        let _ = EnumWindows(Some(scan_proc), LPARAM(0));
    }

    // SAFETY: id du thread courant, lu par le thread qui nous a créé.
    let _ = id_tx.send(unsafe { GetCurrentThreadId() });

    let mut msg = MSG::default();
    // SAFETY: boucle de messages classique, arrêtée par WM_QUIT.
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        for hook in hooks {
            let _ = UnhookWinEvent(hook);
        }
    }
    KNOWN.with(|k| k.borrow_mut().clear());
    UIA.with(|u| *u.borrow_mut() = None);
}

unsafe extern "system" fn scan_proc(hwnd: HWND, _: LPARAM) -> BOOL {
    // SAFETY: fenêtre fournie par EnumWindows.
    if class_name(hwnd).as_deref() == Some(STATUS_CLASS)
        && unsafe { IsWindowVisible(hwnd) }.as_bool()
    {
        update(hwnd);
    }
    BOOL(1)
}

unsafe extern "system" fn hook_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if id_object != OBJID_WINDOW || hwnd.is_invalid() {
        return;
    }
    let window = hwnd.0 as isize;
    match event {
        EVENT_OBJECT_SHOW | EVENT_OBJECT_NAMECHANGE
            if class_name(hwnd).as_deref() == Some(STATUS_CLASS) =>
        {
            update(hwnd);
        }
        // La fenêtre n'a plus de classe lisible : on s'en remet à ce qu'on en sait.
        EVENT_OBJECT_HIDE | EVENT_OBJECT_DESTROY
            if KNOWN.with(|k| k.borrow_mut().remove(&window)).is_some() =>
        {
            send(CopyEvent::Gone { window });
        }
        _ => {}
    }
}

fn send(event: CopyEvent) {
    if let Some(tx) = SINK.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        let _ = tx.send(event);
    }
}

/// Relit le titre (et, au besoin, le libellé) de la fenêtre et le signale.
fn update(hwnd: HWND) {
    let window = hwnd.0 as isize;
    let percent = parse_percent(&window_text(hwnd));
    let label = KNOWN.with(|k| {
        let mut known = k.borrow_mut();
        let entry = known.entry(window).or_insert(Known {
            label: None,
            tries: 0,
        });
        if entry.label.is_none() && entry.tries < LABEL_TRIES {
            entry.tries += 1;
            entry.label = tile_label(hwnd);
        }
        entry.label.clone()
    });
    send(CopyEvent::Update {
        window,
        percent,
        label,
    });
}

/// Libellé de l'opération, lu dans l'arbre UI Automation de la fenêtre.
fn tile_label(hwnd: HWND) -> Option<String> {
    UIA.with(|u| {
        let uia = u.borrow();
        let uia = uia.as_ref()?;
        // SAFETY: appels COM sur des objets valides ; la fenêtre peut disparaître
        // entre-temps, ce qui donne une erreur et non un accès invalide.
        unsafe {
            let root = uia.ElementFromHandle(hwnd).ok()?;
            let items = root
                .FindAll(TreeScope_Descendants, &uia.CreateTrueCondition().ok()?)
                .ok()?;
            for i in 0..items.Length().ok()? {
                let item = items.GetElement(i).ok()?;
                if item.CurrentClassName().ok()? == TILE_CLASS {
                    return operation_label(&item.CurrentName().ok()?.to_string());
                }
            }
        }
        None
    })
}

fn class_name(hwnd: HWND) -> Option<String> {
    let mut buffer = [0u16; 64];
    // SAFETY: tampon local.
    let len = unsafe { GetClassNameW(hwnd, &mut buffer) };
    (len > 0).then(|| String::from_utf16_lossy(&buffer[..len as usize]))
}

fn window_text(hwnd: HWND) -> String {
    let mut buffer = [0u16; 256];
    // SAFETY: tampon local.
    let len = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    String::from_utf16_lossy(&buffer[..len.max(0) as usize])
}

//! Intégration Win32 : style de la fenêtre, zone cliquable, placement,
//! détection du plein écran, démarrage automatique, instance unique.

use std::cell::RefCell;
use std::path::Path;

use bw_config::MonitorChoice;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::BackendSelector;
use slint::winit_030::winit::platform::windows::WindowAttributesExtWindows;
use slint::winit_030::winit::window::{Window, WindowAttributes};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, GetLastError, HANDLE, HWND, LPARAM,
    LRESULT, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    CreateRectRgn, GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY,
    MONITORINFO, MonitorFromPoint, MonitorFromRect, MonitorFromWindow, SetWindowRgn,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows::Win32::System::Threading::{
    CreateMutexW, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Shell::{
    ABE_TOP, ABM_GETSTATE, ABM_GETTASKBARPOS, ABM_NEW, ABM_REMOVE, ABN_FULLSCREENAPP,
    ABN_POSCHANGED, ABS_AUTOHIDE, APPBARDATA, QUNS_BUSY, QUNS_PRESENTATION_MODE,
    QUNS_RUNNING_D3D_FULL_SCREEN, SHAppBarMessage, SHQueryUserNotificationState, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, EVENT_SYSTEM_FOREGROUND, GWL_EXSTYLE,
    GetCursorPos, GetForegroundWindow, GetWindowLongPtrW, GetWindowThreadProcessId, HWND_TOPMOST,
    RegisterClassW, RegisterWindowMessageW, SW_HIDE, SW_SHOWNOACTIVATE, SW_SHOWNORMAL,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_APP,
    WM_DISPLAYCHANGE, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{HSTRING, PCWSTR, w};

use super::PlatformEvent;
use crate::geometry::PhysRect;

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_VALUE: PCWSTR = w!("BoringWindows");
const APPBAR_CALLBACK: u32 = WM_APP + 1;

// ---------------------------------------------------------------------------
// Instance unique

pub struct SingleInstance(HANDLE);

impl Drop for SingleInstance {
    fn drop(&mut self) {
        // SAFETY: handle obtenu de CreateMutexW, fermé une seule fois.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// `None` si BoringWindows tourne déjà pour cet utilisateur.
pub fn single_instance() -> Option<SingleInstance> {
    // SAFETY: appel Win32 sans pointeur fourni par nous hormis le nom statique.
    let handle = unsafe { CreateMutexW(None, false, w!("Local\\BoringWindows.SingleInstance")) };
    match handle {
        Ok(h) if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS => {
            drop(SingleInstance(h));
            None
        }
        Ok(h) => Some(SingleInstance(h)),
        Err(e) => {
            // Pas de verrou possible : on démarre quand même.
            log::warn!("verrou d'instance unique indisponible : {e}");
            Some(SingleInstance(HANDLE::default()))
        }
    }
}

// ---------------------------------------------------------------------------
// Fenêtre de l'île

thread_local! {
    static CREATING_ISLAND: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Crée l'île : seules les fenêtres créées dans `f` reçoivent les attributs
/// de l'île (Slint applique le hook à toutes les fenêtres).
pub fn creating_island<R>(f: impl FnOnce() -> R) -> R {
    CREATING_ISLAND.set(true);
    let result = f();
    CREATING_ISLAND.set(false);
    result
}

pub fn configure_backend(selector: BackendSelector) -> BackendSelector {
    selector.with_winit_window_attributes_hook(window_attributes)
}

/// Attributs appliqués dès la création : la fenêtre n'apparaît jamais dans la
/// barre des tâches et ne prend pas le focus en s'affichant.
fn window_attributes(attrs: WindowAttributes) -> WindowAttributes {
    if !CREATING_ISLAND.get() {
        return attrs;
    }
    attrs
        .with_skip_taskbar(true)
        .with_active(false)
        .with_transparent(true)
        .with_decorations(false)
        .with_resizable(false)
}

struct Monitor {
    rect: RECT,
    scale: f32,
}

fn target_monitor(choice: MonitorChoice) -> Monitor {
    let mut point = POINT::default();
    // SAFETY: appels Win32 sur des structures locales valides.
    unsafe {
        let hmon = match choice {
            MonitorChoice::Cursor if GetCursorPos(&mut point).is_ok() => {
                MonitorFromPoint(point, MONITOR_DEFAULTTOPRIMARY)
            }
            _ => MonitorFromPoint(POINT::default(), MONITOR_DEFAULTTOPRIMARY),
        };
        monitor_info(hmon)
    }
}

unsafe fn monitor_info(hmon: HMONITOR) -> Monitor {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let (mut dpi_x, mut dpi_y) = (96, 96);
    // SAFETY: `info` et les DPI sont des sorties locales valides.
    unsafe {
        let _ = GetMonitorInfoW(hmon, &mut info);
        if GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_err() {
            dpi_x = 96;
        }
    }
    Monitor {
        rect: info.rcMonitor,
        scale: dpi_x as f32 / 96.0,
    }
}

/// Position physique pour centrer une fenêtre de `size` (logique) en haut de l'écran.
pub fn initial_position(
    choice: MonitorChoice,
    size: (f32, f32),
) -> Option<slint::PhysicalPosition> {
    let m = target_monitor(choice);
    let width = (size.0 * m.scale).round() as i32;
    let monitor_width = m.rect.right - m.rect.left;
    Some(slint::PhysicalPosition::new(
        m.rect.left + (monitor_width - width) / 2,
        m.rect.top,
    ))
}

type EventCallback = Box<dyn Fn(PlatformEvent)>;

thread_local! {
    static ON_EVENT: RefCell<Option<EventCallback>> = const { RefCell::new(None) };
    /// Fenêtre de l'île, pour la remettre au premier plan depuis le hook.
    static ISLAND: std::cell::Cell<Option<HWND>> = const { std::cell::Cell::new(None) };
}

/// Outils de capture d'écran : leur calque couvre tout l'écran et le shell
/// le signale comme une application plein écran. L'île reste affichée.
const CAPTURE_TOOLS: &[&str] = &[
    "screenclippinghost.exe",
    "snippingtool.exe",
    "screensketch.exe",
    "sharex.exe",
    "greenshot.exe",
    "lightshot.exe",
    "flameshot.exe",
];

pub struct Platform {
    hwnd: HWND,
    /// Fenêtre cachée qui reçoit les notifications du shell (appbar).
    helper: HWND,
    /// Hook des changements de fenêtre au premier plan.
    foreground_hook: HWINEVENTHOOK,
}

impl Platform {
    /// À appeler une fois la fenêtre native créée (boucle d'événements active).
    pub fn attach(
        window: &Window,
        on_event: impl Fn(PlatformEvent) + 'static,
    ) -> anyhow::Result<Self> {
        let hwnd = match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => HWND(h.hwnd.get() as *mut _),
            _ => anyhow::bail!("fenêtre native introuvable"),
        };

        // SAFETY: `hwnd` est la fenêtre vivante de l'île, sur son thread.
        unsafe {
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            // « Toujours au premier plan » vient de `always-on-top` côté Slint.
            let wanted = (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE).0 as isize;
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | wanted);
            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            )?;
        }

        ON_EVENT.with(|cb| *cb.borrow_mut() = Some(Box::new(on_event)));
        ISLAND.set(Some(hwnd));
        let helper = create_helper_window()?;
        // SAFETY: `foreground_proc` est une fonction `extern "system"` valide ;
        // hors contexte, elle est appelée sur ce thread via sa boucle de messages.
        let foreground_hook = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(foreground_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };
        if foreground_hook.is_invalid() {
            log::warn!("hook du premier plan refusé : l'île peut passer sous la barre des tâches");
        }
        Ok(Self {
            hwnd,
            helper,
            foreground_hook,
        })
    }

    pub fn place(&self, window: &slint::Window, choice: MonitorChoice, size: (f32, f32)) {
        if let Some(pos) = initial_position(choice, size) {
            window.set_position(pos);
        }
    }

    pub fn set_hit_region(&self, r: PhysRect) {
        // SAFETY: la région appartient au système après un SetWindowRgn réussi.
        unsafe {
            let rgn = CreateRectRgn(r.x, r.y, r.x + r.width, r.y + r.height);
            SetWindowRgn(self.hwnd, Some(rgn), true);
        }
    }

    pub fn set_visible(&self, visible: bool) {
        // SAFETY: fenêtre vivante, appel sur son thread.
        unsafe {
            let _ = ShowWindow(self.hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
        }
    }

    /// Plein écran au lancement (les changements arrivent ensuite par événement).
    pub fn fullscreen_now(&self) -> bool {
        // SAFETY: appel sans argument.
        let state = unsafe { SHQueryUserNotificationState() };
        matches!(
            state,
            Ok(QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE)
        ) && self.foreground_on_our_monitor()
            && !self.foreground_is_capture_tool()
    }

    /// Hauteur logique de la barre des tâches si elle est collée en haut de
    /// l'écran de l'île et toujours visible (pas en masquage automatique).
    pub fn top_taskbar_height(&self) -> Option<f32> {
        let mut data = APPBARDATA {
            cbSize: size_of::<APPBARDATA>() as u32,
            ..Default::default()
        };
        // SAFETY: `data` est une structure locale valide pendant les appels.
        unsafe {
            if SHAppBarMessage(ABM_GETSTATE, &mut data) as u32 & ABS_AUTOHIDE != 0
                || SHAppBarMessage(ABM_GETTASKBARPOS, &mut data) == 0
                || data.uEdge != ABE_TOP
            {
                return None;
            }
            let ours = MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST);
            if MonitorFromRect(&data.rc, MONITOR_DEFAULTTONEAREST) != ours {
                return None;
            }
            let scale = monitor_info(ours).scale;
            Some((data.rc.bottom - data.rc.top) as f32 / scale)
        }
    }

    /// La fenêtre au premier plan est-elle un outil de capture d'écran ?
    pub fn foreground_is_capture_tool(&self) -> bool {
        // SAFETY: fonction sans effet de bord.
        let fg = unsafe { GetForegroundWindow() };
        process_name(fg).is_some_and(|name| CAPTURE_TOOLS.contains(&name.as_str()))
    }

    /// La fenêtre au premier plan est-elle sur le même écran que l'île ?
    pub fn foreground_on_our_monitor(&self) -> bool {
        // SAFETY: fonctions sans effet de bord.
        unsafe {
            let fg = GetForegroundWindow();
            fg.is_invalid()
                || MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST)
                    == MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST)
        }
    }
}

impl Drop for Platform {
    fn drop(&mut self) {
        ON_EVENT.with(|cb| cb.borrow_mut().take());
        ISLAND.set(None);
        let mut data = appbar_data(self.helper);
        // SAFETY: désenregistre l'appbar créée dans `create_helper_window` et
        // le hook posé dans `attach`.
        unsafe {
            if !self.foreground_hook.is_invalid() {
                let _ = UnhookWinEvent(self.foreground_hook);
            }
            SHAppBarMessage(ABM_REMOVE, &mut data);
            let _ = DestroyWindow(self.helper);
        }
    }
}

/// Nom de l'exécutable (en minuscules) qui possède `hwnd`.
fn process_name(hwnd: HWND) -> Option<String> {
    if hwnd.is_invalid() {
        return None;
    }
    let mut pid = 0;
    // SAFETY: `pid` est une sortie locale ; le handle du processus est fermé
    // après lecture, et le tampon vit pendant l'appel.
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut len = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(process);
        result.ok()?;
        let path = String::from_utf16_lossy(&buffer[..len as usize]);
        let name = path.rsplit('\\').next()?;
        Some(name.to_ascii_lowercase())
    }
}

/// Changement de fenêtre au premier plan. La barre des tâches, une fois
/// activée, repasse au-dessus des autres fenêtres « topmost » : on remet
/// l'île au sommet de cette pile.
unsafe extern "system" fn foreground_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if let Some(island) = ISLAND.get() {
        // SAFETY: fenêtre vivante tant que `ISLAND` est renseigné ; sans
        // SWP_SHOWWINDOW, une île masquée le reste.
        unsafe {
            let _ = SetWindowPos(
                island,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
    ON_EVENT.with(|cb| {
        if let Some(cb) = cb.borrow().as_ref() {
            cb(PlatformEvent::Foreground);
        }
    });
}

fn appbar_data(hwnd: HWND) -> APPBARDATA {
    APPBARDATA {
        cbSize: size_of::<APPBARDATA>() as u32,
        hWnd: hwnd,
        uCallbackMessage: APPBAR_CALLBACK,
        ..Default::default()
    }
}

/// Crée une fenêtre invisible enregistrée comme « appbar » : le shell lui
/// signale l'entrée/sortie du plein écran (`ABN_FULLSCREENAPP`) sans aucun
/// polling. Elle reçoit aussi `WM_DISPLAYCHANGE`.
///
/// Rien n'est réservé à l'écran : on n'appelle jamais `ABM_SETPOS`.
fn create_helper_window() -> anyhow::Result<HWND> {
    let class = w!("BoringWindows.Helper");
    // SAFETY: classe et fenêtre créées avec des chaînes statiques ; la
    // procédure de fenêtre est une fonction `extern "system"` valide.
    unsafe {
        let instance = GetModuleHandleW(None)?.into();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(helper_proc),
            hInstance: instance,
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class,
            w!("BoringWindows"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance),
            None,
        )?;
        let mut data = appbar_data(hwnd);
        if SHAppBarMessage(ABM_NEW, &mut data) == 0 {
            log::warn!("enregistrement appbar refusé : pas de détection du plein écran");
        }
        Ok(hwnd)
    }
}

/// Message diffusé quand l'Explorateur (re)crée la barre des tâches.
fn taskbar_created() -> u32 {
    static MESSAGE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    // SAFETY: chaîne statique.
    *MESSAGE.get_or_init(|| unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) })
}

unsafe extern "system" fn helper_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let event = match msg {
        APPBAR_CALLBACK if wparam.0 as u32 == ABN_FULLSCREENAPP => {
            Some(PlatformEvent::Fullscreen(lparam.0 != 0))
        }
        APPBAR_CALLBACK if wparam.0 as u32 == ABN_POSCHANGED => Some(PlatformEvent::TaskbarChanged),
        WM_DISPLAYCHANGE => Some(PlatformEvent::DisplayChanged),
        // Explorateur redémarré : l'appbar est à réenregistrer.
        m if m != 0 && m == taskbar_created() => {
            let mut data = appbar_data(hwnd);
            // SAFETY: `hwnd` est notre fenêtre auxiliaire, vivante.
            unsafe { SHAppBarMessage(ABM_NEW, &mut data) };
            Some(PlatformEvent::TaskbarChanged)
        }
        _ => None,
    };
    if let Some(event) = event {
        ON_EVENT.with(|cb| {
            if let Some(cb) = cb.borrow().as_ref() {
                cb(event);
            }
        });
    }
    // SAFETY: transmet les paramètres reçus tels quels.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

// ---------------------------------------------------------------------------
// Démarrage avec Windows (HKCU\...\Run)

pub fn autostart_enabled() -> bool {
    // SAFETY: lecture de la taille seule, aucun tampon fourni.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            RUN_VALUE,
            RRF_RT_REG_SZ,
            None,
            None,
            None,
        )
    };
    status.is_ok()
}

pub fn set_autostart(enabled: bool) -> anyhow::Result<()> {
    if enabled {
        let exe = std::env::current_exe()?;
        let value: Vec<u16> = format!("\"{}\"", exe.display())
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // SAFETY: `value` est une chaîne UTF-16 terminée par zéro, taille en octets.
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                RUN_VALUE,
                REG_SZ.0,
                Some(value.as_ptr().cast()),
                (value.len() * 2) as u32,
            )
        }
        .ok()?;
    } else {
        // SAFETY: suppression d'une valeur nommée statiquement.
        let status = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE) };
        if status != ERROR_FILE_NOT_FOUND {
            status.ok()?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------

/// Ouvre un fichier avec l'application associée.
pub fn open_path(path: &Path) {
    let path = HSTRING::from(path.as_os_str());
    // SAFETY: chaînes valides pour la durée de l'appel.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &path,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        log::warn!("impossible d'ouvrir {path}");
    }
}

/// Rejoue la notification aux textes `lines` depuis le centre de
/// notifications (bon salon, bon onglet…). Bloquant : hors du thread UI.
pub fn open_notification(lines: &[String]) -> bool {
    bw_notify::open_from_center(lines)
}

/// Ouvre (ou ramène) l'application d'identifiant `app_id` (AppUserModelID,
/// lu dans une notification) via le dossier virtuel des applications.
pub fn open_app(app_id: &str) -> bool {
    // L'identifiant vient du système, mais on refuse tout ce qui pourrait
    // sortir du chemin `shell:AppsFolder\…`.
    if app_id.is_empty() || app_id.len() > 512 || app_id.chars().any(|c| c.is_control() || c == '"')
    {
        return false;
    }
    let target = HSTRING::from(format!("shell:AppsFolder\\{app_id}"));
    // SAFETY: chaînes valides pour la durée de l'appel.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &target,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    result.0 as isize > 32
}

// ---------------------------------------------------------------------------
// Boîte de confirmation et terminal des sessions Claude

/// Question Oui/Non modale.
pub fn confirm(title: &str, text: &str) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        IDYES, MB_ICONQUESTION, MB_SETFOREGROUND, MB_TOPMOST, MB_YESNO, MessageBoxW,
    };
    // SAFETY: chaînes valides pendant l'appel, pas de fenêtre parente.
    let answer = unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(text),
            &HSTRING::from(title),
            MB_YESNO | MB_ICONQUESTION | MB_TOPMOST | MB_SETFOREGROUND,
        )
    };
    answer == IDYES
}

/// Ramène au premier plan le terminal qui héberge une session Claude :
/// d'abord via la console du relais (Windows Terminal, conhost), sinon en
/// remontant les processus parents jusqu'à une fenêtre visible (VS Code…).
pub fn focus_terminal(ancestors: &[u32], console_window: Option<i64>) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GA_ROOTOWNER, GetAncestor, IsWindow};

    if let Some(raw) = console_window {
        let hwnd = HWND(raw as isize as *mut _);
        // SAFETY: on vérifie que la fenêtre existe encore avant de l'utiliser.
        let root = unsafe {
            if IsWindow(Some(hwnd)).as_bool() {
                GetAncestor(hwnd, GA_ROOTOWNER)
            } else {
                HWND::default()
            }
        };
        if is_main_window(root) {
            return activate(root);
        }
    }
    ancestors
        .iter()
        .find_map(|&pid| main_window_of(pid))
        .is_some_and(activate)
}

fn is_main_window(hwnd: HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        GW_OWNER, GetWindow, GetWindowTextLengthW, IsWindowVisible,
    };
    // SAFETY: fonctions de lecture sur un handle éventuellement nul.
    unsafe {
        !hwnd.is_invalid()
            && IsWindowVisible(hwnd).as_bool()
            && GetWindow(hwnd, GW_OWNER).is_err()
            && GetWindowTextLengthW(hwnd) > 0
    }
}

fn main_window_of(pid: u32) -> Option<HWND> {
    use windows::Win32::Foundation::TRUE;
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId};
    use windows::core::BOOL;

    struct Search {
        pid: u32,
        found: Option<HWND>,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: `lparam` pointe sur la `Search` vivante de l'appelant.
        let search = unsafe { &mut *(lparam.0 as *mut Search) };
        let mut owner = 0;
        // SAFETY: `owner` est une sortie locale.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut owner)) };
        if owner == search.pid && is_main_window(hwnd) {
            search.found = Some(hwnd);
            return BOOL(0);
        }
        TRUE
    }

    let mut search = Search { pid, found: None };
    // SAFETY: `search` vit jusqu'à la fin de l'énumération synchrone.
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&mut search as *mut Search as isize));
    }
    search.found
}

fn activate(hwnd: HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{IsIconic, SW_RESTORE, SetForegroundWindow};
    // SAFETY: fenêtre vérifiée par l'appelant. Autorisé sans astuce : le clic
    // sur l'île nous a donné la dernière entrée utilisateur.
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd).as_bool()
    }
}

/// En release l'exécutable n'a pas de console : pour `doctor`, on réutilise
/// celle du terminal qui l'a lancé.
pub fn attach_parent_console() {
    use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    // SAFETY: appel sans pointeur ; échoue sans effet si une console existe déjà.
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

/// Son système « notification », joué en arrière-plan.
pub fn alert_sound() {
    use windows::Win32::System::Diagnostics::Debug::MessageBeep;
    use windows::Win32::UI::WindowsAndMessaging::MB_ICONASTERISK;
    // SAFETY: appel sans pointeur, asynchrone.
    unsafe {
        let _ = MessageBeep(MB_ICONASTERISK);
    }
}

/// Boîte de dialogue « Ouvrir » filtrée sur les calendriers .ics.
pub fn pick_ics_file() -> Option<std::path::PathBuf> {
    use windows::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    use windows::core::PWSTR;

    let filter: Vec<u16> = bw_i18n::tr!(
        "Calendars (*.ics)\0*.ics\0All files\0*.*\0\0",
        "Calendriers (*.ics)\0*.ics\0Tous les fichiers\0*.*\0\0"
    )
    .encode_utf16()
    .collect();
    let mut buffer = vec![0u16; 1024];
    let mut ofn = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    // SAFETY: `filter` et `buffer` vivent pendant l'appel modal.
    let ok = unsafe { GetOpenFileNameW(&mut ofn) }.as_bool();
    if !ok {
        return None;
    }
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(std::path::PathBuf::from(String::from_utf16_lossy(
        &buffer[..len],
    )))
}

/// Rouvre une conversation Claude Code : `claude --resume <id>` dans un
/// terminal ouvert sur le dossier de la session. L'identifiant est validé
/// (hexadécimal et tirets) avant d'arriver sur une ligne de commande.
pub fn resume_claude_session(cwd: &Path, session_id: &str) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

    if !bw_claude::is_session_id(session_id) || !cwd.is_dir() {
        log::warn!("reprise de session refusée (id ou dossier invalide)");
        return false;
    }
    // Windows Terminal s'il est là ; `cmd /k` résout `claude.cmd` comme `claude.exe`.
    let in_terminal = std::process::Command::new("wt.exe")
        .arg("-d")
        .arg(cwd)
        .args(["cmd.exe", "/k", "claude", "--resume", session_id])
        .spawn();
    if in_terminal.is_ok() {
        return true;
    }
    std::process::Command::new("cmd.exe")
        .args(["/k", "claude", "--resume", session_id])
        .current_dir(cwd)
        .creation_flags(CREATE_NEW_CONSOLE)
        .spawn()
        .map_err(|e| log::warn!("impossible de rouvrir la session : {e}"))
        .is_ok()
}

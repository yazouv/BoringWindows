//! Intégration Win32 : style de la fenêtre, zone cliquable, placement,
//! détection du plein écran, démarrage automatique, instance unique.

use std::cell::RefCell;
use std::path::Path;

use bw_config::MonitorChoice;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::winit::platform::windows::WindowAttributesExtWindows;
use slint::winit_030::winit::window::{Window, WindowAttributes};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, GetLastError, HANDLE, HWND, LPARAM,
    LRESULT, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    CreateRectRgn, GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY,
    MONITORINFO, MonitorFromPoint, MonitorFromWindow, SetWindowRgn,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Shell::{
    ABM_NEW, ABM_REMOVE, ABN_FULLSCREENAPP, APPBARDATA, QUNS_BUSY, QUNS_PRESENTATION_MODE,
    QUNS_RUNNING_D3D_FULL_SCREEN, SHAppBarMessage, SHQueryUserNotificationState, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GWL_EXSTYLE, GetCursorPos, GetForegroundWindow,
    GetWindowLongPtrW, RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE, SW_SHOWNORMAL, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, WM_APP, WM_DISPLAYCHANGE, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{HSTRING, PCWSTR, w};

use super::PlatformEvent;
use crate::geometry::PhysRect;

mod tray;
pub use tray::Tray;

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

/// Attributs appliqués dès la création : la fenêtre n'apparaît jamais dans la
/// barre des tâches et ne prend pas le focus en s'affichant.
pub fn window_attributes(attrs: WindowAttributes) -> WindowAttributes {
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
}

pub struct Platform {
    hwnd: HWND,
    /// Fenêtre cachée qui reçoit les notifications du shell (appbar).
    helper: HWND,
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
        let helper = create_helper_window()?;
        Ok(Self { hwnd, helper })
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
        let mut data = appbar_data(self.helper);
        // SAFETY: désenregistre l'appbar créée dans `create_helper_window`.
        unsafe {
            SHAppBarMessage(ABM_REMOVE, &mut data);
            let _ = DestroyWindow(self.helper);
        }
    }
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
        WM_DISPLAYCHANGE => Some(PlatformEvent::DisplayChanged),
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

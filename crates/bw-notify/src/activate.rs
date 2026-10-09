//! Rejoue le clic sur une notification : l'API de lecture ne donne pas le
//! lien qu'elle porte (le bon salon Discord, le bon onglet…), alors on ouvre
//! le centre de notifications (Win+N), on y retrouve la notification par son
//! texte via UI Automation et on l'active comme un clic. Windows 11 seulement.

use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationInvokePattern, TreeScope_Descendants,
    UIA_InvokePatternId, UIA_ListItemControlTypeId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_ESCAPE, VK_LWIN, VK_N,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId,
};
use windows::core::{PWSTR, w};

use crate::inbox::matches_center_entry;

/// Temps laissé au panneau pour s'ouvrir et lister les notifications.
const OPEN_TIMEOUT: Duration = Duration::from_millis(1500);
const CENTER_CLASS: &str = "Windows.UI.Core.CoreWindow";
const CENTER_PROCESS: &str = "shellexperiencehost.exe";
/// Premier numéro de build de Windows 11 (Win+N y ouvre les notifications).
const WINDOWS_11_BUILD: u32 = 22_000;

/// Active la notification aux textes `lines` depuis le centre de
/// notifications. `false` si elle n'y est plus (ou hors Windows 11) : le
/// panneau est alors refermé. Bloque jusqu'à 1,5 s : hors du thread UI.
pub fn open_from_center(lines: &[String]) -> bool {
    if lines.is_empty() || windows_build().is_none_or(|b| b < WINDOWS_11_BUILD) {
        return false;
    }
    // SAFETY: initialisation COM du thread appelant (sans effet si déjà faite).
    let uia: IUIAutomation = unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        match CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) {
            Ok(uia) => uia,
            Err(e) => {
                log::warn!("notifications : UI Automation indisponible : {e}");
                return false;
            }
        }
    };

    if center_window().is_none() {
        press(&[VK_LWIN, VK_N]);
    }
    let deadline = Instant::now() + OPEN_TIMEOUT;
    while Instant::now() < deadline {
        if let Some(hwnd) = center_window() {
            match invoke_entry(&uia, hwnd, lines) {
                Ok(true) => return true,
                Ok(false) => {}
                Err(e) => log::debug!("notifications : centre de notifications : {e}"),
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    log::info!("notifications : introuvable dans le centre de notifications");
    if center_window().is_some() {
        press(&[VK_ESCAPE]);
    }
    false
}

fn invoke_entry(uia: &IUIAutomation, hwnd: HWND, lines: &[String]) -> windows::core::Result<bool> {
    // SAFETY: appels COM sur des objets valides ; `hwnd` peut disparaître
    // entre-temps, ce qui donne une erreur et non un accès invalide.
    unsafe {
        let root = uia.ElementFromHandle(hwnd)?;
        // Une centaine d'éléments : on filtre nous-mêmes les lignes de liste.
        let items = root.FindAll(TreeScope_Descendants, &uia.CreateTrueCondition()?)?;
        for i in 0..items.Length()? {
            let item = items.GetElement(i)?;
            if item.CurrentControlType()? == UIA_ListItemControlTypeId
                && matches_center_entry(&item.CurrentName()?.to_string(), lines)
            {
                let invoke: IUIAutomationInvokePattern =
                    item.GetCurrentPatternAs(UIA_InvokePatternId)?;
                invoke.Invoke()?;
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Le centre de notifications, s'il est au premier plan.
fn center_window() -> Option<HWND> {
    // SAFETY: lectures sur la fenêtre au premier plan, tampon local.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }
        let mut class = [0u16; 64];
        let len = GetClassNameW(hwnd, &mut class) as usize;
        let is_center = String::from_utf16_lossy(&class[..len]) == CENTER_CLASS
            && process_name(hwnd).as_deref() == Some(CENTER_PROCESS);
        is_center.then_some(hwnd)
    }
}

/// Nom de l'exécutable (en minuscules) qui possède `hwnd`.
fn process_name(hwnd: HWND) -> Option<String> {
    let mut pid = 0;
    // SAFETY: sorties locales ; le handle du processus est refermé.
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut len = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(process);
        result.ok()?;
        let path = String::from_utf16_lossy(&buffer[..len as usize]);
        Some(path.rsplit('\\').next()?.to_ascii_lowercase())
    }
}

/// Appuie sur `keys` ensemble, puis les relâche dans l'ordre inverse.
fn press(keys: &[VIRTUAL_KEY]) {
    let key = |vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    let inputs: Vec<INPUT> = keys
        .iter()
        .map(|&vk| key(vk, KEYBD_EVENT_FLAGS(0)))
        .chain(keys.iter().rev().map(|&vk| key(vk, KEYEVENTF_KEYUP)))
        .collect();
    // SAFETY: tableau local d'entrées clavier valides.
    unsafe {
        SendInput(&inputs, size_of::<INPUT>() as i32);
    }
}

/// Numéro de build de Windows (`CurrentBuildNumber`).
fn windows_build() -> Option<u32> {
    let mut buffer = [0u16; 32];
    let mut size = (buffer.len() * 2) as u32;
    // SAFETY: tampon local et taille en octets cohérente.
    unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"),
            w!("CurrentBuildNumber"),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
        .ok()
        .ok()?;
    }
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..len]).trim().parse().ok()
}

//! Lecture du registre de confidentialité (`ConsentStore`) et des processus,
//! sur un thread qui dort jusqu'à un changement : `RegNotifyChangeKeyValue`
//! sur chaque capacité, plus la fin du processus « en direct ».

use std::sync::Arc;

use tokio::sync::mpsc::UnboundedSender;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_SUCCESS, FILETIME, HANDLE, WAIT_FAILED, WAIT_OBJECT_0,
};
use windows::Win32::System::ProcessStatus::EnumProcesses;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, REG_NOTIFY_CHANGE_LAST_SET, REG_NOTIFY_CHANGE_NAME,
    RRF_RT_REG_QWORD, RegCloseKey, RegEnumKeyExW, RegGetValueW, RegNotifyChangeKeyValue,
    RegOpenKeyExW,
};
use windows::Win32::System::Threading::{
    CreateEventW, GetProcessTimes, INFINITE, OpenProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, QueryFullProcessImageNameW, SetEvent,
    WaitForMultipleObjects,
};
use windows::core::{HSTRING, PWSTR, w};

use crate::{Capability, Process, Usage};

const CONSENT_STORE: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore";
const CAPABILITIES: [(&str, Capability); 3] = [
    ("microphone", Capability::Microphone),
    ("graphicsCaptureProgrammatic", Capability::ScreenCapture),
    ("graphicsCaptureWithoutBorder", Capability::ScreenCapture),
];

/// Événement Windows partagé entre le module et son thread, fermé avec le
/// dernier des deux.
struct Event(HANDLE);

// SAFETY: un HANDLE d'événement s'utilise depuis n'importe quel thread.
unsafe impl Send for Event {}
// SAFETY: idem, SetEvent et l'attente sont sûrs en parallèle.
unsafe impl Sync for Event {}

impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: handle créé par CreateEventW, fermé une seule fois.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Arrête le thread d'écoute quand le module s'arrête.
pub struct Stop(Arc<Event>);

impl Drop for Stop {
    fn drop(&mut self) {
        // SAFETY: événement vivant (gardé par l'Arc).
        unsafe {
            let _ = SetEvent(self.0.0);
        }
    }
}

pub fn spawn(call_apps: Vec<String>, tx: UnboundedSender<Option<String>>) -> anyhow::Result<Stop> {
    // SAFETY: événement à réinitialisation manuelle, sans nom.
    let stop = Arc::new(Event(unsafe { CreateEventW(None, true, false, None)? }));
    let thread_stop = stop.clone();
    std::thread::Builder::new()
        .name("bw-presence".into())
        .spawn(move || {
            if let Err(e) = run(&call_apps, &tx, &thread_stop) {
                log::warn!("présentation : détection indisponible : {e:#}");
            }
        })?;
    Ok(Stop(stop))
}

/// Clé de capacité surveillée et son événement de changement.
struct Watched {
    key: HKEY,
    event: Event,
    capability: Capability,
}

impl Drop for Watched {
    fn drop(&mut self) {
        // SAFETY: clé ouverte par RegOpenKeyExW, fermée une seule fois.
        unsafe {
            let _ = RegCloseKey(self.key);
        }
    }
}

impl Watched {
    /// (Ré)arme la notification : elle ne sert qu'une fois.
    fn arm(&self) -> anyhow::Result<()> {
        // SAFETY: clé et événement vivants.
        let status = unsafe {
            RegNotifyChangeKeyValue(
                self.key,
                true,
                REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET,
                Some(self.event.0),
                true,
            )
        };
        anyhow::ensure!(
            status == ERROR_SUCCESS,
            "RegNotifyChangeKeyValue : {status:?}"
        );
        Ok(())
    }
}

fn run(
    call_apps: &[String],
    tx: &UnboundedSender<Option<String>>,
    stop: &Event,
) -> anyhow::Result<()> {
    let mut watched = Vec::new();
    for (name, capability) in CAPABILITIES {
        let mut key = HKEY::default();
        let path = HSTRING::from(format!("{CONSENT_STORE}\\{name}"));
        // SAFETY: chemin et pointeur de sortie valides.
        let status = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, &path, None, KEY_READ, &mut key) };
        if status != ERROR_SUCCESS {
            log::debug!("présentation : {name} absent ({status:?})");
            continue;
        }
        // SAFETY: événement à réinitialisation automatique, sans nom.
        let event = Event(unsafe { CreateEventW(None, false, false, None)? });
        let w = Watched {
            key,
            event,
            capability,
        };
        w.arm()?;
        watched.push(w);
    }
    anyhow::ensure!(!watched.is_empty(), "registre ConsentStore introuvable");

    let mut last: Option<String> = None;
    let mut first = true;
    loop {
        let usages: Vec<Usage> = watched
            .iter()
            .flat_map(|w| read_usages(w.key, w.capability))
            .collect();
        // Les processus ne sont listés que si une entrée est « en cours ».
        let processes = if usages.iter().any(|u| u.stop == 0 && !u.packaged) {
            processes()
        } else {
            Vec::new()
        };
        let live = crate::live_app(&usages, &processes, call_apps);
        let app = live.as_ref().map(|l| l.app.clone());
        if first || app != last {
            first = false;
            last.clone_from(&app);
            if tx.send(app).is_err() {
                return Ok(());
            }
        }

        // Fin du processus en direct : une appli qui plante ne remet pas son
        // entrée à jour.
        let process = live
            .and_then(|l| l.pid)
            // SAFETY: simple ouverture, fermée plus bas.
            .and_then(|pid| unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }.ok());
        let mut handles: Vec<HANDLE> = vec![stop.0];
        handles.extend(watched.iter().map(|w| w.event.0));
        handles.extend(process);
        // SAFETY: handles vivants pendant l'attente.
        let woke = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };
        if let Some(p) = process {
            // SAFETY: ouvert juste au-dessus.
            unsafe {
                let _ = CloseHandle(p);
            }
        }
        if woke == WAIT_FAILED {
            anyhow::bail!("attente impossible");
        }
        let index = woke.0.wrapping_sub(WAIT_OBJECT_0.0) as usize;
        match index {
            0 => return Ok(()),
            i if i <= watched.len() => watched[i - 1].arm()?,
            _ => {}
        }
    }
}

/// Entrées d'une capacité : les applications empaquetées directement sous la
/// clé, les autres sous `NonPackaged`.
fn read_usages(key: HKEY, capability: Capability) -> Vec<Usage> {
    let mut out = Vec::new();
    for name in subkeys(key) {
        if name == "NonPackaged" {
            let path = HSTRING::from(name.as_str());
            let mut sub = HKEY::default();
            // SAFETY: clé parente vivante, pointeur de sortie valide.
            if unsafe { RegOpenKeyExW(key, &path, None, KEY_READ, &mut sub) } != ERROR_SUCCESS {
                continue;
            }
            for app in subkeys(sub) {
                if let Some((start, stop)) = times(sub, &app) {
                    out.push(Usage {
                        capability,
                        app,
                        packaged: false,
                        start,
                        stop,
                    });
                }
            }
            // SAFETY: ouverte juste au-dessus.
            unsafe {
                let _ = RegCloseKey(sub);
            }
        } else if let Some((start, stop)) = times(key, &name) {
            out.push(Usage {
                capability,
                app: name,
                packaged: true,
                start,
                stop,
            });
        }
    }
    out
}

fn subkeys(key: HKEY) -> Vec<String> {
    let mut names = Vec::new();
    let mut buf = [0u16; 512];
    for index in 0.. {
        let mut len = buf.len() as u32;
        // SAFETY: tampon et longueur cohérents.
        let status = unsafe {
            RegEnumKeyExW(
                key,
                index,
                Some(PWSTR(buf.as_mut_ptr())),
                &mut len,
                None,
                None,
                None,
                None,
            )
        };
        if status != ERROR_SUCCESS {
            break;
        }
        names.push(String::from_utf16_lossy(&buf[..len as usize]));
    }
    names
}

/// `LastUsedTimeStart` et `LastUsedTimeStop` d'une sous-clé.
fn times(key: HKEY, sub: &str) -> Option<(u64, u64)> {
    let sub = HSTRING::from(sub);
    let read = |value: windows::core::PCWSTR| -> Option<u64> {
        let mut data = 0u64;
        let mut size = size_of::<u64>() as u32;
        // SAFETY: tampon d'un QWORD, taille exacte.
        let status = unsafe {
            RegGetValueW(
                key,
                &sub,
                value,
                RRF_RT_REG_QWORD,
                None,
                Some((&raw mut data).cast()),
                Some(&mut size),
            )
        };
        (status == ERROR_SUCCESS).then_some(data)
    };
    Some((
        read(w!("LastUsedTimeStart"))?,
        read(w!("LastUsedTimeStop"))?,
    ))
}

/// Processus visibles : chemin complet et instant de création.
fn processes() -> Vec<Process> {
    let mut pids = vec![0u32; 4096];
    let mut needed = 0u32;
    // SAFETY: tampon et taille cohérents.
    if unsafe {
        EnumProcesses(
            pids.as_mut_ptr(),
            (pids.len() * size_of::<u32>()) as u32,
            &mut needed,
        )
    }
    .is_err()
    {
        return Vec::new();
    }
    pids.truncate(needed as usize / size_of::<u32>());
    pids.into_iter()
        .filter(|pid| *pid != 0)
        .filter_map(|pid| {
            // SAFETY: handle ouvert puis fermé dans ce bloc.
            unsafe {
                let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
                let mut buf = [0u16; 1024];
                let mut len = buf.len() as u32;
                let named = QueryFullProcessImageNameW(
                    h,
                    PROCESS_NAME_WIN32,
                    PWSTR(buf.as_mut_ptr()),
                    &mut len,
                );
                let (mut created, mut exit, mut kernel, mut user) = (
                    FILETIME::default(),
                    FILETIME::default(),
                    FILETIME::default(),
                    FILETIME::default(),
                );
                let timed = GetProcessTimes(h, &mut created, &mut exit, &mut kernel, &mut user);
                let _ = CloseHandle(h);
                named.ok()?;
                timed.ok()?;
                Some(Process {
                    pid,
                    path: String::from_utf16_lossy(&buf[..len as usize]),
                    created: (u64::from(created.dwHighDateTime) << 32)
                        | u64::from(created.dwLowDateTime),
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lit le vrai registre et les processus : `cargo test -p bw-presence --
    /// --ignored --nocapture`.
    #[test]
    #[ignore]
    fn reads_real_registry() {
        let config = crate::PresentationConfig::default();
        let mut usages = Vec::new();
        for (name, capability) in CAPABILITIES {
            let mut key = HKEY::default();
            let path = HSTRING::from(format!("{CONSENT_STORE}\\{name}"));
            // SAFETY: chemin et pointeur valides.
            if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, &path, None, KEY_READ, &mut key) }
                == ERROR_SUCCESS
            {
                usages.extend(read_usages(key, capability));
                // SAFETY: ouverte juste au-dessus.
                unsafe {
                    let _ = RegCloseKey(key);
                }
            }
        }
        let processes = processes();
        let in_progress: Vec<_> = usages.iter().filter(|u| u.stop == 0).collect();
        println!("{} entrées, en cours : {in_progress:#?}", usages.len());
        println!("{} processus", processes.len());
        println!(
            "en direct : {:?}",
            crate::live_app(&usages, &processes, &config.call_apps)
        );
        assert!(!usages.is_empty());
        assert!(!processes.is_empty());
    }
}

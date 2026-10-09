//! Écoute de la luminosité de l'écran intégré (`WmiMonitorBrightnessEvent`,
//! espace WMI `root\WMI`). N'existe que sur les portables et tablettes : sur
//! un écran externe, Windows ne publie rien et le thread s'arrête tout seul.

use std::sync::mpsc::{Receiver, TryRecvError};

use tokio::sync::mpsc::UnboundedSender;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    CoSetProxyBlanket, CoUninitialize, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL,
    RPC_C_IMP_LEVEL_IMPERSONATE,
};
use windows::Win32::System::Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE};
use windows::Win32::System::Variant::{VARIANT, VariantClear, VariantToUInt32};
use windows::Win32::System::Wmi::{
    IEnumWbemClassObject, IWbemClassObject, IWbemLocator, IWbemServices, WBEM_FLAG_FORWARD_ONLY,
    WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_INFINITE, WbemLocator,
};
use windows::core::{BSTR, IUnknown, Interface, w};

/// Attente maximale d'un événement avant de vérifier si le module s'est
/// arrêté : un réveil toutes les 2 s, sur un thread bloqué dans WMI.
const WAIT_MS: i32 = 2000;

/// Thread dédié : COM, connexion à WMI, puis attente des événements.
pub fn spawn(tx: UnboundedSender<u8>, stop: Receiver<()>) {
    std::thread::spawn(move || {
        // SAFETY: COM initialisé et libéré sur ce thread.
        unsafe {
            if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
                log::warn!("luminosité : COM indisponible");
                return;
            }
        }
        if let Err(e) = listen(&tx, &stop) {
            log::warn!("luminosité : {e:#}");
        }
        // SAFETY: apparié au CoInitializeEx de ce thread.
        unsafe { CoUninitialize() };
    });
}

fn listen(tx: &UnboundedSender<u8>, stop: &Receiver<()>) -> anyhow::Result<()> {
    // SAFETY: appels COM sur un thread initialisé ; les objets renvoyés
    // restent valides tant qu'on les détient.
    unsafe {
        let services = connect()?;
        let wql = BSTR::from("WQL");
        let flags = WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY;

        // Pas d'écran à luminosité réglable (PC fixe, écran externe) : rien à écouter.
        let screens = services.ExecQuery(
            &wql,
            &BSTR::from("SELECT CurrentBrightness FROM WmiMonitorBrightness"),
            flags,
            None,
        );
        let has_screen = match screens {
            Ok(e) => secure(&e).is_ok() && next(&e, WBEM_INFINITE).is_some(),
            Err(_) => false,
        };
        if !has_screen {
            log::info!("luminosité : aucun écran intégré, module inactif");
            return Ok(());
        }

        let events = services.ExecNotificationQuery(
            &wql,
            &BSTR::from("SELECT Brightness FROM WmiMonitorBrightnessEvent"),
            flags,
            None,
        )?;
        secure(&events)?;
        log::info!("luminosité : écoute de l'écran intégré");
        loop {
            match stop.try_recv() {
                Err(TryRecvError::Empty) => {}
                _ => return Ok(()),
            }
            let Some(event) = next(&events, WAIT_MS) else {
                continue;
            };
            if let Some(level) = brightness(&event)
                && tx.send(level).is_err()
            {
                return Ok(());
            }
        }
    }
}

/// Connexion à `root\WMI` avec l'emprunt d'identité qu'attend WMI.
unsafe fn connect() -> anyhow::Result<IWbemServices> {
    // SAFETY: voir `listen`.
    unsafe {
        let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)?;
        let empty = BSTR::new();
        let services = locator.ConnectServer(
            &BSTR::from("root\\WMI"),
            &empty,
            &empty,
            &empty,
            0,
            &empty,
            None,
        )?;
        secure(&services)?;
        Ok(services)
    }
}

/// Niveau d'emprunt d'identité « impersonate » sur un proxy WMI : sans lui,
/// le fournisseur refuse les requêtes (niveau par défaut du processus trop bas).
unsafe fn secure(proxy: &impl Interface) -> windows::core::Result<()> {
    // SAFETY: proxy COM valide, paramètres documentés pour WMI local.
    unsafe {
        CoSetProxyBlanket(
            &proxy.cast::<IUnknown>()?,
            RPC_C_AUTHN_WINNT,
            RPC_C_AUTHZ_NONE,
            None,
            RPC_C_AUTHN_LEVEL_CALL,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
        )
    }
}

/// Objet suivant d'une énumération, `None` si le délai expire ou en cas d'erreur.
unsafe fn next(e: &IEnumWbemClassObject, timeout_ms: i32) -> Option<IWbemClassObject> {
    let mut objects = [None];
    let mut returned = 0;
    // SAFETY: tableau d'une case et compteur valides pendant l'appel.
    let hr = unsafe { e.Next(timeout_ms, &mut objects, &mut returned) };
    if hr.is_err() || returned == 0 {
        return None;
    }
    objects[0].take()
}

/// Propriété `Brightness` (0 à 100) d'un `WmiMonitorBrightnessEvent`.
unsafe fn brightness(event: &IWbemClassObject) -> Option<u8> {
    let mut value = VARIANT::default();
    // SAFETY: VARIANT initialisé, libéré après lecture.
    unsafe {
        event
            .Get(w!("Brightness"), 0, &mut value, None, None)
            .ok()?;
        let level = VariantToUInt32(&value).ok();
        let _ = VariantClear(&mut value);
        level.and_then(|l| u8::try_from(l).ok())
    }
}

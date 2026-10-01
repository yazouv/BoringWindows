//! Écoute du volume principal du périphérique de sortie par défaut
//! (`IAudioEndpointVolume::RegisterControlChangeNotify`).

use std::sync::mpsc::Receiver;

use tokio::sync::mpsc::UnboundedSender;
use windows::Win32::Media::Audio::Endpoints::{
    IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
};
use windows::Win32::Media::Audio::{
    AUDIO_VOLUME_NOTIFICATION_DATA, IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::core::implement;

#[implement(IAudioEndpointVolumeCallback)]
struct Callback {
    tx: UnboundedSender<(f32, bool)>,
}

impl IAudioEndpointVolumeCallback_Impl for Callback_Impl {
    fn OnNotify(&self, pnotify: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        // SAFETY: Windows fournit un pointeur valide pendant l'appel.
        if let Some(data) = unsafe { pnotify.as_ref() } {
            let _ = self.tx.send((data.fMasterVolume, data.bMuted.as_bool()));
        }
        Ok(())
    }
}

/// Thread dédié : COM, inscription du callback, puis attente de l'arrêt.
pub fn spawn(tx: UnboundedSender<(f32, bool)>, stop: Receiver<()>) {
    std::thread::spawn(move || {
        // SAFETY: COM initialisé et libéré sur ce thread.
        unsafe {
            if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
                log::warn!("volume : COM indisponible");
                return;
            }
        }
        match register(tx) {
            Ok((endpoint, callback)) => {
                // Bloque jusqu'à la fermeture du canal (arrêt du module).
                let _ = stop.recv();
                // SAFETY: désinscription du callback enregistré ci-dessus.
                unsafe {
                    let _ = endpoint.UnregisterControlChangeNotify(&callback);
                }
            }
            Err(e) => log::warn!("volume : {e:#}"),
        }
        // SAFETY: apparié au CoInitializeEx de ce thread.
        unsafe { CoUninitialize() };
    });
}

fn register(
    tx: UnboundedSender<(f32, bool)>,
) -> anyhow::Result<(IAudioEndpointVolume, IAudioEndpointVolumeCallback)> {
    // SAFETY: appels COM sur un thread initialisé.
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
        let callback: IAudioEndpointVolumeCallback = Callback { tx }.into();
        endpoint.RegisterControlChangeNotify(&callback)?;
        log::info!("volume : écoute du périphérique de sortie par défaut");
        Ok((endpoint, callback))
    }
}

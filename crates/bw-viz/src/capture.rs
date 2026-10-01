//! Capture « loopback » WASAPI du périphérique de sortie par défaut, sur un
//! thread dédié qui n'existe que pendant l'affichage du visualiseur.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bw_core::ModuleCtx;
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
    IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize,
};

use crate::config::VizConfig;
use crate::dsp::Analyzer;
use crate::module::VizSnapshot;

pub fn spawn(ctx: ModuleCtx, config: VizConfig, running: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        // SAFETY: COM initialisé et libéré sur ce thread.
        unsafe {
            if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
                log::warn!("visualiseur : COM indisponible");
                return;
            }
        }
        if let Err(e) = run(&ctx, &config, &running) {
            log::warn!("visualiseur : {e:#}");
        }
        // Barres à zéro quand la capture s'arrête.
        ctx.set_state(VizSnapshot {
            bands: vec![0.0; config.bands],
        });
        // SAFETY: apparié au CoInitializeEx de ce thread.
        unsafe { CoUninitialize() };
    });
}

fn run(ctx: &ModuleCtx, config: &VizConfig, running: &AtomicBool) -> anyhow::Result<()> {
    // SAFETY: appels COM sur un thread initialisé ; les pointeurs rendus par
    // WASAPI ne sont lus que pendant leur validité (entre GetBuffer et
    // ReleaseBuffer, et jusqu'à CoTaskMemFree pour le format).
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let format = client.GetMixFormat()?;
        let (channels, rate, bits) = {
            let f = &*format;
            (usize::from(f.nChannels), f.nSamplesPerSec, f.wBitsPerSample)
        };
        if bits != 32 || channels == 0 {
            CoTaskMemFree(Some(format.cast()));
            anyhow::bail!("format audio inattendu ({bits} bits, {channels} canaux)");
        }
        let init = client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            10_000_000,
            0,
            format,
            None,
        );
        CoTaskMemFree(Some(format.cast()));
        init?;
        let capture: IAudioCaptureClient = client.GetService()?;
        client.Start()?;
        log::info!("visualiseur : capture à {rate} Hz, {channels} canaux");

        let mut analyzer = Analyzer::new(rate, config.bands);
        let mut mono: Vec<f32> = Vec::new();
        let mut last: Option<Vec<f32>> = None;
        while running.load(Ordering::Relaxed) {
            std::thread::sleep(config.frame());
            mono.clear();
            loop {
                let pending = capture.GetNextPacketSize()?;
                if pending == 0 {
                    break;
                }
                let mut data = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
                let frames_n = frames as usize;
                if flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0 || data.is_null() {
                    mono.extend(std::iter::repeat_n(0.0, frames_n));
                } else {
                    let samples =
                        std::slice::from_raw_parts(data.cast::<f32>(), frames_n * channels);
                    mono.extend(
                        samples
                            .chunks_exact(channels)
                            .map(|frame| frame.iter().sum::<f32>() / channels as f32),
                    );
                }
                capture.ReleaseBuffer(frames)?;
            }
            let bands = if mono.is_empty() {
                analyzer.decay().to_vec()
            } else {
                analyzer.process(&mono).to_vec()
            };
            // Rien de neuf (barres déjà à plat) : on évite un rendu inutile.
            if last.as_ref() != Some(&bands) {
                ctx.set_state(VizSnapshot {
                    bands: bands.clone(),
                });
                last = Some(bands);
            }
        }
        let _ = client.Stop();
    }
    Ok(())
}

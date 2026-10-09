//! Icône d'une application à partir de son AppUserModelID, via le dossier
//! virtuel des applications (`shell:AppsFolder`) : marche pour les applis du
//! Store comme pour Discord, Firefox… dès qu'elles ont un raccourci Démarrer.

use std::sync::Arc;

use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits,
    GetObjectW, HBITMAP, ReleaseDC,
};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY,
};
use windows::core::HSTRING;

use crate::inbox::{AppIcon, straight_rgba};

/// Côté de l'icône demandée (px) : assez pour l'avatar à 200 %.
const SIZE_PX: i32 = 64;

/// À appeler sur un thread où COM est initialisé.
pub(crate) fn app_icon(app_id: &str) -> Option<AppIcon> {
    if app_id.is_empty() || app_id.chars().any(char::is_control) {
        return None;
    }
    let path = HSTRING::from(format!("shell:AppsFolder\\{app_id}"));
    // SAFETY: chaîne valide pendant l'appel ; le bitmap rendu nous appartient
    // et est libéré après lecture.
    unsafe {
        let factory: IShellItemImageFactory = SHCreateItemFromParsingName(&path, None).ok()?;
        let bitmap = factory
            .GetImage(
                SIZE {
                    cx: SIZE_PX,
                    cy: SIZE_PX,
                },
                SIIGBF_ICONONLY,
            )
            .ok()?;
        let icon = read(bitmap);
        let _ = DeleteObject(bitmap.into());
        icon
    }
}

/// Pixels du bitmap, convertis en RGBA.
unsafe fn read(bitmap: HBITMAP) -> Option<AppIcon> {
    let mut header = BITMAP::default();
    // SAFETY: `header` est une sortie locale de la bonne taille.
    let got = unsafe {
        GetObjectW(
            bitmap.into(),
            size_of::<BITMAP>() as i32,
            Some((&raw mut header).cast()),
        )
    };
    let (width, height) = (header.bmWidth, header.bmHeight.abs());
    if got == 0 || width <= 0 || height <= 0 || width > 256 || height > 256 {
        return None;
    }
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            // Négatif : lignes de haut en bas.
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bgra = vec![0u8; (width * height * 4) as usize];
    // SAFETY: le tampon fait exactement width × height pixels de 32 bits.
    let lines = unsafe {
        let dc = GetDC(None);
        let lines = GetDIBits(
            dc,
            bitmap,
            0,
            height as u32,
            Some(bgra.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, dc);
        lines
    };
    if lines != height {
        return None;
    }
    let rgba = straight_rgba(&bgra);
    Some(AppIcon {
        width: width as u32,
        height: height as u32,
        accent: bw_media::artwork::accent_color(&rgba),
        rgba: Arc::new(rgba),
    })
}

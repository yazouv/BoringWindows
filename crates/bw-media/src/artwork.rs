//! Pochette : décodage, recadrage carré, miniature et couleur d'accent.

use std::sync::Arc;

use image::imageops::FilterType;

/// Taille de la miniature gardée en mémoire (pixels).
pub const THUMB_SIZE: u32 = 96;
/// Au-delà, on ne tente même pas de décoder.
pub const MAX_ENCODED: usize = 8 * 1024 * 1024;

/// Miniature RGBA carrée prête à afficher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artwork {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<Vec<u8>>,
    /// Couleur dominante, ajustée pour rester lisible sur fond sombre.
    pub accent: Option<[u8; 3]>,
}

impl Artwork {
    /// Décode une image (PNG, JPEG, BMP), la recadre au centre en carré et la
    /// réduit à [`THUMB_SIZE`].
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() || bytes.len() > MAX_ENCODED {
            return None;
        }
        let img = image::load_from_memory(bytes).ok()?;
        let side = img.width().min(img.height());
        if side == 0 {
            return None;
        }
        let x = (img.width() - side) / 2;
        let y = (img.height() - side) / 2;
        let thumb = img
            .crop_imm(x, y, side, side)
            .resize_exact(THUMB_SIZE, THUMB_SIZE, FilterType::Triangle)
            .into_rgba8();
        Some(Self::from_rgba(THUMB_SIZE, THUMB_SIZE, thumb.into_raw()))
    }

    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        let accent = accent_color(&rgba);
        Self {
            width,
            height,
            rgba: Arc::new(rgba),
            accent,
        }
    }
}

/// Couleur dominante parmi les pixels colorés, éclaircie si besoin pour se
/// détacher sur le fond noir de l'île. `None` pour une image grise.
pub fn accent_color(rgba: &[u8]) -> Option<[u8; 3]> {
    const BUCKETS: usize = 24;
    let mut weight = [0f32; BUCKETS];
    let mut sum = [[0f32; 3]; BUCKETS];
    let mut total = 0f32;

    for px in rgba.chunks_exact(4) {
        if px[3] < 128 {
            continue;
        }
        let (h, s, v) = hsv(px[0], px[1], px[2]);
        total += 1.0;
        if s < 0.25 || v < 0.2 {
            continue;
        }
        let w = s * v;
        let b = ((h / 360.0 * BUCKETS as f32) as usize).min(BUCKETS - 1);
        weight[b] += w;
        for c in 0..3 {
            sum[b][c] += f32::from(px[c]) * w;
        }
    }

    let (best, &w) = weight
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))?;
    // Trop peu de couleur : on garde l'accent du thème.
    if total == 0.0 || w < total * 0.03 {
        return None;
    }
    let avg = sum[best].map(|c| c / w);
    let (h, s, v) = hsv(avg[0] as u8, avg[1] as u8, avg[2] as u8);
    Some(from_hsv(h, s.max(0.45), v.clamp(0.75, 1.0)))
}

fn hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    (h, s, max)
}

fn from_hsv(h: f32, s: f32, v: f32) -> [u8; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r, g, b].map(|ch| ((ch + m) * 255.0).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(n: usize, rgb: [u8; 3]) -> Vec<u8> {
        (0..n).flat_map(|_| [rgb[0], rgb[1], rgb[2], 255]).collect()
    }

    #[test]
    fn dominant_color_wins_and_is_brightened() {
        let mut px = solid(900, [20, 40, 140]); // bleu sombre, majoritaire
        px.extend(solid(100, [200, 30, 30])); // un peu de rouge
        let [r, g, b] = accent_color(&px).unwrap();
        assert!(b > r && b > g, "bleu attendu, obtenu {r},{g},{b}");
        assert!(b >= 190, "assez lumineux pour un fond noir : {b}");
    }

    #[test]
    fn grey_or_transparent_images_have_no_accent() {
        assert_eq!(accent_color(&solid(500, [128, 128, 128])), None);
        assert_eq!(accent_color(&[255, 0, 0, 0].repeat(100)), None);
        assert_eq!(accent_color(&[]), None);
    }

    #[test]
    fn hsv_round_trip() {
        for rgb in [
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
            [255, 138, 61],
            [10, 200, 180],
        ] {
            let (h, s, v) = hsv(rgb[0], rgb[1], rgb[2]);
            let back = from_hsv(h, s, v);
            for c in 0..3 {
                assert!(rgb[c].abs_diff(back[c]) <= 1, "{rgb:?} -> {back:?}");
            }
        }
    }

    #[test]
    fn decode_crops_to_square_thumbnail() {
        // PNG 40x20 : moitié gauche rouge, moitié droite verte.
        let mut img = image::RgbaImage::new(40, 20);
        for (x, _, p) in img.enumerate_pixels_mut() {
            *p = if x < 20 {
                image::Rgba([220, 20, 20, 255])
            } else {
                image::Rgba([20, 220, 20, 255])
            };
        }
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();

        let art = Artwork::decode(&png).unwrap();
        assert_eq!((art.width, art.height), (THUMB_SIZE, THUMB_SIZE));
        assert_eq!(art.rgba.len(), (THUMB_SIZE * THUMB_SIZE * 4) as usize);
        assert!(Artwork::decode(b"pas une image").is_none());
    }
}

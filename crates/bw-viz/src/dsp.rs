//! Analyse du son : FFT radix-2, bandes réparties en échelle logarithmique,
//! lissage (montée rapide, descente lente). Pur calcul, testé sans audio.

use std::f32::consts::PI;

/// Taille de la fenêtre d'analyse (puissance de 2).
pub const FFT_SIZE: usize = 1024;

pub struct Analyzer {
    window: Vec<f32>,
    /// Dernières `FFT_SIZE` valeurs mono.
    samples: Vec<f32>,
    /// Bornes (indices de bins FFT) de chaque bande : `edges.len() == bands + 1`.
    edges: Vec<usize>,
    levels: Vec<f32>,
}

impl Analyzer {
    pub fn new(sample_rate: u32, bands: usize) -> Self {
        let sample_rate = sample_rate as f32;
        let window = (0..FFT_SIZE)
            .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / (FFT_SIZE - 1) as f32).cos())
            .collect();
        // De 60 Hz à 16 kHz (ou Nyquist), espacement logarithmique.
        let low = 60.0_f32;
        let high = 16_000.0_f32.min(sample_rate / 2.0 - 1.0);
        let bin = |hz: f32| {
            ((hz * FFT_SIZE as f32 / sample_rate).round() as usize).clamp(1, FFT_SIZE / 2 - 1)
        };
        let mut edges: Vec<usize> = (0..=bands)
            .map(|i| bin(low * (high / low).powf(i as f32 / bands as f32)))
            .collect();
        // Chaque bande contient au moins un bin.
        for i in 1..edges.len() {
            if edges[i] <= edges[i - 1] {
                edges[i] = (edges[i - 1] + 1).min(FFT_SIZE / 2);
            }
        }
        Self {
            window,
            samples: vec![0.0; FFT_SIZE],
            edges,
            levels: vec![0.0; bands],
        }
    }

    /// Ajoute des échantillons mono, puis calcule les niveaux (0 à 1) de chaque bande.
    pub fn process(&mut self, mono: &[f32]) -> &[f32] {
        if mono.len() >= FFT_SIZE {
            self.samples.copy_from_slice(&mono[mono.len() - FFT_SIZE..]);
        } else {
            self.samples.rotate_left(mono.len());
            let start = FFT_SIZE - mono.len();
            self.samples[start..].copy_from_slice(mono);
        }

        let mut re: Vec<f32> = self
            .samples
            .iter()
            .zip(&self.window)
            .map(|(s, w)| s * w)
            .collect();
        let mut im = vec![0.0_f32; FFT_SIZE];
        fft(&mut re, &mut im);

        for (band, level) in self.levels.iter_mut().enumerate() {
            let (a, b) = (self.edges[band], self.edges[band + 1]);
            let power: f32 = (a..b.max(a + 1))
                .map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt())
                .sum::<f32>()
                / (b.max(a + 1) - a) as f32;
            // Amplitude normalisée par la fenêtre, puis échelle en dB (-70 dB → 0, 0 dB → 1).
            let amplitude = power / (FFT_SIZE as f32 / 4.0);
            let db = 20.0 * (amplitude + 1e-9).log10();
            let target = ((db + 70.0) / 70.0).clamp(0.0, 1.0);
            let k = if target > *level { 0.65 } else { 0.18 };
            *level += (target - *level) * k;
        }
        &self.levels
    }

    /// Silence : les barres retombent doucement.
    pub fn decay(&mut self) -> &[f32] {
        for l in &mut self.levels {
            *l *= 0.82;
        }
        &self.levels
    }
}

/// FFT radix-2 en place (`re.len()` est une puissance de 2).
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -2.0 * PI / len as f32;
        let (wr, wi) = (angle.cos(), angle.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0_f32, 0.0_f32);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let next = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = next;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f32, rate: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| 0.8 * (2.0 * PI * hz * i as f32 / rate).sin())
            .collect()
    }

    #[test]
    fn fft_finds_a_pure_tone() {
        let rate = 48_000.0;
        let mut re = sine(3_000.0, rate, FFT_SIZE);
        let mut im = vec![0.0; FFT_SIZE];
        fft(&mut re, &mut im);
        let peak = (1..FFT_SIZE / 2)
            .max_by(|a, b| {
                let ma = re[*a].hypot(im[*a]);
                let mb = re[*b].hypot(im[*b]);
                ma.total_cmp(&mb)
            })
            .unwrap();
        let hz = peak as f32 * rate / FFT_SIZE as f32;
        assert!((hz - 3_000.0).abs() < 60.0, "{hz}");
    }

    #[test]
    fn loudest_band_holds_the_tone_and_silence_is_flat() {
        let mut a = Analyzer::new(48_000, 16);
        let tone = sine(1_000.0, 48_000.0, FFT_SIZE);
        let mut levels = Vec::new();
        for _ in 0..6 {
            levels = a.process(&tone).to_vec();
        }
        let top = levels
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        // 1 kHz tombe dans la partie basse-médium de l'échelle 60 Hz – 16 kHz.
        assert!((4..=9).contains(&top), "bande {top}: {levels:?}");
        assert!(levels[top] > 0.5);
        assert!(levels[15] < 0.2 && levels[0] < 0.2);

        let mut quiet = Analyzer::new(48_000, 16);
        let silent = quiet.process(&vec![0.0; FFT_SIZE]).to_vec();
        assert!(silent.iter().all(|v| *v < 0.01), "{silent:?}");
    }

    #[test]
    fn decay_lowers_levels_toward_zero() {
        let mut a = Analyzer::new(44_100, 8);
        let tone = sine(500.0, 44_100.0, FFT_SIZE);
        for _ in 0..6 {
            a.process(&tone);
        }
        let before: f32 = a.levels.iter().sum();
        for _ in 0..30 {
            a.decay();
        }
        let after: f32 = a.levels.iter().sum();
        assert!(after < before * 0.05, "{before} -> {after}");
    }
}

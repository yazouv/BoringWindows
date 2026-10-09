//! Calcul de la forme de l'île en pixels physiques, partagé entre la zone
//! cliquable Win32 et le placement de la fenêtre.

use bw_config::{Size, Theme};

/// Rectangle en pixels physiques, relatif au coin haut-gauche de la fenêtre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl PhysRect {
    pub fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Self {
            x,
            y,
            width: right - x,
            height: bottom - y,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Compact,
    Attention,
    /// Nouvelle notification : plus large et plus haute que l'attention.
    Notification,
    Expanded,
}

/// Hauteur minimale d'une pilule ramenée à la barre des tâches.
const MIN_PILL_HEIGHT: f32 = 16.0;

/// Barre des tâches en haut de l'écran de l'île, de hauteur `taskbar`
/// (logique) : la pilule fermée et celle d'attention n'en dépassent pas.
/// Une barre plus haute que les pilules (taille normale) ne change rien.
pub fn fit_under_taskbar(theme: &Theme, taskbar: Option<f32>) -> Theme {
    let mut fitted = theme.clone();
    if let Some(bar) = taskbar {
        let cap = (bar - theme.top_offset).max(MIN_PILL_HEIGHT);
        fitted.compact.height = fitted.compact.height.min(cap);
        fitted.attention.height = fitted.attention.height.min(cap);
    }
    fitted
}

/// Taille de la pilule qui annonce une notification : dérivée de l'attention,
/// sans jamais dépasser l'île ouverte (la fenêtre).
pub fn notification_size(theme: &Theme) -> Size {
    Size::new(
        (theme.attention.width + 80.0).min(theme.expanded.width),
        (theme.attention.height + 22.0).min(theme.expanded.height),
    )
}

/// Marge autour de la pilule pour ne pas rogner l'anticrénelage des bords.
const AA_PAD: i32 = 2;

/// Taille logique de la fenêtre (identique au calcul de `island.slint`).
pub fn window_size(theme: &Theme) -> (f32, f32) {
    (
        theme.expanded.width,
        theme.expanded.height + theme.top_offset,
    )
}

/// Zone occupée par la pilule pour `shape`, à l'échelle `scale`.
pub fn pill_rect(theme: &Theme, shape: Shape, scale: f32) -> PhysRect {
    let size = match shape {
        Shape::Compact => theme.compact,
        Shape::Attention => theme.attention,
        Shape::Notification => notification_size(theme),
        Shape::Expanded => theme.expanded,
    };
    let (win_w, win_h) = window_size(theme);

    let left = ((win_w - size.width) / 2.0 * scale).floor() as i32 - AA_PAD;
    let top = (theme.top_offset * scale).floor() as i32 - AA_PAD;
    let right = ((win_w + size.width) / 2.0 * scale).ceil() as i32 + AA_PAD;
    let bottom = ((theme.top_offset + size.height) * scale).ceil() as i32 + AA_PAD;
    let (max_w, max_h) = ((win_w * scale).ceil() as i32, (win_h * scale).ceil() as i32);

    let x = left.max(0);
    let y = top.max(0);
    PhysRect {
        x,
        y,
        width: right.min(max_w) - x,
        height: bottom.min(max_h) - y,
    }
}

/// Forme exacte de la pilule (coins arrondis) : sert de région à la fenêtre
/// quand le flou est actif, puisque le flou de Windows remplit toute la région.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundRect {
    pub rect: PhysRect,
    /// Rayon des coins (pixels physiques).
    pub radius: i32,
    /// Coins du haut carrés : pilule collée en haut de l'écran.
    pub flat_top: bool,
}

/// Pilule telle que l'île la dessine en ce moment (valeurs logiques, animées
/// comprises), en pixels physiques.
pub fn pill_round_rect(
    (x, y, width, height): (f32, f32, f32, f32),
    radius: f32,
    flat_top: bool,
    scale: f32,
) -> RoundRect {
    let left = (x * scale).round() as i32;
    let top = (y * scale).round() as i32;
    let right = ((x + width) * scale).round() as i32;
    let bottom = ((y + height) * scale).round() as i32;
    RoundRect {
        rect: PhysRect {
            x: left,
            y: top,
            width: (right - left).max(1),
            height: (bottom - top).max(1),
        },
        radius: (radius * scale).round().max(0.0) as i32,
        flat_top,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_rect_follows_the_drawn_pill() {
        let r = pill_round_rect((165.0, 0.0, 190.0, 32.0), 16.0, true, 1.5);
        assert_eq!(
            r,
            RoundRect {
                rect: PhysRect {
                    x: 248,
                    y: 0,
                    width: 285,
                    height: 48
                },
                radius: 24,
                flat_top: true
            }
        );
    }

    #[test]
    fn pill_is_centered_and_clamped_to_window() {
        let theme = Theme::default();
        let r = pill_rect(&theme, Shape::Compact, 1.0);
        // (520 - 190) / 2 = 165, moins la marge.
        assert_eq!(
            r,
            PhysRect {
                x: 163,
                y: 0,
                width: 194,
                height: 34
            }
        );

        let full = pill_rect(&theme, Shape::Expanded, 1.0);
        assert_eq!(
            full,
            PhysRect {
                x: 0,
                y: 0,
                width: 520,
                height: 170
            }
        );
    }

    #[test]
    fn scales_with_dpi() {
        let theme = Theme::default();
        let r = pill_rect(&theme, Shape::Compact, 1.5);
        assert_eq!(
            r,
            PhysRect {
                x: 245,
                y: 0,
                width: 290,
                height: 50
            }
        );
    }

    #[test]
    fn top_offset_moves_the_pill() {
        let theme = Theme {
            top_offset: 10.0,
            ..Theme::default()
        };
        let r = pill_rect(&theme, Shape::Attention, 1.0);
        assert_eq!(r.y, 8);
        assert_eq!(r.height, 36 + 4);
    }

    #[test]
    fn notification_fits_in_the_window() {
        let theme = Theme::default();
        assert_eq!(notification_size(&theme), Size::new(380.0, 58.0));
        let small = Theme {
            expanded: Size::new(320.0, 50.0),
            ..Theme::default()
        };
        assert_eq!(notification_size(&small), Size::new(320.0, 50.0));
    }

    #[test]
    fn small_taskbar_caps_the_pills() {
        let theme = Theme::default();
        // Barre réduite (32 px) : l'attention (36) descend à 32.
        let small = fit_under_taskbar(&theme, Some(32.0));
        assert_eq!(small.compact.height, 32.0);
        assert_eq!(small.attention.height, 32.0);
        assert_eq!(small.expanded, theme.expanded);
        // Barre normale (48 px) ou ailleurs qu'en haut : rien ne change.
        assert_eq!(fit_under_taskbar(&theme, Some(48.0)), theme);
        assert_eq!(fit_under_taskbar(&theme, None), theme);
        // Décalage depuis le haut : la pilule finit au bord de la barre.
        let offset = Theme {
            top_offset: 4.0,
            ..Theme::default()
        };
        assert_eq!(
            fit_under_taskbar(&offset, Some(32.0)).attention.height,
            28.0
        );
        assert_eq!(fit_under_taskbar(&theme, Some(4.0)).compact.height, 16.0);
    }

    #[test]
    fn union_covers_both() {
        let a = PhysRect {
            x: 10,
            y: 0,
            width: 10,
            height: 5,
        };
        let b = PhysRect {
            x: 0,
            y: 2,
            width: 5,
            height: 10,
        };
        assert_eq!(
            a.union(b),
            PhysRect {
                x: 0,
                y: 0,
                width: 20,
                height: 12
            }
        );
    }
}

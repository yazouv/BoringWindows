//! Calcul de la forme de l'île en pixels physiques, partagé entre la zone
//! cliquable Win32 et le placement de la fenêtre.

use bw_config::Theme;

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
    Expanded,
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

#[cfg(test)]
mod tests {
    use super::*;

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

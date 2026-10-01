use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer};

/// Couleur RGBA, écrite `#RRGGBB` ou `#RRGGBBAA` dans la config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 0xFF }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseColorError(String);

impl fmt::Display for ParseColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&bw_i18n::tr!(
            "invalid color {:?} (expected \"#RRGGBB\" or \"#RRGGBBAA\")",
            "couleur invalide {:?} (attendu \"#RRGGBB\" ou \"#RRGGBBAA\")",
            self.0
        ))
    }
}

impl FromStr for Color {
    type Err = ParseColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseColorError(s.to_owned());
        let hex = s.strip_prefix('#').ok_or_else(err)?;
        if !(hex.len() == 6 || hex.len() == 8) || !hex.is_ascii() {
            return Err(err());
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| err());
        Ok(Self {
            r: byte(0)?,
            g: byte(2)?,
            b: byte(4)?,
            a: if hex.len() == 8 { byte(6)? } else { 0xFF },
        })
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rgb_and_rgba() {
        assert_eq!("#ff8A3d".parse(), Ok(Color::rgb(0xFF, 0x8A, 0x3D)));
        assert_eq!(
            "#01020304".parse(),
            Ok(Color {
                r: 1,
                g: 2,
                b: 3,
                a: 4
            })
        );
    }

    #[test]
    fn rejects_garbage() {
        for bad in ["", "ff8a3d", "#fff", "#gg0000", "#ff8a3d0", "#ééé"] {
            assert!(bad.parse::<Color>().is_err(), "{bad:?} aurait dû échouer");
        }
    }
}

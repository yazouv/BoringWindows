//! macOS : Spotify et Musique se lisent et se pilotent en AppleScript.
//! Ce fichier ne contient que la logique pure (scripts, lecture de leur
//! sortie), testée sur toutes les plateformes.

use std::time::Duration;

/// Séparateur des champs renvoyés par les scripts (« record separator ») :
/// il n'apparaît pas dans un titre.
const SEP: char = '\u{1e}';

/// Lecteurs pris en charge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Player {
    Spotify,
    Music,
}

impl Player {
    pub const ALL: [Self; 2] = [Self::Spotify, Self::Music];

    pub fn bundle_id(self) -> &'static str {
        match self {
            Self::Spotify => "com.spotify.client",
            Self::Music => "com.apple.Music",
        }
    }

    /// Notification distribuée envoyée à chaque changement (morceau, lecture,
    /// pause).
    pub fn notification(self) -> &'static str {
        match self {
            Self::Spotify => "com.spotify.client.PlaybackStateChanged",
            Self::Music => "com.apple.Music.playerInfo",
        }
    }

    /// État et morceau en cours, champs séparés par [`SEP`].
    pub fn query_script(self) -> String {
        // Spotify donne la durée en ms, Musique en secondes ; seul Spotify
        // a une adresse de pochette.
        let (duration, artwork, id) = match self {
            Self::Spotify => (
                "((duration of t) / 1000) as string",
                "artwork url of t",
                "id of t",
            ),
            Self::Music => (
                "(duration of t) as string",
                "\"\"",
                "(persistent ID of t) as string",
            ),
        };
        format!(
            r#"tell application id "{bundle}"
	set sep to character id 30
	try
		set s to player state as string
		if s is "stopped" then return s
		set t to current track
		return s & sep & (name of t) & sep & (artist of t) & sep & (album of t) & sep & {duration} & sep & (player position as string) & sep & {artwork} & sep & {id}
	on error
		return "stopped"
	end try
end tell"#,
            bundle = self.bundle_id(),
        )
    }

    /// Pochette brute du morceau (Musique seulement).
    pub fn artwork_script(self) -> Option<String> {
        (self == Self::Music).then(|| {
            format!(
                r#"tell application id "{}"
	try
		return raw data of artwork 1 of current track
	end try
end tell"#,
                self.bundle_id()
            )
        })
    }

    pub fn command_script(self, command: Control) -> String {
        let verb = match command {
            Control::PlayPause => "playpause".to_owned(),
            Control::Next => "next track".to_owned(),
            Control::Previous => "previous track".to_owned(),
            Control::Seek(at) => format!("set player position to {:.1}", at.as_secs_f64()),
        };
        format!(r#"tell application id "{}" to {verb}"#, self.bundle_id())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    PlayPause,
    Next,
    Previous,
    Seek(Duration),
}

/// Morceau lu dans la sortie de [`Player::query_script`].
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub playing: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Option<Duration>,
    pub position: Duration,
    /// Adresse de la pochette (Spotify), vide sinon.
    pub artwork_url: String,
    pub id: String,
}

/// `None` : lecteur arrêté, ou sortie inattendue.
pub fn parse_track(output: &str) -> Option<Track> {
    let output = output.trim_end_matches(['\n', '\r']);
    let fields: Vec<&str> = output.split(SEP).collect();
    let [
        state,
        title,
        artist,
        album,
        duration,
        position,
        artwork_url,
        id,
    ] = fields[..]
    else {
        return None;
    };
    let playing = match state {
        "playing" => true,
        "paused" => false,
        _ => return None,
    };
    if title.is_empty() && artist.is_empty() {
        return None;
    }
    Some(Track {
        playing,
        title: title.to_owned(),
        artist: artist.to_owned(),
        album: album.to_owned(),
        duration: parse_seconds(duration).filter(|d| !d.is_zero()),
        position: parse_seconds(position).unwrap_or_default(),
        artwork_url: artwork_url.to_owned(),
        id: id.to_owned(),
    })
}

/// Nombre de secondes écrit par AppleScript, virgule décimale comprise
/// (réglages régionaux français).
pub fn parse_seconds(text: &str) -> Option<Duration> {
    let value: f64 = text.trim().replace(',', ".").parse().ok()?;
    (value.is_finite() && value >= 0.0).then(|| Duration::from_secs_f64(value))
}

/// Données brutes affichées par osascript : `«data JPEG FFD8…»` (le type
/// sur quatre caractères, puis l'hexadécimal).
pub fn parse_raw_data(output: &str) -> Option<Vec<u8>> {
    let start = output.find("«data ")? + "«data ".len();
    let rest = output.get(start..)?;
    let hex = rest.get(4..)?;
    let hex = &hex[..hex.find('»')?];
    if hex.len() % 2 != 0 || hex.is_empty() {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(fields: &[&str]) -> String {
        fields.join("\u{1e}") + "\n"
    }

    #[test]
    fn parses_tracks() {
        let t = parse_track(&line(&[
            "playing",
            "Digital Love",
            "Daft Punk",
            "Discovery",
            "301,5",
            "12.25",
            "https://i.scdn.co/image/abc",
            "spotify:track:1",
        ]))
        .unwrap();
        assert!(t.playing);
        assert_eq!(t.title, "Digital Love");
        assert_eq!(t.duration, Some(Duration::from_millis(301_500)));
        assert_eq!(t.position, Duration::from_millis(12_250));
        assert_eq!(t.artwork_url, "https://i.scdn.co/image/abc");

        let paused = parse_track(&line(&["paused", "A", "", "", "0", "x", "", "1"])).unwrap();
        assert!(!paused.playing);
        assert_eq!(paused.duration, None);
        assert_eq!(paused.position, Duration::ZERO);

        assert_eq!(parse_track("stopped\n"), None);
        assert_eq!(parse_track(""), None);
        assert_eq!(
            parse_track(&line(&["playing", "", "", "", "1", "1", "", ""])),
            None
        );
    }

    #[test]
    fn parses_raw_data() {
        assert_eq!(
            parse_raw_data("«data JPEGFFD8FF00»\n"),
            Some(vec![0xFF, 0xD8, 0xFF, 0x00])
        );
        assert_eq!(
            parse_raw_data("«data tdta89504E47»"),
            Some(vec![0x89, 0x50, 0x4E, 0x47])
        );
        assert_eq!(parse_raw_data("«data JPEGFFD»"), None);
        assert_eq!(parse_raw_data("«data JPEGZZ»"), None);
        assert_eq!(parse_raw_data(""), None);
    }

    #[test]
    fn scripts_name_the_right_app() {
        assert!(
            Player::Spotify
                .query_script()
                .contains("com.spotify.client")
        );
        assert!(Player::Music.query_script().contains("persistent ID"));
        assert_eq!(
            Player::Music.command_script(Control::Seek(Duration::from_millis(61_500))),
            r#"tell application id "com.apple.Music" to set player position to 61.5"#
        );
        assert!(Player::Spotify.artwork_script().is_none());
    }
}

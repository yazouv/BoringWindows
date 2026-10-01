//! Lien de visio (Teams, Meet, Zoom, Webex…) trouvé dans une invitation.

const HOSTS: &[&str] = &[
    "teams.microsoft.com",
    "teams.live.com",
    "meet.google.com",
    "zoom.us",
    "zoom.com",
    "webex.com",
    "whereby.com",
    "gotomeeting.com",
    "meet.jit.si",
    "chime.aws",
    "discord.gg",
    "facetime.apple.com",
];

/// Premier lien de réunion trouvé dans les textes donnés, dans l'ordre.
pub fn find_join_url<'a>(texts: impl IntoIterator<Item = &'a str>) -> Option<String> {
    texts.into_iter().find_map(|text| {
        text.split(|c: char| {
            c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | '(' | ')' | '[' | ']')
        })
        .filter(|w| w.starts_with("https://"))
        .map(|w| w.trim_end_matches(['.', ',', ';', ':', '!', '?']))
        .find(|url| {
            let host = url["https://".len()..]
                .split(['/', '?', '#'])
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            HOSTS
                .iter()
                .any(|h| host == *h || host.ends_with(&format!(".{h}")))
        })
        .map(str::to_owned)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_meeting_links() {
        let teams = "Rejoindre : <https://teams.microsoft.com/l/meetup-join/19%3ameeting_abc/0?context=x>\nAutre";
        assert_eq!(
            find_join_url([teams]).as_deref(),
            Some("https://teams.microsoft.com/l/meetup-join/19%3ameeting_abc/0?context=x")
        );
        assert_eq!(
            find_join_url(["", "Visio : https://meet.google.com/abc-defg-hij."]).as_deref(),
            Some("https://meet.google.com/abc-defg-hij")
        );
        assert_eq!(
            find_join_url(["https://acme.zoom.us/j/123?pwd=x"]).as_deref(),
            Some("https://acme.zoom.us/j/123?pwd=x")
        );
    }

    #[test]
    fn ignores_other_links() {
        assert_eq!(
            find_join_url(["https://example.com/doc https://notzoom.us.evil.com"]),
            None
        );
        assert_eq!(find_join_url(["http://meet.google.com/x"]), None);
    }
}

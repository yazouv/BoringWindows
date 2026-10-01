//! Vérifie que chaque texte `@tr("…")` des fichiers Slint a sa traduction
//! française dans `lang/fr/LC_MESSAGES/bw-app.po`.

const SLINT: [(&str, &str); 2] = [
    ("island.slint", include_str!("../ui/island.slint")),
    ("settings.slint", include_str!("../ui/settings.slint")),
];
const PO_FR: &str = include_str!("../lang/fr/LC_MESSAGES/bw-app.po");

/// Chaînes entre guillemets qui suivent `prefix` (échappements `\"` gérés).
fn strings_after<'a>(text: &'a str, prefix: &'a str) -> impl Iterator<Item = String> + 'a {
    text.match_indices(prefix).map(move |(i, _)| {
        let mut out = String::new();
        let mut chars = text[i + prefix.len()..].chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => out.extend(chars.next()),
                '"' => break,
                c => out.push(c),
            }
        }
        out
    })
}

#[test]
fn every_slint_text_is_translated() {
    let ids: Vec<String> = strings_after(PO_FR, "msgid \"").collect();
    let strs: Vec<String> = strings_after(PO_FR, "msgstr \"").collect();
    let mut missing = Vec::new();
    for (file, text) in SLINT {
        for id in strings_after(text, "@tr(\"") {
            match ids.iter().position(|m| *m == id) {
                Some(i) if !strs[i].is_empty() => {}
                _ => missing.push(format!("{file} : {id}")),
            }
        }
    }
    assert!(
        missing.is_empty(),
        "textes sans traduction :\n{}",
        missing.join("\n")
    );
}

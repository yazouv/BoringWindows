# iCloud

iCloud publie un calendrier sous forme de **calendrier public** : un lien
`webcal://` difficile à deviner, que BoringWindows accepte tel quel.

**Depuis un iPhone ou un iPad**

1. Ouvre l'application **Calendrier** › **Calendriers** (en bas).
2. Touche ⓘ à côté du calendrier voulu.
3. Active **Calendrier public**, puis **Partager le lien…** › **Copier**.

**Depuis un Mac** : application Calendrier, clic droit sur le calendrier ›
**Partager le calendrier…** › coche **Calendrier public** › copie l'adresse.

**Depuis [iCloud.com](https://www.icloud.com/calendar)** : icône de partage à
côté du calendrier › **Calendrier public** › **Copier le lien**.

Puis dans `config.toml` :

```toml
[[modules.calendar.sources]]
name = "iCloud"
url = "webcal://p12-caldav.icloud.com/published/2/…"
```

**Remarques**

- « Public » veut dire : quiconque a le lien peut lire le calendrier. Garde-le
  pour toi.
- Pour couper l'accès : désactive **Calendrier public** (puis réactive-le pour
  obtenir un nouveau lien).

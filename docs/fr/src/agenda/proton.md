# Proton Calendar

Proton peut partager un calendrier par **lien** en lecture seule.

1. Ouvre [Proton Calendar](https://calendar.proton.me) sur le web.
2. ⚙️ **Paramètres** › **Calendriers** › clique sur le calendrier.
3. Section **Partager avec n'importe qui** : **Créer un lien**.
4. Choisis **Afficher tous les détails de l'événement**, puis copie le lien.

Puis dans `config.toml` :

```toml
[[modules.calendar.sources]]
name = "Proton"
url = "https://calendar.proton.me/api/calendar/v1/url/…/calendar.ics?…"
```

**Remarques**

- Ce lien contient la clé de déchiffrement de ton calendrier : traite-le comme
  un mot de passe.
- Pour couper l'accès : même page, supprime le lien.

# Outlook et Microsoft 365

Outlook sait **publier un calendrier** sous forme de lien ICS. La démarche est
la même pour Outlook.com (compte perso) et Microsoft 365 (compte pro ou école).

1. Ouvre Outlook sur le web :
   - compte perso : [outlook.live.com](https://outlook.live.com/calendar)
   - compte pro / école : [outlook.office.com](https://outlook.office.com/calendar)
2. ⚙️ **Paramètres** › **Calendrier** › **Calendriers partagés**.
3. Section **Publier un calendrier** :
   - choisis le calendrier ;
   - choisis **Peut afficher tous les détails** (sinon tu ne verras que
     « Occupé », sans titre ni lien de réunion) ;
   - clique sur **Publier**.
4. Deux liens apparaissent : copie le lien **ICS** (pas le lien HTML).

Puis dans `config.toml` :

```toml
[[modules.calendar.sources]]
name = "Outlook"
url = "https://outlook.office365.com/owa/calendar/…/calendar.ics"
```

**Remarques**

- Les liens Teams des invitations sont détectés : bouton **Rejoindre**.
- Les fuseaux horaires propres à Outlook (« Romance Standard Time »…) sont
  gérés.
- Outlook met à jour ce lien avec quelques minutes de retard.
- **Pas de section « Publier un calendrier » ?** Ton organisation l'a
  désactivée. Dans ce cas, la connexion directe à Microsoft 365 est prévue
  dans une prochaine version.
- **Lien compromis ?** Même page, **Annuler la publication**, puis publie à
  nouveau : un nouveau lien est créé.
- L'application Outlook « classique » de bureau n'a pas cette option : passe
  par Outlook sur le web, une fois suffit.

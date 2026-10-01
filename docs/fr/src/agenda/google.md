# Google Agenda

Google donne à chaque agenda une **adresse secrète au format iCal**.

1. Ouvre [Google Agenda](https://calendar.google.com) dans un navigateur
   (pas dans l'application mobile).
2. En haut à droite : ⚙️ **Paramètres**.
3. Dans la colonne de gauche, sous **Paramètres de mes agendas**, clique sur
   l'agenda voulu.
4. Descends jusqu'à **Intégrer l'agenda**.
5. Copie **Adresse secrète au format iCal** (elle se termine par `basic.ics`).

> Ne prends pas l'« Adresse publique au format iCal » : elle ne fonctionne que
> si l'agenda est rendu public.

Puis dans `config.toml` :

```toml
[[modules.calendar.sources]]
name = "Google"
url = "https://calendar.google.com/calendar/ical/…/private-…/basic.ics"
```

Répète l'opération pour chaque agenda (perso, famille…) : un bloc par agenda.

**Remarques**

- Google met parfois plusieurs heures à refléter une modification dans ce
  lien. Pour les changements de dernière minute, l'application Google reste
  plus rapide.
- Les liens Google Meet des invitations sont détectés : bouton **Rejoindre**.
- **Lien compromis ?** Même page, bouton **Réinitialiser** sous l'adresse
  secrète : l'ancien lien cesse de fonctionner.
- Avec un compte Google Workspace (travail, école), l'administrateur peut
  avoir désactivé cette option.

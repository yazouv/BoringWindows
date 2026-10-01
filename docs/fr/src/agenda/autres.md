# Autres calendriers et fichiers .ics

## Emploi du temps, ENT, planning partagé

Beaucoup d'emplois du temps universitaires (ADE, HyperPlanning, Celcat…),
d'ENT, de clubs ou d'outils de réservation proposent un lien d'**export iCal**
ou **abonnement** qui se met à jour tout seul. Cherche « iCal », « ICS »,
« Exporter », « S'abonner » ou « Synchroniser » sur la page de l'emploi du
temps. Le lien se termine souvent par `.ics`, ou commence par `webcal://`.

```toml
[[modules.calendar.sources]]
name = "EDT"
url = "https://edt.exemple.fr/api/v1/calendar/G7a.ics"
```

La salle de chaque cours est affichée à côté de l'heure : `Dans 3 min · 112 ·
R5A.07 - Automatisation…`.

Pour voir aussi le lendemain dès le matin, augmente l'horizon :

```toml
[modules.calendar]
lookahead_hours = 48
```

## Fichier .ics sur ton disque

Un fichier exporté ou reçu par mail marche aussi : indique son chemin.

```toml
[[modules.calendar.sources]]
name = "Planning"
url = "C:/Users/moi/Documents/planning.ics"
```

Écris le chemin avec des `/`, comme ci-dessus. Si tu colles un chemin avec des
`\`, mets-le entre **guillemets simples** (sinon le fichier de configuration
est invalide) :

```toml
url = 'C:\Users\moi\Documents\planning.ics'
```

Le fichier est relu toutes les 10 minutes : si tu le remplaces par une
nouvelle version, elle est prise en compte.

## Compatibilité

Tout calendrier au format iCalendar standard fonctionne : Fastmail, Nextcloud,
Thunderbird, Zimbra, Zoho… Si l'un d'eux s'affiche mal, ouvre une
[issue](https://github.com/yazouv/BoringWindows/issues) en joignant un
extrait du fichier (en retirant les informations personnelles).

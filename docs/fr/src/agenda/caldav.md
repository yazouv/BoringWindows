# CalDAV : iCloud, Fastmail, Nextcloud…

CalDAV est le protocole de synchronisation des agendas. Avec lui, BoringWindows
lit **tous les agendas de ton compte** (pas besoin de publier chaque calendrier
ni de copier un lien ICS). En échange, il faut un identifiant et un **mot de
passe d'application** : jamais ton mot de passe principal.

Le mot de passe est rangé dans le **Gestionnaire d'identifiants de Windows**,
pas dans `config.toml`, qui contient seulement `password = "secret:…"`.

## Où trouver quoi

| Service | Adresse du serveur | Identifiant | Mot de passe |
|---|---|---|---|
| iCloud | `https://caldav.icloud.com` | ton identifiant Apple | [mot de passe spécifique à l'app](https://account.apple.com/account/manage) : Connexion et sécurité › Mots de passe pour app |
| Fastmail | `https://caldav.fastmail.com` | ton adresse Fastmail | Réglages › Confidentialité et sécurité › Mots de passe d'app (accès « Calendriers ») |
| Nextcloud | `https://ton-serveur/remote.php/dav` | ton nom d'utilisateur | Paramètres › Sécurité › Créer un nouveau mot de passe d'application |

Une adresse précise d'agenda fonctionne aussi : BoringWindows ne lit alors que
celui-là.

## Ajouter le compte

Réglages › **Agenda** › **CalDAV** › adresse, identifiant, mot de passe
d'application › **Tester** › **Ajouter**.

À la main, dans `config.toml` :

```toml
[[modules.calendar.sources]]
name = "iCloud"
kind = "caldav"
url = "https://caldav.icloud.com"
username = "moi@icloud.com"
password = "secret:caldav-18f3a2"   # rangé dans le Gestionnaire d'identifiants
```

Un `password` écrit en clair fonctionne aussi (utile sous Linux, où il n'y a pas
de coffre), mais il est alors lisible par quiconque ouvre le fichier.

## Remarques

- Les liens ICS privés (Google, Outlook…) ajoutés depuis la fenêtre de réglages
  sont rangés de la même façon : `url = "secret:ics-…"`. Retirer le calendrier
  des réglages supprime aussi le secret.
- Si le **Test** échoue : vérifie l'adresse (sans `/principal`), l'identifiant
  et que le mot de passe est bien un mot de passe *d'application*.

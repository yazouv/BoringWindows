# Réseau

Une petite **icône** à côté de l'heure, dans l'île ouverte, dit où tu en es :

| Icône | Signification |
|---|---|
| Wi-Fi | connecté en Wi-Fi |
| Prise réseau | connecté en filaire |
| Bouclier | un VPN est connecté |
| Wi-Fi barré (rouge) | pas de connexion Internet |

**Survole l'icône** : la date laisse la place au détail, par exemple
« Maison · ↓ 2,3 Mo/s  ↑ 120 Ko/s », ou « Maison · VPN · ↓ … » avec un VPN.

L'état de la connexion vient d'événements de Windows : rien n'est interrogé en
boucle.

## Le débit dans la pilule repliée

Par défaut, le débit n'est visible que dans l'île ouverte. Tu peux aussi
l'afficher **dans la pilule repliée**, avec `compact_speed` :

| Valeur | Effet |
|---|---|
| `off` | jamais (par défaut) |
| `active` | seulement quand ça circule : un téléchargement, un envoi… Il disparaît après quelques secondes de calme |
| `always` | tout le temps |

La pilule affiche alors « ↓ 2,3 Mo/s   ↑ 120 Ko/s ». Elle passe **après** la
musique et Claude (ordre de `layout.compact`) : tant qu'un morceau joue, c'est
lui qui occupe la pilule. Mets `network` en tête de la liste pour inverser :

```toml
[layout]
compact = ["network", "claude", "media", "calendar"]
```

Le débit se mesure seulement quand il est visible : île ouverte, ou pilule
repliée réglée sur `active` ou `always`. Avec `off`, rien n'est mesuré île
fermée.

Avec un VPN connecté, les octets passent à la fois dans le tunnel et sur la
carte réelle : l'île garde alors l'interface la plus chargée plutôt que la
somme, pour ne pas compter deux fois.

## Annonces

La pilule prévient quand :

- la **connexion Internet est perdue** (« Plus de connexion Internet ») ou
  **rétablie** (« Connexion rétablie ») ;
- un **VPN se connecte** ou se **déconnecte**. Une déconnexion est signalée
  comme urgente : le trafic n'est plus protégé ;
- tu **changes de réseau Wi-Fi** (« Connecté à Bureau »), si tu l'actives.

Chaque annonce se règle séparément.

> Le VPN est repéré à un **adaptateur réseau en service**, avec une adresse IP
> utilisable, dont le type est « tunnel » (VPN de Windows) ou dont le nom est
> celui d'un VPN : OpenVPN (cartes TAP, Wintun, DCO), WireGuard, FortiClient,
> Cisco AnyConnect, GlobalProtect, Mullvad, NordVPN… Un client qui nomme sa carte
> autrement ne sera pas reconnu.

## Réglages

Réglages › **Système** › *Réseau*, ou dans `config.toml` :

```toml
[modules.network]
enabled = true
compact_speed = "off"        # off, active ou always
unit = "bytes"               # bytes (Mo/s) ou bits (Mb/s)
refresh_secs = 1             # 1 à 10
active_kb_per_sec = 50       # mode active : seuil d'affichage (Ko/s)
announce_connection = true   # perte et retour de la connexion
announce_vpn = true          # VPN connecté ou déconnecté
announce_wifi = false        # changement de réseau Wi-Fi
show_secs = 5                # durée d'une annonce, 2 à 30
```

Windows pour l'état de la connexion (Wi-Fi, VPN). Le débit marche aussi sous
macOS et Linux.

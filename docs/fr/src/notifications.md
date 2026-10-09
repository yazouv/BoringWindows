# Notifications

Quand une application t'envoie une notification Windows (un message Discord,
Slack, Teams, WhatsApp, un mail Outlook…), l'île l'annonce : la pilule
s'élargit, une pastille à l'initiale de l'application apparaît avec une onde de
sa couleur, puis l'expéditeur et le message. Au bout de quelques secondes, la
pilule reprend sa taille.

Tant que tu ne les as pas regardées, les notifications laissent **un point de
couleur** par application à droite de la pilule.

## L'onglet « Notifs »

Ouvre l'île et clique sur l'onglet **Notifs** (en haut à droite), ou ouvre
l'île pendant qu'une notification s'affiche : elle s'ouvre directement sur cet
onglet.

- Les 4 dernières notifications, avec leur ancienneté.
- Un **clic** sur une ligne ouvre (ou ramène) l'application qui l'a envoyée.
- La **×** au survol efface la notification, aussi du centre de notifications
  de Windows. **Tout effacer** efface celles de la liste.
- Le badge de l'onglet compte les notifications arrivées depuis ta dernière
  visite.

## Réglages

Activé par défaut. Réglages › **Général** › « Afficher les notifications des
applications dans l'île », ou dans `config.toml` :

```toml
[modules.notifications]
enabled = true
show_secs = 5          # durée d'affichage d'une nouvelle notification (1 à 30)
show_content = true    # false : seulement le nom de l'application, sans le message
ignore = ["docker"]    # applications à ignorer (morceau de nom)
```

`show_content = false` est utile si tu partages ton écran : l'île annonce
« Discord · Nouvelle notification » sans montrer qui t'écrit ni quoi.

## Bon à savoir

- BoringWindows lit le **centre de notifications** de Windows. Si l'île ne
  montre rien, vérifie Paramètres Windows › Confidentialité et sécurité ›
  **Notifications** : l'accès aux notifications doit être autorisé pour les
  applications de bureau.
- Une application dont tu as coupé les notifications dans Windows n'apparaît
  pas non plus dans l'île.
- Windows ne prévient pas les applications non empaquetées quand une
  notification arrive : l'île relit la liste toutes les 2 secondes. C'est un
  appel léger, mais c'est la seule exception à la règle « pas de polling » de
  BoringWindows. Pour l'éviter, désactive le module.
- La couleur vient de l'application (bleu Discord, vert WhatsApp…). Une
  application inconnue reçoit une couleur stable tirée de son nom.
- Une notification passe devant tout dans la pilule, sauf une demande qui
  attend ta réponse (permission de Claude Code).
- Windows seulement.

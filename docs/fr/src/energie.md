# Batterie et Bluetooth

## Batterie du PC

Sur un portable, l'île montre un instant ce qui arrive à la batterie, avec un
éclair et une jauge :

- **chargeur branché** : « En charge · 54 % », éclair et jauge verts ;
- **chargeur débranché** : « Sur batterie · 54 % » ;
- **charge terminée** (100 %, chargeur branché) ;
- **batterie faible** : au passage sous le seuil (20 % par défaut), puis à la
  moitié du seuil (10 %), en rouge.

Activé par défaut. Sur un PC de bureau, sans batterie, le module ne fait rien.

## Appareils Bluetooth

Quand un casque, des écouteurs, une souris ou un clavier Bluetooth se
connecte, l'île affiche son nom et son niveau de batterie : « WH-1000XM5 ·
80 % ». Le niveau arrive souvent quelques secondes après la connexion : l'île
le montre alors à son tour. Une déconnexion et une batterie faible (sous le
seuil) s'affichent aussi.

- Le niveau vient de Windows : c'est celui que montre Paramètres › Bluetooth et
  appareils. Un appareil qui ne le communique pas à Windows s'affiche sans
  jauge.
- Les casques et écouteurs ont une icône de casque, les autres appareils
  l'icône Bluetooth.

Activé par défaut.

## Réglages

Réglages › **Notifications** › « Batterie » et « Bluetooth ». Ou dans
`config.toml` :

```toml
[modules.battery]
enabled = true
show_secs = 4     # durée d'affichage (1 à 10 s)
low_percent = 20  # seuil de batterie faible (5 à 50 %)

[modules.bluetooth]
enabled = true
show_secs = 4
low_percent = 20
```

Tout passe par des événements de Windows (gestion de l'alimentation,
observateurs d'appareils) : rien n'est interrogé en boucle, rien ne tourne au
repos. Windows seulement.

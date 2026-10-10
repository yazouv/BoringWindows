# Météo

La météo du moment s'affiche à côté de la date, dans l'île ouverte : un
pictogramme (soleil, nuages, pluie, neige, orage…) et la température.
**Survole-la** : la date laisse la place au détail du jour, par exemple
« Lyon · Éclaircies · 9° / 16° ».

Désactivé par défaut : il faut une ville.

Réglages › **Météo** : active « Afficher la météo », écris ta ville et choisis
l'unité. Ou dans `config.toml` :

```toml
[modules.weather]
enabled = true
city = "Lyon"            # ou "Lyon, France" pour lever une ambiguïté
units = "celsius"        # ou "fahrenheit"
refresh_minutes = 30     # 10 à 180
```

Pour un lieu précis, donne ses coordonnées à la place de la ville (elles
passent devant `city`) :

```toml
[modules.weather]
enabled = true
latitude = 45.76
longitude = 4.84
```

## D'où viennent les données

De [open-meteo.com](https://open-meteo.com) : gratuit, sans compte ni clé.
Seuls le nom de la ville (une fois, pour trouver ses coordonnées) et les
coordonnées partent sur Internet. Si la requête échoue (pas de réseau au
réveil…), un nouvel essai a lieu deux minutes plus tard ; le journal en dit
plus (`météo : …`).

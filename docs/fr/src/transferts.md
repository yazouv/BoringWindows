# Transferts de fichiers

Quand tu **copies** des fichiers dans l'Explorateur ou que tu **télécharges**
quelque chose avec ton navigateur, une **barre de progression** apparaît au
bord bas de la pilule repliée. Elle se superpose à ce que la pilule montre
déjà (la musique par exemple) : tu n'as rien à ouvrir pour voir où ça en est.

## Copies, déplacements, suppressions

L'Explorateur ouvre une fenêtre de progression pour les opérations un peu
longues. L'île la suit : la barre avance avec le pourcentage, et la pilule
affiche par exemple « 42 % · Copie de 3 éléments de Documents vers Sauvegarde »
quand rien de plus important n'occupe la pilule.

Tout vient des événements de Windows (apparition et changement de titre de la
fenêtre), rien n'est interrogé en boucle. Windows seulement.

## Téléchargements

Les navigateurs écrivent leur téléchargement dans un fichier provisoire
(`.crdownload` pour Chrome, Edge et Opera, `.part` pour Firefox…), puis le
renomment une fois terminé. L'île surveille ton dossier Téléchargements et suit
ces fichiers.

Les navigateurs ne donnent pas la **taille finale** : la barre va et vient au
lieu de se remplir, et la pilule montre la taille reçue et la vitesse, par
exemple « video.mp4 · 128 Mo · 3,2 Mo/s ». Avec plusieurs téléchargements,
elle les additionne (« 2 téléchargements · 340 Mo · 5,1 Mo/s »).

Le dossier n'est relevé (une fois par seconde) que **tant qu'un fichier
provisoire existe** ; le reste du temps, l'île attend simplement un
changement dans le dossier.

Un fichier provisoire qui n'a pas été écrit depuis plus de **30 secondes**
(téléchargement abandonné ou en pause, `.part` oublié par Firefox) n'est pas
compté : l'île ne reste pas bloquée sur un vieux fichier. Il est repris dès
qu'il grossit à nouveau.

Un téléchargement annulé disparaît sans rien annoncer.

## Annonces

À la fin d'un transfert, la pilule affiche « Téléchargé : video.mp4 » ou
« Opération terminée » quelques secondes. Une copie annulée n'est pas annoncée.

## Réglages

Réglages › **Système** › *Transferts*, ou dans `config.toml` :

```toml
[modules.transfers]
enabled = true
copies = true              # copies, déplacements et suppressions de l'Explorateur
downloads = true           # téléchargements des navigateurs
downloads_folder = ""      # vide : le dossier Téléchargements du système
announce_done = true       # annoncer la fin d'un transfert
show_secs = 5              # durée de l'annonce, 2 à 30
```

Si tes téléchargements vont ailleurs que dans le dossier Téléchargements de
Windows, indique-le dans `downloads_folder`.

# Tâches rapides

Une liste de **cases à cocher** dans l'île, pour noter ce qu'il y a à faire sans
quitter ce que tu fais. Désactivée par défaut : Réglages › **Système** ›
*Tâches*, ou `enabled = true` dans `[modules.tasks]`.

## L'onglet Tâches

Un onglet **Tâches** s'ajoute à l'île ouverte, avec le nombre de tâches à faire
en pastille. Dans l'onglet :

- **clique une tâche** pour la cocher (elle passe en grisé, à la fin de la
  liste), clique-la encore pour la décocher ;
- **×** (au survol) la retire ;
- **+ Ajouter une tâche** ouvre la boîte d'ajout ;
- **Vider les faites** retire toutes les tâches cochées ;
- quand il y en a plus que ce qui tient à l'écran, « +3 » dit combien sont
  masquées.

## Ajouter une tâche

L'île ne prend jamais le clavier (c'est ce qui lui permet de ne pas voler le
focus de ce que tu tapes) : on ne peut donc pas écrire dedans. Le raccourci
**Ctrl+Alt+N** ouvre à la place une petite boîte, **sous l'île**, qui reçoit le
clavier :

- **Entrée** ajoute la tâche et referme la boîte ;
- **Échap** annule.

Le raccourci se change dans Réglages › *Tâches* › *Ajouter une tâche*, ou avec
`quick_add` dans `[hotkeys]`. Il n'est actif que si les tâches sont activées.

## Le fichier

Les tâches vivent dans un fichier **Markdown**, `tasks.md`, à côté de
`config.toml` (Réglages › *Tâches* › *Ouvrir*) :

```markdown
# Courses

- [ ] Acheter du lait
- [x] Appeler Marie
Une note libre.
- [ ] Payer la facture
```

Seules les lignes `- [ ] …` et `- [x] …` (ou avec `*` et `+`) sont des tâches.
**Tout le reste est conservé tel quel** : titres, notes, lignes vides. Tu peux
donc le modifier avec n'importe quel éditeur, le ranger dans Obsidian, ou le
synchroniser avec un service de fichiers.

L'île relit le fichier à chaque ouverture, et avant chaque modification : une
tâche ajoutée à la main apparaît sans redémarrer, et rien n'est écrasé. Une
tâche ajoutée depuis l'île va à la suite des autres. Les tâches sont limitées
à 200 caractères et tiennent sur une ligne.

## Réglages

```toml
[modules.tasks]
enabled = false      # désactivé par défaut
file = ""            # vide : tasks.md à côté de config.toml

[hotkeys]
quick_add = "Ctrl+Alt+N"
```

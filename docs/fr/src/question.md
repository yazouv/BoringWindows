# Questions à Claude ou Gemini

Tape une question, la réponse s'affiche **dans l'île**. Pratique pour une
définition, une commande oubliée ou une traduction, sans ouvrir un navigateur.
Désactivé par défaut : Réglages › **Système** › *Questions à Claude ou Gemini*,
ou `enabled = true` dans `[modules.ask]`.

## Poser une question

L'île ne prend jamais le clavier (c'est ce qui lui permet de ne pas voler le
focus de ce que tu tapes). Le raccourci **Ctrl+Alt+Space** ouvre à la place une
petite boîte **sous l'île** :

- **Entrée** envoie la question et referme la boîte ;
- **Échap** annule ;
- la liste sous le champ **choisit le modèle** : il est retenu pour la fois
  suivante.

Pendant que le modèle répond, la pilule affiche « Question à Claude… ». À
l'arrivée de la réponse, l'île s'ouvre toute seule sur une **carte** qui
recouvre les onglets :

- la réponse défile à la molette si elle est longue ;
- **Reposer** rouvre la boîte avec la question, pour la modifier ;
- **×** ferme la carte. Elle reste sinon, même si l'île se referme, jusqu'à la
  prochaine question.

En appel ou en partage d'écran ([mode présentation](presentation.md)), l'île ne
s'ouvre pas toute seule : la carte attend que tu l'ouvres.

## Le modèle et la clé

Le modèle choisit aussi le fournisseur : un identifiant qui commence par
`claude` va chez Anthropic, `gemini` chez Google. Proposés :

| Fournisseur | Modèles |
|---|---|
| Claude | `claude-haiku-5-5` (rapide), `claude-sonnet-5-5`, `claude-opus-5-5` |
| Gemini | `gemini-2.5-flash-lite`, `gemini-2.5-flash`, `gemini-2.5-pro` |

Un autre identifiant s'écrit à la main dans `modules.ask.model`.

Il faut la **clé d'API** du fournisseur utilisé (une seule suffit) :
[console.anthropic.com](https://console.anthropic.com) pour Claude,
[aistudio.google.com](https://aistudio.google.com) pour Gemini. Colle-la dans
les réglages et appuie sur Entrée : elle va dans le **Gestionnaire
d'identifiants de Windows**, et `config.toml` n'en garde qu'une référence
(`secret:…`). Ces API sont **payantes à l'usage** (Gemini a un palier gratuit
limité) : une question brève coûte une fraction de centime.

## Ce qui part sur le réseau

Chaque question part **seule**, sans historique, chez le fournisseur choisi, avec
ta clé dans un en-tête (jamais dans l'adresse). Rien d'autre ne quitte ton PC,
et la réponse n'est pas enregistrée : elle disparaît à la fermeture de l'app.

Par défaut le modèle répond brièvement, en texte brut. `system` change cette
consigne, `max_tokens` la longueur maximale.

## Si ça ne marche pas

La carte affiche la raison :

- « clé refusée » : clé erronée, révoquée ou sans crédit ;
- « modèle inconnu » : identifiant mal écrit ou retiré par le fournisseur ;
- « limite ou quota atteint » : attends un peu, ou change de modèle ;
- « Réponse coupée » : augmente `max_tokens`.

## Réglages

```toml
[modules.ask]
enabled = false
model = "claude-sonnet-5-5"
claude_key = ""     # secret:<id> (rempli par les réglages)
gemini_key = ""
max_tokens = 1024   # 64 à 8192
system = ""         # vide : réponses brèves, en texte brut

[hotkeys]
ask = "Ctrl+Alt+Space"
```

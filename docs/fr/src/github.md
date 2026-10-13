# GitHub

Une petite icône de **pull request** s'affiche à côté de l'heure, dans l'île
ouverte. Sa couleur résume la **CI de tes pull requests ouvertes** :

| Couleur | Signification |
|---|---|
| grise | pas de CI, ou pas de pull request ouverte |
| accent | une CI est en cours |
| verte | toute la CI est réussie |
| rouge | au moins une CI est en échec |

Un **point** sur l'icône dit que quelqu'un attend **ta revue**.

**Survole l'icône** : une bulle sous l'en-tête donne le détail, par exemple
« 2 PR ouvertes · CI en échec : api#31 · 1 revue demandée ». **Clique-la** pour
ouvrir la page qui a besoin de toi : la pull request en échec, sinon la
première revue demandée, sinon ta première pull request.

## Annonces

La pilule prévient quand :

- la **CI d'une de tes pull requests échoue** (« CI en échec · api#31 ») ou
  **réussit** après avoir tourné (« CI réussie · api#31 ») ;
- on te **demande une revue** (« Revue demandée · api#7 »).

Plusieurs annonces à la fois : la plus importante s'affiche, avec leur nombre
(« CI en échec · api#31 (+1) »). Un échec passe avant une revue, qui passe
avant un succès. Le premier relevé sert de référence, il n'annonce rien.

## Le jeton

L'île a besoin d'un jeton d'accès GitHub, et le cherche dans cet ordre :

1. **le jeton enregistré** : Réglages › **Système** › *GitHub* › *Jeton
   d'accès*. Colle un [jeton personnel](https://github.com/settings/tokens)
   (droits `repo` et `read:org`) puis Entrée : il part dans le Gestionnaire
   d'identifiants de Windows, et `config.toml` n'en garde qu'une référence
   (`secret:…`) ;
2. **la session du GitHub CLI** : si tu es déjà connecté avec `gh auth login`,
   l'île lui demande son jeton (`gh auth token`). Rien à saisir. Désactive
   *Utiliser la session du GitHub CLI* si tu ne le veux pas.

Un jeton absent ou refusé s'annonce une fois dans la pilule (« GitHub : jeton
absent ou refusé »), puis l'île réessaie toutes les 15 minutes.

## Fréquence

GitHub n'a pas d'événement à écouter sans serveur public : l'île **interroge**
GitHub, **une seule requête** par intervalle (2 minutes par défaut) qui ramène
tes pull requests, leur CI et les revues demandées. C'est l'une des rares
exceptions à la règle « rien n'est interrogé en boucle », comme la météo.
Après un échec réseau (réveil du PC…), elle réessaie au bout de 2 minutes.

## Réglages

```toml
[modules.github]
enabled = false              # désactivé par défaut
token = ""                   # "secret:…" (réglages) ou jeton ; vide : session gh
use_gh_cli = true            # sans jeton, demander le sien à `gh auth token`
refresh_secs = 120           # 30 à 3600
announce_ci = true
announce_reviews = true
show_secs = 6                # durée d'une annonce, 2 à 30
```

> Ne mets pas ton jeton en clair dans `config.toml` si tu partages ce fichier :
> passe par les réglages, qui le rangent dans le coffre de l'OS.

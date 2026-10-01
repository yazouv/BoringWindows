# Plugins (WASM)

Un plugin est un petit module WebAssembly qui publie **une ligne de texte** dans
l'île (et peut demander un peu d'attention). Il est écrit dans n'importe quel
langage qui vise WASM (Rust, Zig, AssemblyScript, C…), et tourne dans un **bac à
sable** : aucun accès aux fichiers, au réseau ni au système, seulement les
quelques fonctions ci-dessous.

Les plugins sont **désactivés par défaut** : on n'exécute du code tiers que si tu
le demandes (Réglages › Général › « Lancer les plugins WASM », ou
`[modules.plugins] enabled = true`).

## Installer un plugin

Dans `%APPDATA%\BoringWindows\plugins\` (bouton **Dossier des plugins** dans les
réglages), un dossier par plugin :

```
plugins/
└─ mon-plugin/
   ├─ plugin.toml
   └─ plugin.wasm
```

```toml
# plugin.toml
name = "Mon plugin"        # nom affiché (défaut : nom du dossier)
wasm = "plugin.wasm"       # fichier WASM du dossier (défaut)
interval_secs = 30         # délai entre deux appels, 5 à 3600 (défaut 30)
```

Pour n'en lancer que certains : `[modules.plugins] only = ["mon-plugin"]`. Un
exemple complet est fourni dans
[`examples/plugins/hello`](https://github.com/yazouv/BoringWindows/tree/main/examples/plugins/hello).

## Écrire un plugin

Le module **exporte** :

| Export | Rôle |
|---|---|
| `memory` | sa mémoire linéaire (obligatoire : l'hôte y lit les textes) |
| `bw_update()` | appelé au démarrage puis toutes les `interval_secs` ; ni paramètre ni résultat |

et peut **importer** (module `"bw"`) :

| Import | Rôle |
|---|---|
| `set_text(ptr: i32, len: i32)` | texte affiché (UTF-8, 120 caractères au plus) |
| `set_attention(level: i32)` | 0 rien, 1 discret (pilule), 2 à regarder ; l'« urgent » est réservé à l'app |
| `log(ptr: i32, len: i32)` | message dans le journal de BoringWindows |
| `now_unix() -> i64` | heure actuelle, en secondes depuis 1970 |

Tout autre import (WASI, système de fichiers, réseau…) fait échouer le
chargement du plugin.

Exemple minimal en WAT :

```wat
(module
  (import "bw" "set_text" (func $set_text (param i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 16) "Bonjour")
  (func (export "bw_update")
    (call $set_text (i32.const 16) (i32.const 7))))
```

## Limites du bac à sable

- **Calcul** : un budget par appel de `bw_update` ; une boucle sans fin est
  interrompue (erreur journalisée).
- **Mémoire** : 16 Mo au plus.
- **Erreurs** : un appel en erreur affiche « ⚠ erreur » ; après 3 erreurs de suite
  le plugin est arrêté jusqu'au prochain rechargement des modules.
- Un plugin ne voit rien de ce que fait l'île (musique, agenda, Claude…) : il ne
  peut que publier du texte.

L'île affiche au plus **deux** lignes de plugins ; `layout.view` peut les
reprendre via `plugin-rows` (voir [Vues personnelles](layouts.md)).

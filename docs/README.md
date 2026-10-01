# Documentation

Site publié sur <https://yazouv.github.io/BoringWindows/> par le workflow
`Docs` à chaque push sur `main`.

| Dossier | Langue | Adresse |
|---|---|---|
| `fr/` | français | `/BoringWindows/` |
| `en/` | anglais | `/BoringWindows/en/` |

## Règles

- **Mêmes fichiers dans chaque langue** (`fr/src/agenda/google.md` ↔
  `en/src/agenda/google.md`) : le bouton FR / EN renvoie vers la page du même
  nom. La CI vérifie que la liste des pages est identique.
- Une modification dans une langue → la même dans l'autre, dans le même
  commit.
- `lang.js` et `lang.css` (le bouton de langue) existent en double, un par
  livre, et doivent rester identiques (vérifié aussi).
- Les captures sont dans `src/images/` de chaque livre.

## Ajouter une langue

1. Copier `en/` vers `xx/`, traduire, régler `language`, `build-dir` et
   `site-url` dans `xx/book.toml`.
2. Ajouter son build au workflow `Docs`, après le français.
3. Adapter `lang.js` (aujourd'hui : bascule fr ↔ en).

## Prévisualiser

```bash
cargo install mdbook
mdbook serve docs/fr --open    # ou docs/en
```

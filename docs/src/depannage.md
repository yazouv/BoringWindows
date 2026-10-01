# Dépannage

## L'île n'apparaît pas

- Regarde si l'icône est dans la zone de notification (flèche ^). Si oui,
  vérifie que **Masquer l'île** n'est pas coché.
- Une application est-elle en plein écran sur cet écran ? L'île se cache
  alors volontairement (`hide_in_fullscreen`).
- Plusieurs écrans : l'île va sur l'écran **principal** par défaut
  (`monitor = "cursor"` pour l'écran de la souris).
- BoringWindows ne se lance qu'une fois : un second lancement ne fait rien si
  le premier tourne déjà.

## « ⚠ config.toml invalide »

Le message indique la ligne et le problème. Les causes les plus courantes :

- **un chemin Windows entre guillemets doubles** : `"C:\Users\…"` →
  écris `"C:/Users/…"` ou `'C:\Users\…'` ;
- **une option mal orthographiée** : les noms sont vérifiés, pour ne pas
  ignorer une faute en silence ;
- **une valeur hors limites** : voir la [référence](configuration.md) ;
- des guillemets « typographiques » collés depuis un traitement de texte :
  utilise `"` ou `'`.

Corrige et enregistre : l'île repasse à la normale aussitôt.

## Claude Code : rien ne s'affiche

1. As-tu installé les hooks (clic droit › **Claude Code : installer les
   hooks…**) **et relancé** tes sessions Claude Code depuis ?
2. Lance le diagnostic, BoringWindows ouvert, dans un autre terminal :

   ```powershell
   boringwindows.exe doctor
   ```

   Il vérifie, dans l'ordre :

   | Point | Si `[!!]` |
   |---|---|
   | 1. Hooks dans `settings.json` | installe les hooks depuis le menu |
   | 2. Relais | relance BoringWindows (il recopie le relais) |
   | 3. BoringWindows en cours d'exécution | lance BoringWindows avant le diagnostic |
   | 4. Relais lancé comme Claude Code | envoie le rapport dans une issue |
   | 5. Journal | montre les derniers appels réels de Claude Code |
   | 6. Test visuel | « diagnostic » doit s'afficher 6 s dans l'île |

   Le rapport est aussi enregistré dans
   `%LOCALAPPDATA%\BoringWindows\doctor.txt`.
3. Le journal du relais (`%LOCALAPPDATA%\BoringWindows\hook.log`) contient une
   ligne par événement reçu de Claude Code : s'il est vide, Claude n'appelle
   pas le relais (hooks pas encore relus : relance Claude).

## Claude Code : une demande reste affichée

Si tu as fait Échap sur une demande, elle disparaît dès le message suivant
que tu envoies à Claude, ou au bout du délai (60 s par défaut).

## Musique : un lecteur n'apparaît pas

BoringWindows affiche ce que Windows affiche dans son overlay de volume
(touches multimédia). Si le lecteur n'y apparaît pas non plus, c'est lui qui ne
publie pas ses informations : certains lecteurs ont une option du type
« Intégration avec les contrôles multimédias de Windows ». Vérifie aussi qu'il
n'est pas dans `ignore`.

## Agenda : rien ne s'affiche

- Lance BoringWindows depuis un terminal : la console indique
  `agenda : Pro — 12 événement(s) à venir`, ou l'erreur rencontrée.
- **0 événement** : l'agenda est peut-être vide sur les prochaines 24 h
  (`lookahead_hours = 48` pour voir plus loin), ou le lien publie seulement
  « Occupé/Libre » (Outlook : choisis « Peut afficher tous les détails »).
- **téléchargement impossible** : copie le lien dans ton navigateur ; s'il
  télécharge un fichier `.ics`, le lien est bon. Sinon régénère-le depuis ton
  service.
- Les modifications récentes peuvent mettre du temps à apparaître : Google et
  Outlook ne mettent pas à jour leur lien ICS instantanément.

## Signaler un problème

Ouvre une [issue](https://github.com/yazouv/BoringWindows/issues) avec ce que
tu as fait, ce que tu attendais, ce qui s'est passé, et si possible les lignes
de la console ou le rapport `doctor`.

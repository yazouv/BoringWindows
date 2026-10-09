# Claude Code

L'île suit tes sessions [Claude Code](https://claude.com/claude-code) : elle
montre ce que fait Claude, te prévient quand il t'attend, et te laisse
**autoriser ou refuser une commande sans revenir au terminal**.

![Pilule : question, demande de permission, puis Claude qui reprend](images/claude-pilule.png)

## Brancher Claude Code

1. Lance BoringWindows.
2. Clic droit sur son icône › **Claude Code : installer les hooks…** (ou
   **Réglages…** › onglet **Claude Code** › **Installer les hooks**).
3. La fenêtre qui s'ouvre liste exactement ce qui sera ajouté à
   `~/.claude/settings.json`. Réponds **Oui**.
4. **Relance tes sessions Claude Code** : elles ne lisent leurs hooks qu'au
   démarrage.

Ce qui se passe :

- une **sauvegarde datée** de `settings.json` est faite avant toute
  modification ;
- seules les entrées de BoringWindows sont ajoutées : tes autres réglages et
  tes propres hooks ne bougent pas ;
- le relais est copié dans `%LOCALAPPDATA%\BoringWindows\bin\bw-hook.exe` ;
  il est remis à jour automatiquement quand BoringWindows est mis à jour ;
- le même menu (« retirer les hooks… ») enlève tout proprement.

## Ce que montre l'île

| Pilule | Signification |
|---|---|
| ● gris · `projet · Bash` | Claude travaille (outil en cours) |
| ● gris · `projet · réfléchit…` | Claude rédige sa réponse |
| ● orange · `projet · autoriser Bash ?` | **demande de permission**, à traiter dans l'île |
| ● orange · `projet · te pose une question` | question ou plan à valider : **réponds dans le terminal** |
| ● orange · `projet · attend ta réponse` | Claude a fini et attend ton prochain message |
| `projet · terminé` | fin de tour (quelques secondes) |

Avec plusieurs sessions, la plus urgente occupe la pilule (`(+2)` indique les
autres) et l'île ouverte les liste toutes. **Clique sur une session** pour
ramener son terminal (Windows Terminal, VS Code…) au premier plan.

Un **son système** retentit chaque fois que Claude se met à t'attendre.

## Répondre à une demande de permission

Quand Claude veut lancer une commande qui demande ton accord, survole l'île :
elle affiche l'outil et la commande, avec trois boutons.

- **Autoriser** : Claude exécute la commande.
- **Refuser** : Claude ne l'exécute pas et continue autrement.
- **Terminal** : la question repasse dans le terminal, qui revient au premier
  plan (pour choisir « toujours autoriser », par exemple).

Sans réponse au bout de 60 secondes, la question repasse aussi dans le
terminal. Si tu réponds directement dans le terminal, la demande disparaît de
l'île.

Les **questions** de Claude (choix multiples, validation d'un plan) ne passent
pas par ces boutons : elles s'affichent normalement dans le terminal, l'île te
signale juste « te pose une question ».

## Réglages

Dans [`[modules.claude]`](configuration.md#modulesclaude) :

```toml
[modules.claude]
permissions = true          # répondre aux permissions depuis l'île
permission_wait_secs = 60   # délai avant de rendre la main au terminal
sound = true                # son quand Claude t'attend
```

## Si rien ne s'affiche

Lance le diagnostic, BoringWindows ouvert, dans un autre terminal :

```powershell
boringwindows.exe doctor
```

Il vérifie toute la chaîne et dit où elle casse. Détails dans
[Dépannage](depannage.md#claude-code--rien-ne-saffiche).

## Confidentialité

Le relais n'envoie à l'île qu'un **résumé** : nom du projet, outil, commande ou
nom de fichier. Jamais le contenu de tes fichiers ni tes messages à Claude. Le
canal (un *named pipe*) est local et réservé à ton compte Windows. Si
BoringWindows est fermé ou ne répond pas en 300 ms, le relais s'efface sans
rien faire : **Claude n'est jamais bloqué**.

## Conversations récentes et consommation

Ouvre l'île et clique sur l'onglet **Claude** (en haut à droite) : il affiche

- une **estimation de ta consommation** sur la fenêtre de 5 h en cours, avec le
  temps avant son renouvellement (et une jauge si tu fixes une limite) ;
- tes **4 dernières conversations** (titre, dossier, ancienneté). Un clic rouvre
  la conversation : un terminal s'ouvre dans son dossier et lance
  `claude --resume <id>`.

C'est lu dans les transcripts que Claude Code garde en local
(`~/.claude/projects`) : aucune connexion, rien n'est envoyé. Les transcripts ne
sont relus qu'à l'ouverture de l'île (au plus toutes les 15 s), et seulement les
fichiers qui ont changé.

**La consommation est une estimation, pas ton quota réel.** Les limites de ton
abonnement sont côté serveur et Claude Code ne les publie pas. BoringWindows
additionne les tokens (entrée, sortie, création de cache) des messages de la
fenêtre en cours, comme l'outil `ccusage` ; pour avoir une jauge, indique la
limite que tu estimes avoir (millions de tokens) dans Réglages › Claude Code. Pour
ton vrai reste, `/usage` dans Claude Code fait foi.

Les conversations **distantes** (sessions SSH) ne sont pas listées : leur dossier
n'existe pas sur ce PC. Elles comptent quand même dans la consommation.

```toml
[modules.claude_activity]
enabled = true
recent = 4                # conversations proposées (0 à 4)
window_hours = 5          # durée de la fenêtre de consommation (1 à 24)
limit_tokens = 0          # limite estimée pour la jauge (0 : pas de jauge)
reset_at = ""             # fin d'une fenêtre connue (voir ci-dessous)
count_cache_reads = false # compter aussi les lectures de cache
```

La reprise ouvre Windows Terminal (`wt`) s'il est installé, sinon une console, et
suppose que la commande `claude` est dans le `PATH`. Windows seulement pour
l'instant.

### Caler le compte à rebours

La fenêtre de 5 h de ton abonnement est **commune à tout le compte** : elle compte
aussi claude.ai et l'application Claude, que BoringWindows ne voit pas. Résultat :
sans calage, le début de fenêtre est déduit des seuls messages de Claude Code et
le « reset dans… » peut être décalé (jusqu'à plusieurs heures).

Pour l'aligner, tape l'heure de reset que Claude affiche (`/usage`, ou claude.ai)
dans Réglages › Claude Code › **Heure de reset de la fenêtre**, au format
`HH:MM` (heure locale, ex. `03:01`). BoringWindows retient cette fin de fenêtre ;
une fois passée, il repart des messages suivants, début exact au premier message.
Le champ ne sert donc qu'après une activité hors Claude Code.

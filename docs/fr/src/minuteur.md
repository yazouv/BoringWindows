# Minuteur

Un minuteur dans l'île : Pomodoro, cuisson, pause café. Il est **désactivé par
défaut** ; active-le dans Réglages › **Minuteur**.

## Utilisation

1. Ouvre l'île (survol ou clic) : une ligne **Minuteur** propose tes durées
   (5, 15 et 25 minutes par défaut).
2. Clique sur une durée pour démarrer. Le compte à rebours s'affiche, avec
   **Pause** / **Reprendre** et **Arrêter**.
3. Île fermée, la pilule affiche « Minuteur · 12 min » quand rien de plus
   important ne demande ton attention.
4. À la fin, l'île s'agrandit (« C'est fini ! ») et joue un son. **OK** ferme
   l'alerte ; sans action, elle s'éteint seule au bout de 20 secondes.

Le minuteur ne consomme rien au repos : il ne se réveille qu'à chaque minute et
à la fin.

## Session de concentration

À côté des durées, **Focus 25 min** lance une session de concentration :

- **ne pas déranger** s'active : les notifications des applications ne
  s'annoncent plus (la lune s'allume dans l'île) ;
- la pilule affiche « Concentration · 18 min », avec un fin trait de
  progression le long de son bord bas ;
- la mascotte plisse les yeux et ne bouge plus (elle ne danse pas sur la
  musique), puis se réjouit à la fin.

À la fin de la session, ou si tu l'arrêtes, « ne pas déranger » revient à son
état d'avant (s'il était déjà actif, il le reste). Claude Code continue de te
prévenir quand il t'attend. La durée se règle dans Réglages › **Minuteur**
(0 : pas de bouton).

## Réglages

```toml
[modules.timer]
enabled = true
presets = [5, 15, 25]   # 1 à 5 durées, en minutes (1 à 600)
focus_minutes = 25      # session de concentration (0 : pas de bouton)
sound = true            # son à la fin
done_secs = 20          # durée de l'alerte « terminé »
```

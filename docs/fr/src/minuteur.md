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

## Réglages

```toml
[modules.timer]
enabled = true
presets = [5, 15, 25]   # 1 à 5 durées, en minutes (1 à 600)
sound = true            # son à la fin
done_secs = 20          # durée de l'alerte « terminé »
```

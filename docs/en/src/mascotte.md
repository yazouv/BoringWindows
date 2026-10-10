# Mascot

A little blob with eyes lives in the pill. Its mood follows what is going on:

| When | The mascot |
|---|---|
| Nothing special | in the middle of the pill, calm |
| In the morning (6 am to 11 am) | smiling eyes |
| In the evening (9 pm to 6 am) | half-closed eyes, sleepy |
| Nobody at the keyboard for 10 min | asleep ("z"), wakes up as soon as you touch the keyboard or mouse |
| Claude Code is working | busy, eyes going back and forth |
| Claude is waiting for you (permission, question) | hops around, all colored, with a "!" |
| Claude is done | smiles and jumps for joy |
| Music is playing | dances to the bass, next to the artwork, and the pill breathes in rhythm |

When Claude has the pill, the mascot takes the place of the status dot; when
music has it, the mascot moves to the right.

In the **open island**, it shows up big on the left of the Home tab, and
arrives with a bounce. Big, it has more details: a sprout on its head that
sways, cheeks, shiny eyes, arms (raised for joy, up in the air when Claude is
waiting for you, typing while it works), feet, a mouth that follows its mood,
music notes when it dances and "z"s floating up while it sleeps. It stays
animated as long as the island is open.

## Seasons

It dresses up, and the open island gets decorated:

- **Halloween** (all of October, until November 1): witch hat and little
  pumpkins;
- **Christmas** (December 1 to 26): Santa hat and snowflakes;
- **New Year** (December 30 to January 2): party hat and confetti.

## Settings

Settings › **Mascot**, or in `config.toml`:

```toml
[modules.mascot]
enabled = true
always_animated = false   # always animated (breathes, blinks)
music = true              # dance to the music
seasonal = true           # costumes and decorations
sleep_after_minutes = 10  # before it falls asleep (1 to 240)
```

By default, the mascot **only moves when something happens** (Claude, music):
at rest, it stands still and the island uses nothing. `always_animated = true`
makes it breathe and blink all the time, at the cost of a little CPU.

Dancing uses the audio capture of the [visualizer](configuration.md#modulesvisualizer),
only while music plays.

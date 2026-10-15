# Ask Claude or Gemini

Type a question, the answer shows **in the island**. Handy for a definition, a
forgotten command or a translation, without opening a browser. Off by default:
Settings › **System** › *Ask Claude or Gemini*, or `enabled = true` in
`[modules.ask]`.

## Asking

The island never takes the keyboard (that is what lets it not steal the focus of
what you are typing). The **Ctrl+Alt+Space** shortcut opens a small box **under
the island** instead:

- **Enter** sends the question and closes the box;
- **Esc** cancels;
- the list under the field **picks the model**: it is remembered for next time.

While the model answers, the pill shows "Asking Claude…". When the answer
arrives, the island opens by itself on a **card** covering the tabs:

- the answer scrolls with the wheel if it is long;
- **Ask again** reopens the box with the question, to edit it;
- **×** closes the card. Otherwise it stays, even if the island closes, until
  the next question.

During a call or screen sharing ([presentation mode](presentation.md)), the
island does not open by itself: the card waits for you to open it.

## The model and the key

The model also picks the provider: an id starting with `claude` goes to
Anthropic, `gemini` to Google. Offered:

| Provider | Models |
|---|---|
| Claude | `claude-haiku-5-5` (fast), `claude-sonnet-5-5`, `claude-opus-5-5` |
| Gemini | `gemini-2.5-flash-lite`, `gemini-2.5-flash`, `gemini-2.5-pro` |

Any other id goes in `modules.ask.model` by hand.

You need the **API key** of the provider you use (one is enough):
[console.anthropic.com](https://console.anthropic.com) for Claude,
[aistudio.google.com](https://aistudio.google.com) for Gemini. Paste it in the
settings and press Enter: it goes to the **Windows Credential Manager**, and
`config.toml` only keeps a reference (`secret:…`). These APIs are **pay per
use** (Gemini has a limited free tier): a short question costs a fraction of a
cent.

## What goes over the network

Each question is sent **alone**, with no history, to the provider you picked,
with your key in a header (never in the URL). Nothing else leaves your PC, and
the answer is not saved: it disappears when the app quits.

By default the model answers briefly, in plain text. `system` changes that
instruction, `max_tokens` the maximum length.

## When it does not work

The card says why:

- "key was refused": wrong or revoked key, or no credit;
- "unknown model": id misspelled or retired by the provider;
- "rate limit or quota reached": wait a little, or switch model;
- "The answer was cut off": raise `max_tokens`.

## Settings

```toml
[modules.ask]
enabled = false
model = "claude-sonnet-5-5"
claude_key = ""     # secret:<id> (filled by the settings)
gemini_key = ""
max_tokens = 1024   # 64 to 8192
system = ""         # empty: short plain-text answers

[hotkeys]
ask = "Ctrl+Alt+Space"
```

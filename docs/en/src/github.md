# GitHub

A small **pull request** icon shows next to the time, in the open island. Its
color sums up the **CI of your open pull requests**:

| Color | Meaning |
|---|---|
| gray | no CI, or no open pull request |
| accent | a CI is running |
| green | all CI passed |
| red | at least one CI is failing |

A **dot** on the icon means someone is waiting for **your review**.

**Hover the icon**: the date gives way to the details, for example "2 open
PRs · CI failing: api#31 · 1 review requested". **Click it** to open the page
that needs you: the failing pull request, otherwise the first requested
review, otherwise your first pull request.

## Announcements

The pill tells you when:

- the **CI of one of your pull requests fails** ("CI failed · api#31") or
  **passes** after running ("CI passed · api#31");
- you are **asked for a review** ("Review requested · api#7").

With several at once, the most important shows, with their count ("CI failed
· api#31 (+1)"). A failure comes before a review, which comes before a
success. The first read is the reference, it announces nothing.

## The token

The island needs a GitHub access token, and looks for it in this order:

1. **the saved token**: Settings › **System** › *GitHub* › *Access token*.
   Paste a [personal token](https://github.com/settings/tokens) (`repo` and
   `read:org` scopes) and press Enter: it goes to the Windows Credential
   Manager, and `config.toml` only keeps a reference (`secret:…`);
2. **the GitHub CLI session**: if you are already logged in with
   `gh auth login`, the island asks it for its token (`gh auth token`).
   Nothing to type. Turn off *Use the GitHub CLI session* if you don't want
   that.

A missing or refused token is announced once in the pill ("GitHub: token
missing or refused"), then the island retries every 15 minutes.

## Refresh rate

GitHub has no event to listen to without a public server: the island **polls**
GitHub, **one single request** per interval (2 minutes by default) that brings
back your pull requests, their CI and the review requests. It is one of the
rare exceptions to the "nothing is polled" rule, like the weather. After a
network failure (PC waking up…), it retries after 2 minutes.

## Settings

```toml
[modules.github]
enabled = false              # off by default
token = ""                   # "secret:…" (settings) or token; empty: gh session
use_gh_cli = true            # without a token, ask `gh auth token` for one
refresh_secs = 120           # 30 to 3600
announce_ci = true
announce_reviews = true
show_secs = 6                # how long an announcement stays, 2 to 30
```

> Don't put your token in plain text in `config.toml` if you share that file:
> use the settings, which store it in the OS vault.

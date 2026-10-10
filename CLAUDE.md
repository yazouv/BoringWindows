# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

BoringWindows is a "Dynamic Island" for Windows, written in Rust with Slint. It shows Claude Code session status (via hooks), the music currently playing, and the next meetings from ICS calendars. Windows is the target platform. macOS and Linux build and run as a floating window, which makes them good enough for development and for screenshots under Xvfb. `PLAN.md` is the roadmap, in French, with phase checkboxes; keep it up to date when you finish an item.

Code comments, commit bodies, logs and docs are in **French**. UI strings use English as the source language, with French translations (see i18n below).

## Commands

```sh
cargo run -p bw-app                      # dev run (logs to console; RUST_LOG=debug for more)
cargo run -p bw-app -- --settings        # open the settings window at startup
cargo run -p bw-app -- doctor            # Claude Code hook diagnostic (writes doctor.txt)
cargo build --release                    # target/release/boringwindows(.exe)

cargo fmt --all --check
cargo test --workspace --locked
cargo test -p bw-calendar agenda::tests::back_to_back_classes   # single test
cargo test -p bw-update -- --ignored     # tests that need the network

# Clippy for all three platforms. This works from Linux: it type-checks without linking.
# Run it before pushing, because PR CI only runs on Linux.
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --workspace --all-targets --locked --target x86_64-pc-windows-msvc -- -D warnings
cargo clippy --workspace --all-targets --locked --target aarch64-apple-darwin -- -D warnings

# Docs (mdBook, FR + EN)
cd docs && mdbook build fr && mdbook build en    # output: docs/book/
```

On Linux, building needs `libfontconfig1-dev libxkbcommon-dev`.

## CI/CD and versioning

GitHub-hosted runners only (free for a public repo). Never add a self-hosted runner: a fork PR can edit a workflow to run anything on it.

- **`main` is protected** by the "Protect Main" ruleset: changes go through a PR, and the `ci.yml` jobs (`check`, `test (windows)`, `test (macos)`, `deny`, `commits`) are required checks. Renaming one of these jobs means updating the ruleset.
- **`ci.yml`**: on PRs, pushes to `main`, every Monday (new advisories, new clippy lints) and on dispatch.
  - `check` (Linux): fmt, clippy for Linux, Windows and macOS targets, tests.
  - `test (windows)`, `test (macos)`: `cargo test` on the real systems.
  - `deny`: `cargo deny check advisories sources` (config in `deny.toml`).
  - `commits`: every non-merge commit of a PR must follow Conventional Commits.
- **Actions are pinned by commit SHA** with a `# vX.Y.Z` comment; Dependabot (`.github/dependabot.yml`) updates them and the Cargo dependencies weekly, with a 7-day cooldown. `workflows.yml` runs actionlint and zizmor when `.github/` changes. Each job requests only the permissions it uses, and checkouts set `persist-credentials: false`.
- **`release.yml`**: release-please runs on every push to `main`. It keeps a "chore: release x.y.z" PR open; the `lockfile` job aligns `Cargo.lock` on it, then dispatches `ci.yml` and `release-check.yml` on that branch (a PR pushed with the `GITHUB_TOKEN` triggers no workflow, so its required checks would never run). Merging that PR tags `vX.Y.Z` and calls `build.yml`.
- **`build.yml`** (reusable): builds the Windows, macOS and Linux binaries with `.sha256` files and the Inno Setup installer (`installer/boringwindows.iss`). With a `tag`, it adds build provenance attestations, attaches everything to the release and updates the `Yazouv.BoringWindows` winget package (skipped when the `WINGET_TOKEN` secret is absent). Without one (`release-check.yml`: PRs touching the release build or dependencies, the release PR, manual runs), it is a dry run and the files become run artifacts. The updater depends on the asset names in that workflow and in `bw_update::asset_name()`; keep them in sync. macOS builds both architectures in a single job.
- **Commit messages must follow Conventional Commits** (`feat:`, `fix:`, `docs:`, `chore:`…). release-please derives the version and changelog from them.
- Never bump versions by hand. release-please owns `[workspace.package] version` in `Cargo.toml`, plus `version.txt`, `CHANGELOG.md` and the manifest.
- **`docs.yml`**: builds the mdBook on PRs touching `docs/`, and publishes it to GitHub Pages when `docs/` changes on `main`.

## Architecture

The Cargo workspace (edition 2024) has one crate per concern. `bw-app` is the only binary and the only UI.

- **`bw-core`**:
  - the `Module` trait and `ModuleHost`, which runs every module on one tokio current-thread runtime on a background thread;
  - the `Arbiter`, which decides which module's `Attention` (None/Low/High/Urgent) owns the compact pill, ordered by `layout.compact`.

  Modules publish `ModuleEvent`s. These are either an `Attention` claim or a `State(Arc<dyn Any + Send + Sync>)` that the controller downcasts (`Snapshot`, `MediaSnapshot`, `CalendarSnapshot`). UI actions come back through `Module::on_action(&str)`.
- **Modules**:
  - **`bw-claude`**: Claude Code integration.
    - `boringwindows hook` is a relay that Claude Code runs on each hook event. It forwards a JSON summary over a named pipe (Unix socket elsewhere) and **must print nothing else on stdout**. It exits within about 300 ms if the app is unreachable.
    - The relay can return "allow" to Claude Code, so the channel is authenticated. On Windows, the relay checks that the pipe's server process runs under the same account (`server_identity` in `hook.rs`). On Unix, the socket and its folder must be ours and not writable by others (`ipc::check_private`, on both the client and the server side). Don't weaken these checks.
    - `install.rs` edits `~/.claude/settings.json`, adding only our entries, with a backup. It also auto-upgrades outdated hooks.
    - `tracker.rs` turns hook events into per-session state.
    - `doctor.rs` runs the end-to-end diagnostic.
  - **`bw-media`**: music. It uses Windows GSMTC on a dedicated thread.
  - **`bw-calendar`**: calendars.
    - `ics.rs` is a homegrown ICS parser (RRULE via `rrule`, Windows time-zone names mapped in `tz.rs`).
    - `agenda.rs` is pure logic: time is passed in, and it returns what to display plus the next time the display changes.
    - `probe.rs` backs the settings window's "Test" button.
- **`bw-config`**: config.
  - `Config` is deserialized with `deny_unknown_fields` and validated.
  - **`Config::parse(text, config_dir)` resolves the theme.** `theme.name` selects a built-in preset (`src/themes/*.toml`) or `themes/<name>.toml` next to config.toml. Keys the user writes in `[theme]` override the preset. `auto` resolves to `light` or `default` from a process-global flag (`set_system_light`) that the app sets from Windows before loading, and again when Windows changes mode (then it reloads the config).
  - `DEFAULT_TOML` (`default.toml`) must stay equivalent to `Config::default()`, and a test enforces it.
  - `ConfigEditor` (toml_edit) writes changes while keeping comments, and validates before an atomic save.
  - `watch.rs` hot-reloads on changes to config.toml or theme files. It notifies only when the resulting config changes, because inotify also reports reads.
- **`bw-notify`**: app notifications (Discord, Slack…). It reads the Windows notification center through `UserNotificationListener`. Unpackaged apps may read the list but not subscribe to its change event, so `listener.rs` re-reads it every 2 s: the one exception to the no-polling rule. `inbox.rs` is pure logic (what is new, unread count, brand colors). `icon.rs` reads app icons from `shell:AppsFolder\<AUMID>`. The read API doesn't expose a toast's launch link, so `activate.rs` replays a click: it opens the notification center (Win+N), finds the entry by its text with UI Automation and invokes it.
- **More modules**: `bw-timer` (timer), `bw-volume` (system volume changes, plus built-in screen brightness via WMI), `bw-viz` (audio visualizer via WASAPI loopback, active only while the island is open and music plays), `bw-plugins` (third-party WASM plugins on wasmi: only 4 host functions, no file or network access, fuel and memory caps — keep it that way).
- **`bw-power`**: two modules, `battery` (WinRT `PowerManager` events) and `bluetooth` (two `DeviceWatcher`s, paired endpoints for the connection state and PnP `BTH*` nodes for `DEVPKEY_Bluetooth_Battery`, joined by `ContainerId`). `battery_notice` and `bluetooth_notice` are pure logic. Each Windows thread blocks on a `Wake` channel, and `Drop` sends `Wake::Stop` because the event handlers also hold senders. Each announcement publishes a `bw_power::Gauge` state just before its attention claim; the controller shows the gauge for the winning module.
- **`bw-presence`** (module `presentation`): "live" detection (call or screen sharing) from the Windows privacy registry `CapabilityAccessManager\ConsentStore` (`microphone` for apps in `call_apps`, `graphicsCapture*` for any app; `LastUsedTimeStop == 0` means in progress). Crashed apps leave stale entries, so an entry only counts if its executable is running and started before the usage. The thread sleeps on `RegNotifyChangeKeyValue` events plus the live process handle. `live_app` is pure logic. The controller mutes announcements while live and shows a red dot; `hide_from_capture` applies `WDA_EXCLUDEFROMCAPTURE` to the island and the blur backdrop.
- **`bw-system`** (module `system`): CPU and memory via `sysinfo`, shown next to the time in the open island. It measures only while the island is open (the controller sends `open`/`close` actions), unless an alert threshold is set (then every 5 s). `watch.rs` is the pure alert logic (sustained load, hysteresis). The alert names the top process.
- **`bw-weather`**: current weather from open-meteo (no key): geocodes the city once, then refreshes every `refresh_minutes`. Requests send `Accept-Encoding: identity`, because open-meteo's default `deflate` fails in WinINet (`ERROR_INTERNET_DECODING_FAILED`).
- **`bw-secrets`**: private ICS links and CalDAV passwords live in the OS credential manager. `config.toml` only holds `secret:<id>` references.
- **`bw-i18n`**: the global language plus the `tr!("English", "Français", args…)` macro, used for every user-facing string on the Rust side. Logs and the hook journal stay in French.
- **`bw-net`**: blocking HTTP requests. Windows uses the WinRT HttpClient, so system proxy and certificates apply. Elsewhere it shells out to `curl`, with the URL, headers and body passed **through stdin (`-K -`), never as arguments**, because other local accounts can read the command line. Redirects never resend `Authorization` to another host.
- **`bw-update`**: self-update from the GitHub releases of `yazouv/BoringWindows`:
  - it checks the latest release, verifies the SHA-256, and replaces the running executable (on Windows, the old one is renamed to `.old`);
  - `BORINGWINDOWS_GITHUB_TOKEN` is only needed for a private fork;
  - builds run from a cargo `target/` dir never self-update.
- **`bw-app`**:
  - `controller.rs` is the single-threaded hub. It holds `Rc<Controller>` in a thread-local and owns the Slint windows, arbiter, module host, tray, config watcher and timers.
  - Other threads (module host, notify watcher, tray, worker threads) reach it only through `post(|c| …)`, which uses `slint::invoke_from_event_loop`.
  - Submodules: `controller/settings.rs` (settings window: fills it from config, each change goes through `ConfigEditor` with a 300 ms debounce) and `controller/update.rs` (update checks, restart via `--restarted`).
  - `hotkeys.rs`: global shortcuts (`[hotkeys]` in config.toml) through `global-hotkey`, created on the UI thread. Syntax errors and shortcuts taken by another app show in the island like config errors; the settings window rejects them before saving.
  - `platform/macos.rs`: the island's winit `NSWindow` gets its class swapped to a runtime `NSPanel` subclass (non-activating, never key) and sits above the menu bar. Click-through: `ignoresMouseEvents` flips on mouse moves seen by global/local `NSEvent` monitors, and `PlatformEvent::PointerLeft` makes the controller dispatch `PointerExited` to Slint. An always-active `NSTrackingArea` gives hover without being key. Music on macOS is `bw-media/src/macos.rs`: Spotify and Music distributed notifications wake an AppleScript query (`applescript.rs` is the pure part).
  - `platform/`: `win32.rs` has all Win32 code behind `cfg(windows)`: non-activating tool window, `SetWindowRgn` click-through region, full-screen detection, single-instance mutex, autostart, file picker, Windows light/dark mode and accent (`UISettings`, with its change event).
  - **Blur (`theme.blur`)**: the undocumented `SetWindowCompositionAttribute` blur covers the whole window and ignores `SetWindowRgn`. A composition visual on the island's own window hides its OpenGL output. The working design is a separate content-less `Backdrop` window just below the island (the island is *owned* by it, and it is re-inserted right under it) holding a `HostBackdropBrush` clipped by a rounded-rectangle geometry. While blur is on, the controller reads the pill's animated geometry (`pill-*` out properties) every 16 ms during transitions and reshapes the region and the clip. `tray.rs` is the tray icon (Windows and macOS). `fallback.rs` holds stubs for other platforms.
  - `geometry.rs` computes pill sizes and placement.
  - `mascot.rs` (config `[modules.mascot]`, pure mood and season logic) and `gestures.rs` (config `[modules.gestures]`, pure wheel accumulator; Slint gives 60 px per wheel notch, `dy` > 0 is up, wheel tilted right gives `dx` < 0). The mascot only animates (a Slint `Timer` inside the `Mascot` component) for lively moods unless `always_animated`; keep it that way so the island stays at 0 % CPU at rest. "Away" uses a single-shot timer aimed at `last input + sleep_after_minutes`, then Raw Input (`RIDEV_INPUTSINK` on the helper window, removed on the first `WM_INPUT`) to wake up. Dancing reuses the `bw-viz` capture, started while the island is closed and music plays.
  - The island window is subclassed (`island_proc`): `WM_NCACTIVATE` goes to `DefWindowProc` with `lParam = -1` and `WM_NCPAINT` is swallowed, otherwise Windows paints a classic title bar over the island on the first click. Caption styles are stripped too (`strip_caption`).

### UI (Slint)

- `ui/main.slint` re-exports `island.slint` (the island) and `settings.slint` (the settings window, std-widgets "fluent" style). `build.rs` compiles them.
- Only the island gets the special window attributes (transparent, no focus, topmost). That is done by wrapping its creation in `platform::creating_island(...)`.
- The native winit window exists only once the event loop runs. Use `window().winit_window().await` inside `slint::spawn_local`, as `Controller::start` does.
- Users can replace the island's views with their own `.slint` files (`layout.view`), loaded at runtime by `slint-interpreter`.
- All island colors derive from the `bg`, `fg`, `accent` and `border` properties set from the theme. Don't hardcode colors in `island.slint`. The only exceptions are status colors (`ok-color`, `alert-color`) and the seasonal decorations (pumpkin orange, Santa red…).

### i18n

- In `.slint` files, text is `@tr("English text")`. The French translation goes in `crates/bw-app/lang/fr/LC_MESSAGES/bw-app.po`, which is bundled into the binary with no per-component context.
- A test (`src/translations.rs`) fails if an `@tr` string has no French entry. Add the `.po` entry whenever you add or change a UI string.
- Changing the language at runtime calls `slint::select_bundled_translation` (`""` means English) and re-renders the Rust-side texts (tray menu, modules, settings window).
- Tests that assert French strings call `bw_i18n::set(Lang::Fr)` first. The language is global to the process; `lang()` initializes it with `compare_exchange` so a concurrent `set` is never overwritten.

## Docs

`docs/fr` (root of the site) and `docs/en` (`/en/`) are two mdBook books. They must have **the same file names and page lists**, and the docs workflow checks this. Shared `lang.js` and `lang.css` must stay identical in both. When a feature changes user-visible behavior, update both languages. The English docs use the English UI labels.

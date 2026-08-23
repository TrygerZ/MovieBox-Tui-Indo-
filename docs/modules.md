# Modules

The crate (`moviebox_tui`) is split into top-level modules and, inside `tui`, an `app`
directory that holds the application object. Below is the full tree with each module's
responsibility.

```
src/
  main.rs            Entry point. Logging init, panic hook, raw mode, alternate
                     screen, TerminalGuard, App::new + App::run.
  lib.rs             Crate root: declares pub mod cache/config/download/history/
                     logging/models/player/providers/service/tui/updater.

  cache.rs           Disk cache: provider-namespaced directories, TTL expiry,
                     atomic temp-file writes, payload validation, background purge.

  config.rs          Config struct: load/save config.json (mode persistence,
                     mode toggles, provider, theme, auto-update, default player,
                     download directory, bdix flag).

  download.rs        Download engine (pure, async): resume via .part files,
                     HTTP ranges, optional multi-segment download, retries, cancel
                     via an AtomicBool, Windows-safe file stems.

  history.rs         Watch history: read/write history.json, dedupe exact
                     provider/subject/episode entries, cap 100.

  logging.rs         File logging: flexi_logger, rotation (5MB, keep 3),
                     MOVIEBOX_LOG level, URL/path sanitization for sharing.

  models.rs          Pure domain data models: SearchResult, BrowseMetric,
                     BrowsePreset, BrowseMetrics, SubjectStreamPool, Notification.

  player.rs          Player detection (OnceLock) and command construction for
                     mpv / VLC / IINA / Android intent, subtitle args, headers,
                     terminal-sized window.
  player/
    tracker.rs       Injected Lua tracker script (`moviebox_tracker.lua`) and
                     periodic 5-second playback state auto-saving.

  providers/
    mod.rs           Module declarations.
    models.rs        Shared types: ProviderKind, CatalogItem, MediaDetails,
                     Release, PlaybackSource, RequestContext, SourceMirror.
    moviebox/        Primary provider.
      client.rs      Async reqwest client with signed requests (anti-bot).
      crypto.rs      Request signing: HMAC-MD5 signature, client token,
                     spoofed device identity (by design of the scraper).
      title.rs       clean_moviebox_title: strip quality/site suffix noise.
    fourkhdhub/      Secondary provider.
      client.rs      Search/details/stream resolution + preflight validation.
      hubcloud.rs    Mirror resolver: fetch drive pages, extract playable links.
      parser.rs      HTML parsing into typed CatalogItem/MediaDetails/Release,
                     plus moviebox-JSON adapters.
    bdix/
      circleftp/     BDIX CircleFTP provider (client + parser).
      dhakaflix/     BDIX DhakaFlix provider (client + parser).
    addons/          Community HTTP addons provider (client, aggregator, adapter, models).
    tv/              Live TV / IPTV provider (models + M3U parser).

  service.rs         MovieBoxService: unified multi-provider headless client &
                     engine (suggest, search, details, homepage, resolutions,
                     captions, subtitle download, path resolution).

  updater.rs         GitHub release update check.

  tui/
    action.rs        The Action enum: every UI event/message (input, network
                     results, downloads, playback, tv, system).
    commands.rs      Slash command registry (SlashCommand enum, argument parsing,
                     availability filters, dynamic suggestions, descriptions).
    state.rs         AppState: all UI state, LRU image/preview caches, and the
                     PlayerKind enum + label()/parse(), tv manager row model.
    event.rs         EventHandler: crossterm event stream + tick interval,
                     forwards to the action channel.
    overlay.rs       Popups: notifications, pickers, confirmation, modal centering.
    screens/
      home.rs        Home/startup + search list rendering (streaming and TV).
      details.rs     Details screen rendering.
      help.rs        Keybinding help (mode-aware).
    terminal.rs      Terminal capability probes (basic UI, image querying).
    theme.rs         Color themes + terminal color detection.
    text.rs          Grapheme-safe width/truncation helpers.

  tui/app/           The application object (App) and all behavior.
    mod.rs           App struct, App::new, and small helpers.
    run.rs           App::run (event loop), App::draw (rendering), and
                     handle_action dispatcher (thin routing table over action groups).
    network.rs       fetch_poster_bytes, decode_poster, provider_search,
                     provider_details.
    search.rs        Search-mode command routing, search state setup, provider
                     search dispatch, poster prefetch helpers.
    requests.rs      handle_requests: suggest/history/homepage/details/preview/
                     episode-streams/poster actions.
    playback.rs      handle_playback: play/subtitle/picker/launch/crash actions
                     + launch_player.
    download.rs      handle_download: download orchestration + start_resilient_download.
    navigation.rs    handle_navigation + provider/nav helpers.
    keyboard.rs      handle_key: raw key-event handling.
    mouse.rs         handle_mouse: mouse click routing and hitboxes across screens,
                     popups, tabs, buttons, and dialogs.
    system.rs        handle_system: tick/quit/focus/resize/help/refresh/cache/
                     theme/status/updates.
    tv.rs            handle_tv: playlist manager + TV actions.
    addons.rs        handle_addons: addon manager + HTTP addon actions.
```

See [architecture.md](architecture.md) for the event loop, async model and data flow,
and the per-topic docs for details.

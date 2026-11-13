# Players

MovieBox-TUI delegates playback to external media players (`mpv`, `IINA`, `VLC`, or Android intent players).

## Detection Order

Players are detected in priority order and cached across runs:

- **macOS**: `IINA` → `MPV` → `VLC`
  - Searches `/Applications`, `~/Applications`, Homebrew, MacPorts, Nix, and native CLI tools.
- **Linux**: `MPV` → `VLC`
  - Searches `$PATH`, `~/.local/bin`, Flatpak exports (`io.mpv.Mpv`, `org.videolan.VLC`), and Snap.
- **Windows**: `MPV` → `VLC`
  - Searches `%LOCALAPPDATA%`, `Program Files`, WinGet packages, Scoop shims, Chocolatey, and Windows Registry `App Paths`.
- **Android / Termux**: `Android Intent` (headless CLI) or `MPV` → `VLC` → `Android Intent` (graphical X11/Wayland desktop)
  - In headless terminal environments without an active display server, dispatches directly to external Android media apps via `termux-open` or `am start`.
  - In graphical environments with an active display server (`$DISPLAY` or `$WAYLAND_DISPLAY`, such as Xfce in udroid/PRoot or Termux:X11), native desktop players (`MPV`/`VLC`) take priority over Android intents. Unconfigured sessions resolve players dynamically, and desktop displays bypass legacy saved Android defaults unless overridden by `MOVIEBOX_PLAYER`.

You can set a default player via `/settings` (Media Player), or override it with the `MOVIEBOX_PLAYER` environment variable.

## Player Invocations

### mpv
```bash
mpv --autofit=WxH --geometry=50%:50% --hwdec=auto-safe --stream-buffer-size=4M --idle=no --keep-open=no [OPTIONS] <url>
```
- **Tracking**: Injects `moviebox_tracker.lua` to record playback progress, total duration, and chosen stream filename, then restores that stream mirror when re-opening multi-stream titles.
- **Hardware Decoding & Direct Streams**: Enables `--hwdec=auto-safe` for hardware-accelerated 4K 10-bit HEVC HDR decoding and passes `--ytdl=no` on non-DASH direct streams (`.mkv`, `.mp4`, `.m3u8`) for direct `ffmpeg` demuxer startup.
- **Headers**: Passes custom stream headers (`User-Agent`, `Referer`) via `--http-header-fields`.
- **DASH Manifests & Loopback Acceleration**: Routes DASH `.mpd` streams through the local `StreamRelay` proxy (`127.0.0.1:<port>`), which caches rewritten manifests in RAM, warm-prefetches initialization and opening fragments on sidecar startup, fetches `.m4s` segments via parallel `95 KB` HTTP `Range` sub-requests, and prefetches the next 3 video segments (`N+1..N+3`) and matching audio segments (`chunk-stream3-N..N+1`).
- **Subtitles**: Pre-downloads remote subtitles to temporary storage in parallel with sidecar startup and passes `--sub-file=<path>`, with fallback to the remote URL if local download fails.

### VLC
```bash
vlc --width=W --height=H --no-one-instance --network-caching=3000 --file-caching=3000 --http-reconnect --adaptive-logic=predictive [OPTIONS] <url>
```
- **Headers**: Mapped to `--http-user-agent` and `--http-referrer`.
- **DASH & CloudFront Streams**: Cookie-authenticated and DASH `.mpd` streams route through the local `StreamRelay` proxy sidecar (`127.0.0.1:<port>`) with HTTP/1.1 persistent connections (`Keep-Alive`), parallel `95 KB` `Range`-chunked `.m4s` segment fetching, lookahead segment prefetching (`N+1..N+3`), and resolution representation capping.
- **Subtitles**: Remote subtitles are pre-downloaded to temporary storage and passed via `--sub-file=<path>` using native Windows backslash separators (`C:\...`) on Windows and `--no-one-instance` on Windows/Linux so existing VLC instances do not strip CLI flags over IPC.

### IINA (macOS)
```bash
iina-cli --keep-running --no-stdin --mpv-autofit=WxH [OPTIONS] <url>
```
- Forwards `--mpv-*` arguments (including `--mpv-sub-files=<path>`) to internal mpv core.
- Falls back to `open -a IINA <url>` if CLI tools are unlinked.

### Android / Termux
```bash
termux-open --chooser --content-type video/* <url>
```
- Opens the system app chooser, delegating playback to installed Android players (VLC, MX Player, Just Player, mpv-android).
- Subtitles are saved to shared storage (`~/storage/downloads/moviebox_subs`) and passed via intent extras, or served over the local `StreamRelay` loopback proxy (`http://127.0.0.1:<port>/sub/...`) when shared storage is unavailable.

## Watch History & Resume

- **Position Resumption**: In-progress items launch with `--start=<seconds>` (`mpv`/`IINA`) or `--start-time=<seconds>` (`VLC`).
- **Completion Detection**: Reaching 90% or stream EOF marks media as completed.
- **State Reconciliation**: `mpv` and `IINA` sync playback progress via background IPC state files so pauses or seeks do not cause wall-clock drift.

## Spawning & Process Safety

- Players and the `StreamRelay` sidecar launch in detached OS sessions (`libc::setsid()` with `SIGHUP` ignored on Unix/macOS/Linux/Android, and `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP` on Windows) so closing the terminal or quitting `moviebox-tui` does not stop active video playback.
- Player `stderr` writes to a temporary log file so `moviebox-tui` can exit without triggering `SIGPIPE` crashes while still capturing crash diagnostics when the TUI stays open.
- Clean exits (VLC exit code `1` with empty stderr, or Unix `SIGTERM`) count as normal exits and update watch history without false crash popups.

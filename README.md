<div align="center">

# MovieBox-TUI

**Stream movies, shows, anime, and live TV from your terminal.** <br>
Fast and clean. No configuration, no torrents, and no debrid required.

[![Crates.io](https://img.shields.io/crates/v/moviebox-tui.svg?logo=rust)](https://crates.io/crates/moviebox-tui)
[![Downloads](https://img.shields.io/crates/d/moviebox-tui.svg)](https://crates.io/crates/moviebox-tui)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg?logo=rust)](#requirements)

<br>

<img src="https://raw.githubusercontent.com/mesamirh/MovieBox-Tui/main/assets/screenshots/01-home-blocky.jpg" alt="MovieBox-TUI Home" width="85%">

**[See what's new in v0.1.7 on YouTube](https://youtu.be/5M2_mjH5r5Y)**

<sub>Found a bug? [Open an issue](https://github.com/mesamirh/MovieBox-Tui/issues) so I can fix it for everyone!</sub>

</div>


## Screenshots

<details>
<summary><b>Movie & Series Details</b></summary><br>
<p align="center">
  <img src="https://raw.githubusercontent.com/mesamirh/MovieBox-Tui/main/assets/screenshots/07-movie-details.jpg" alt="Movie Details" width="49%">
  <img src="https://raw.githubusercontent.com/mesamirh/MovieBox-Tui/main/assets/screenshots/08-series-details.jpg" alt="Series Details" width="49%">
</p>
</details>

<details>
<summary><b>Search & Downloads</b></summary><br>
<p align="center">
  <img src="https://raw.githubusercontent.com/mesamirh/MovieBox-Tui/main/assets/screenshots/06-search-results.jpg" alt="Search Results" width="49%">
  <img src="https://raw.githubusercontent.com/mesamirh/MovieBox-Tui/main/assets/screenshots/12-download-progress.jpg" alt="Download Progress" width="49%">
</p>
</details>

<details>
<summary><b>Playback & Subtitles</b></summary><br>
<p align="center">
  <img src="https://raw.githubusercontent.com/mesamirh/MovieBox-Tui/main/assets/screenshots/11-player-picker.jpg" alt="Media Player Selection" width="49%">
  <img src="https://raw.githubusercontent.com/mesamirh/MovieBox-Tui/main/assets/screenshots/10-playback-subtitles.jpg" alt="Subtitle Language Selection" width="49%">
</p>
</details>

<details>
<summary><b>Live TV Experience</b></summary><br>
<p align="center">
  <img src="https://raw.githubusercontent.com/mesamirh/MovieBox-Tui/main/assets/screenshots/09-live-tv-list.jpg" alt="Live TV Channels" width="49%">
  <img src="https://raw.githubusercontent.com/mesamirh/MovieBox-Tui/main/assets/screenshots/05-tv-help.jpg" alt="Live TV Configuration" width="49%">
</p>
</details>

<details>
<summary><b>Home Themes</b></summary><br>
<p align="center">
  <img src="https://raw.githubusercontent.com/mesamirh/MovieBox-Tui/main/assets/screenshots/03-home-3d.jpg" alt="3D Block Theme" width="49%">
  <img src="https://raw.githubusercontent.com/mesamirh/MovieBox-Tui/main/assets/screenshots/02-home-ascii.jpg" alt="Minimal ASCII Theme" width="49%">
</p>
</details>

<details>
<summary><b>Help & Configuration</b></summary><br>
<p align="center">
  <img src="https://raw.githubusercontent.com/mesamirh/MovieBox-Tui/main/assets/screenshots/04-global-help.jpg" alt="Global Help Menu" width="85%">
</p>
</details>


## Features

### Streaming & Playback
- **Instant Search & Catalogs:** Type to search instantly, or browse trending movies, shows, and anime using slash commands (e.g., `/movies`, `/anime`).
- **Seamless Local Playback:** Resolves 4K/1080p streams and opens them instantly in your preferred local video player (`mpv`, `IINA`, or `VLC`).
- **Integrated Subtitles:** Automatically fetches available subtitles — built-in source, then SubDL, then OpenSubtitles — and lets you select your preferred language before playback.
- **Live IPTV:** Press `Ctrl+T` to toggle Live TV mode and stream thousands of live television channels globally.

### Advanced Downloading
- **Batch Season Downloader:** Queue up entire television seasons for concurrent downloading with a single keystroke.
- **Resilient Downloads:** Built-in support for download resumes. If a download is interrupted or fails, it picks up right where it left off.
- **Auto-Subtitle Fetching:** Automatically downloads the best-matching `.srt` subtitle files alongside your video files.

### Terminal Experience
- **Native Image Rendering:** Enjoy high-resolution movie posters rendered directly in supported terminals.
- **Dynamic Theming:** Switch between beautiful 3D block layouts and clean ASCII themes to fit your aesthetic.
- **Power-User Slash Commands:** Use terminal-style commands to update the app (`/update`), switch categories, or customize your Live TV playlists (`/config`).
- **Smart Auto-Cleanup:** A silent background worker intelligently manages and deletes old cache files to protect your disk space.


## Installation

**Prerequisites:** You will need a terminal (at least 85×24 characters) and a local video player installed (e.g. `mpv`, `IINA`, or `VLC`).

The easiest way to get started is by using our quick install scripts. These scripts will automatically download the correct version for your computer.

### Homebrew (macOS & Linux)
```bash
brew tap mesamirh/moviebox-tui https://github.com/mesamirh/MovieBox-Tui
brew install moviebox-tui
```

### Install Script (macOS & Linux)
```bash
curl -fsSL https://raw.githubusercontent.com/mesamirh/MovieBox-Tui/main/install.sh | bash
```

### Windows
```powershell
powershell -c "irm https://raw.githubusercontent.com/mesamirh/MovieBox-Tui/main/install.ps1 | iex"
```

### Cargo (For Rust Developers)
```bash
cargo install moviebox-tui
```

<details>
<summary><i>Need to uninstall?</i></summary>

- **Homebrew:** `brew uninstall moviebox-tui && brew untap mesamirh/moviebox-tui`
- **Mac/Linux:** `sudo rm -f /usr/local/bin/moviebox-tui`
- **Windows:** `Remove-Item -Recurse -Force $env:USERPROFILE\AppData\Local\MovieBox-Tui`
- **Cargo:** `cargo uninstall moviebox-tui`
</details>



## Getting Started

Once installed, just open your terminal and type `moviebox-tui` to jump in!

### OpenSubtitles (Optional)

When a movie or episode has no Indonesian subtitle from the built-in source,
`moviebox-tui` can look up subtitles from the official
[OpenSubtitles](https://opensubtitles.com) API. Configure it with environment
variables (bring your own API key):

| Variable | Description | Default |
|----------|-------------|---------|
| `MOVIEBOX_OPENSUBTITLES_API_KEY` | OpenSubtitles API key (required for this feature) | *(empty)* |
| `MOVIEBOX_OPENSUBTITLES_USERNAME` | OpenSubtitles username (optional) | *(empty)* |
| `MOVIEBOX_OPENSUBTITLES_PASSWORD` | OpenSubtitles password (optional) | *(empty)* |
| `MOVIEBOX_OPENSUBTITLES_ENABLED` | Enable/disable the OpenSubtitles fallback | `true` |
| `MOVIEBOX_OPENSUBTITLES_LANGUAGES` | Comma-separated subtitle languages to search | `id,en` |
| `MOVIEBOX_OPENSUBTITLES_BASE_URL` | Override the OpenSubtitles API base URL | *(official API)* |

> **Note:** OpenSubtitles enforces a limited free-tier quota. The remaining
> daily downloads are shown right in the subtitle picker, and the automatic
> lookup is skipped when the quota is low or exhausted so manual picks stay
> available. Rate-limited requests (HTTP 429) are retried automatically, and
> API errors are reported in plain language. When no env vars are set, behavior
> is unchanged — playback and downloads proceed without the OpenSubtitles
> fallback. Searches and downloads are cached locally to avoid wasting quota.

### SubDL (Optional)

Before falling back to OpenSubtitles, `moviebox-tui` first tries the free
[SubDL](https://subdl.com) provider, so your OpenSubtitles quota is preserved
whenever a match exists. It needs its own (free) API key:

| Variable | Description | Default |
|----------|-------------|---------|
| `MOVIEBOX_SUBDL_API_KEY` | SubDL API key (required for this feature) | *(empty)* |
| `MOVIEBOX_SUBDL_ENABLED` | Enable/disable the SubDL fallback | `true` |
| `MOVIEBOX_SUBDL_LANGUAGES` | Comma-separated subtitle languages to search | `id,en` |
| `MOVIEBOX_SUBDL_BASE_URL` | Override the SubDL API base URL | *(official API)* |

> **Note:** SubDL is tried before OpenSubtitles, so any match found there never
> consumes the OpenSubtitles quota. When no API key is set, the provider is
> disabled and playback falls back to OpenSubtitles as usual. Subtitle searches
> and downloads are cached locally; downloads arrive as ZIP archives that are
> extracted automatically.

### Embedded Subtitles (Optional)

MKV/MP4 streams may carry subtitles baked into the file. When a stream has no
external subtitle, `moviebox-tui` shows a hint based on the file extension, and
can optionally probe the stream with `ffprobe` (if installed) to report the
embedded tracks. Playback is never changed — your player auto-loads them.

| Variable | Description | Default |
|----------|-------------|---------|
| `MOVIEBOX_PROBE_EMBEDDED_SUBTITLES` | Probe streams with `ffprobe` to list embedded subtitle tracks | `false` |

### Keyboard Controls

<table>
  <tr>
    <th align="left">Key</th>
    <th align="left">Action</th>
  </tr>
  <tr>
    <td>Alphanumeric</td>
    <td>Start searching instantly</td>
  </tr>
  <tr>
    <td><kbd>↑</kbd> <kbd>↓</kbd> <kbd>←</kbd> <kbd>→</kbd></td>
    <td>Navigate menus and grids</td>
  </tr>
  <tr>
    <td><kbd>Enter</kbd></td>
    <td>View details, pick episodes, or play video</td>
  </tr>
  <tr>
    <td><kbd>o</kbd></td>
    <td>Switch to a different video player on playback</td>
  </tr>
  <tr>
    <td><kbd>d</kbd></td>
    <td>Download an episode or an entire season</td>
  </tr>
  <tr>
    <td><kbd>Ctrl</kbd>+<kbd>p</kbd></td>
    <td>Switch between different content providers / sources</td>
  </tr>
  <tr>
    <td><kbd>Ctrl</kbd>+<kbd>t</kbd></td>
    <td>Toggle Live TV mode to browse IPTV channels</td>
  </tr>
  <tr>
    <td><kbd>?</kbd></td>
    <td>Open the global help menu</td>
  </tr>
  <tr>
    <td><kbd>q</kbd></td>
    <td>Quit (or use <kbd>Esc</kbd> to go back/clear search)</td>
  </tr>
</table>

### Slash Commands
You can type these special commands straight into the search bar:

<table>
  <tr>
    <th align="left">Command</th>
    <th align="left">Category</th>
    <th align="left">Description</th>
  </tr>
  <tr>
    <td><code>/discover</code> or <code>/home</code></td>
    <td>Streaming</td>
    <td>See what's trending right now</td>
  </tr>
  <tr>
    <td><code>/movies</code>, <code>/shows</code>, <code>/anime</code></td>
    <td>Streaming</td>
    <td>Jump straight to a specific category</td>
  </tr>
  <tr>
    <td><code>/list</code></td>
    <td>Live TV</td>
    <td>Show the list of available live channels</td>
  </tr>
  <tr>
    <td><code>/config</code></td>
    <td>Live TV</td>
    <td>Open the TV configuration menu to add your own m3u playlists</td>
  </tr>
  <tr>
    <td><code>/update</code></td>
    <td>General</td>
    <td>Check to see if there's a new version of the app</td>
  </tr>
  <tr>
    <td><code>/toggle-update</code></td>
    <td>General</td>
    <td>Turn automatic background update checking on or off</td>
  </tr>
</table>


## Contributing

I'd love your help making this even better! If you've got a big feature in mind, it's usually best to open an issue first so we can chat about it.

```bash
git clone https://github.com/mesamirh/MovieBox-Tui.git
cd MovieBox-Tui
cargo build
```

Just try to follow [Conventional Commits](https://www.conventionalcommits.org/) and make sure `cargo fmt` and `cargo clippy` are happy before you open a PR. You can check out [CONTRIBUTING.md](CONTRIBUTING.md) for the full rundown.


## Credits & Legal

Live TV channel playlists are graciously provided by [iptv-org/iptv](https://github.com/iptv-org/iptv).

> **Disclaimer:** This is a third-party client. It does not host or store any media and only resolves links from upstream APIs. Intended for personal use only.


## Community & Support

The best way to support MovieBox-TUI is simply to use it, share it, and leave a star on GitHub!

If you'd like to buy me a coffee for the late nights spent coding, you can use the addresses below.

- **EVM:** `0x7ea20d5fa29d87f33195f5a3b211ff94038d794c`
- **BTC:** `3MEAtqtRWrQBhnaMi3Zuf5nt2efNUS2LUQ`
- **LTC:** `ltc1qhjkq2n6tsayxj56n3c53uqv23v8vqhvc9g3vxl`

---

<div align="center">

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE) at your option.<br>
Built by [**@mesamirh**](https://github.com/mesamirh)

<sub>Not affiliated with any third-party content providers or operators.</sub>

</div>

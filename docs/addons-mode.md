# Stremio Addons

MovieBox-TUI supports Stremio HTTP addons. You can install standard addon manifest URLs to browse catalogs and stream media.

## Quick Start

- **Switch to Addons**: Press `Ctrl+P` on the home screen until `[Addons]` is selected.
- **Open Addon Manager**: Type `/config` or click `[Addons]` in the search bar.
- **Browse Catalogs**: Type `/browse` to view Top Movies, Top Series, and curated lists.
- **Search**: Type in the search bar to search titles across installed addons.

## Addon Manager

The Addon Manager lets you add, enable, or remove addon manifests:

- **Toggle Addon**: Press `Enter` or `Space` on an addon to enable (`✓`) or disable it.
- **Add Addon**: Select `+ Add manifest URL`, paste the addon manifest URL, and press `Enter`.
- **Remove Addon**: Press `d` or `Delete` to remove an addon.
- **Core Addon (Cinemeta)**: Pre-installed for movie and series metadata. It cannot be deleted.

## Supported Streams

- Streams are parsed and labeled with quality tags (`4K`, `1080p`, `720p`), codec, and file size.
- Video stream headers (`User-Agent`, `Referer`) are automatically forwarded to external players (`mpv`, `VLC`, `IINA`) and download workers.
- Watch progress and episode resumption are saved automatically to `/history`.

## Storage

Installed addons are saved to `addons_config.json` in your configuration directory:
- **Linux / macOS / Termux**: `~/.config/moviebox-tui/addons_config.json`
- **Windows**: `%APPDATA%\MovieBox-Tui\addons_config.json`

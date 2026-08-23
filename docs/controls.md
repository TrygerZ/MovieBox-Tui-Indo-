# Controls & Shortcuts

MovieBox-TUI is designed for fast keyboard navigation with complete mouse support throughout the interface. You can press `?` anywhere inside the application to open the mode-aware interactive help dialog.

## Global Shortcuts

| Key | Action |
| :--- | :--- |
| **Arrow keys** | Navigate lists, search results, tabs, and modal dialogs |
| **Enter** | Open, play, or confirm the selected item |
| **Esc** | Go back, dismiss popup dialog, or clear active search |
| **`Ctrl+S`** | Switch to standard **Streaming Mode** |
| **`Ctrl+T`** | Toggle / switch to **TV Mode** |
| **`Ctrl+A`** | Toggle / switch to **Addon Mode** |
| **`?`** | Open interactive in-app help menu |
| **`Ctrl+C` / `q`** | Quit application and restore terminal |

## Mode-Specific Controls

### Streaming Mode
- **`Ctrl+P`**: Cycle content providers (`MovieBox` $\to$ `4KHDHub` $\to$ `BDIX`).
- **`Tab` / `Shift+Tab`**: Switch details screen panes (Seasons, Episodes, Streams, Overview).
- **`Enter`**: Play with default player.
- **`o`**: Open player selection picker for the current stream.
- **`d`**: Download current episode or full season batch.
- **`r`**: Refresh search results / stream list.
- **`/browse`**: Open curated browse categories (Trending, Popular, Top Rated, etc.).
- **`/history`**: Open watch history.

### TV Mode (Live IPTV)
- **`Enter`**: Play selected TV channel immediately.
- **`o`**: Open player selection picker for the channel.
- **`r`**: Reload all active M3U playlist sources.
- **`/config`**: Open TV Playlist Manager dialog.
- **`/list`**: Show all loaded channels.

### Addon Mode (HTTP Addons)
- **`Ctrl+P` / `/config`**: Open Addon Manager dialog.
- **`Enter`**: Select title or play resolved stream.
- **`o`**: Open player selection picker for the stream.
- **`d`**: Download HTTP stream release.
- **`r`**: Refresh addon catalog search results.
- **`/browse`**: Browse curated addon catalogs (Top Movies, Top Series, Top Rated).

## Mouse Controls

| Action | Result |
| :--- | :--- |
| **Click search bar** | Enter search input mode |
| **Click suggestion item** | Search for that suggestion immediately |
| **Click search result row** | Select item and load preview; click again to open full details |
| **Click audio / season / episode / stream** | Switch audio language, change season, select episode, or start playback |
| **Click footer buttons** | Switch provider / mode, open help (`[?]`), or quit (`[q]`) |
| **Click modal buttons** | Choose a theme, subtitles, player, or confirm actions |
| **Click outside a modal** | Dismiss popup dialog |

## Slash Commands

Type these commands directly into the search bar:

| Command | Applicable Mode | Action |
| :--- | :--- | :--- |
| `/browse` | Streaming / Addon | Browse curated views (Trending, Popular) or Addon catalogs (Top Movies, Top Series) |
| `/history` | Streaming / Addon | View watch history with latest progress |
| `/list` | TV | View live TV channels |
| `/config` | TV / Addon | Manage IPTV playlists (TV Mode) or configure HTTP addons (Addon Mode) |
| `/enable-streaming` | All | Enable Streaming Mode navigation in bottom dock |
| `/disable-streaming` | All | Disable Streaming Mode navigation |
| `/enable-tv` | All | Enable TV Mode navigation in bottom dock |
| `/disable-tv` | All | Disable TV Mode navigation |
| `/enable-addons` | All | Enable Addon Mode navigation in bottom dock |
| `/disable-addons` | All | Disable Addon Mode navigation |
| `/download-dir` | All | View, change, or reset the download directory |
| `/theme` | All | Open theme picker (Mocha, Latte, Macchiato, Frappe, Nord, TokyoNight, Dracula, Gruvbox, RosePine) |
| `/clear-cache` | All | Clear temporary cache files |
| `/update` | All | Check if a new release is available on GitHub |
| `/toggle-update` | All | Toggle automatic startup update checks |
| `/enable-bdix` | Streaming | Enable BDIX FTP sources (Bangladesh ISPs only) |
| `/disable-bdix` | Streaming | Disable BDIX FTP sources |
| `/github` | All | Open the project repository |

# Controls & Shortcuts

Press `?` anywhere inside the application to open the interactive help dialog.

## Global Shortcuts

| Key | Action |
| :--- | :--- |
| **`↑` / `↓` / `k` / `j`** | Move selection up / down |
| **`←` / `→` / `h` / `l`** | Switch Details panes, step grid columns, or move input cursor |
| **`Home` / `End` / `g` / `G`** | Jump to start / end of lists or text inputs |
| **`PageUp` / `PageDown`** | Scroll lists by one page height |
| **`Enter`** | Play, open, or confirm selected item |
| **`Space` / `P`** | Direct resume playback from Home Continue Watching, `/history`, or play stream |
| **`Esc`** | Dismiss dialogs, close popups, or cancel search query |
| **`Tab` / `Shift+Tab`** | Cycle landing tabs (Resume / Favorites), switch Details panes, or auto-complete |
| **`d`** | Download selected stream/season (Details) or delete entry (`/history`, managers) |
| **`f`** | Toggle favorite on highlighted card or active media title |
| **`r`** | Refresh search results (Home), reload playlists (TV), or refresh streams (Details) |
| **`c`** | Clear active search query and return to landing (Normal mode) |
| **`x` / `X`** | Cancel active download |
| **`Ctrl+P`** | Cycle active streaming provider |
| **`Ctrl+S`** | Switch to **Streaming Mode** |
| **`Ctrl+T`** | Switch to **TV Mode** |
| **`?`** | Open interactive help overlay |
| **`Ctrl+C` / `q`** | Quit application |

## Text Editing (Search & Dialog Prompts)

- **`Left` / `Right`**: Move cursor one character.
- **`Home` / `End`**: Jump to start / end of input.
- **`Backspace` / `Delete`**: Delete character before / at cursor.
- **`Ctrl+W`**: Delete preceding word.
- **`Ctrl+U`**: Clear entire input line.
- **`Tab`**: Auto-complete suggestion or command.
- **`Enter`**: Submit input.
- **`Esc`**: Cancel input prompt.

## Dialog & Modal Controls

All popups (Themes, Providers, Players, Resolution, Subtitles, TV Manager, Addon Manager) share standard navigation:

- **`↑` / `↓` / `k` / `j`**: Navigate choices.
- **`Home` / `End`**: Jump to first / last item.
- **`Enter`**: Confirm selection.
- **`Esc` / `q`**: Dismiss popup without applying changes.
- **TV & Addon Managers**:
  - `r`: Reload playlist sources or addon catalogs.
  - `d` / `Delete`: Remove selected source.


## Details Screen Controls

| Key | Action |
| :--- | :--- |
| **`Tab` / `Shift+Tab`** | Switch active pane (Languages, Seasons, Episodes, Streams) |
| **`←` / `→` / `h` / `l`** | Switch pane horizontally (Languages ↔ Seasons/Episodes ↔ Streams) |
| **`↑` / `↓` / `k` / `j`** | Navigate items in active pane |
| **`Enter` / `Space` / `p`** | Play selected stream, expand season episodes, or select language |
| **`d`** | Start downloading selected stream (or entire season if Seasons pane active) |
| **`f`** | Toggle favorite bookmark for current title |
| **`i`** | Open synopsis and plot overview dialog |
| **`r`** | Refresh stream list for current title or episode |
| **`Esc` / `q`** | Return to previous screen (Home or Search) |

## Synopsis & Overview Modal

- **`↑` / `↓` / `k` / `j`**: Scroll synopsis text line-by-line.
- **`PageUp` / `PageDown`**: Scroll synopsis text 5 lines.
- **`Esc` / `q` / `i` / `Enter`**: Dismiss synopsis dialog.

## Settings Hub (`/settings`)

- **`Tab` / `Shift+Tab`**: Switch category tabs (General, Content Modes, Appearance, Maintenance).
- **`↑` / `↓` / `k` / `j`**: Navigate setting rows.
- **`←` / `→` / `h` / `l`**: Adjust setting values or toggle options.
- **`Enter` / `Space`**: Activate setting row (edit path, pick theme, run maintenance action).
- **`d`** (on Download Folder): Reset custom download path back to `~/Downloads/MovieBox-TUI`.
- **`Esc` / `q`**: Close settings and persist changes.

## Update Notification Modal

- **`u`**: Update immediately (Direct installation). Downloads, verifies, stages, and restarts the process automatically.
- **`b`**: Copy Homebrew update command to clipboard.
- **`s`**: Copy Scoop update command to clipboard.
- **`o`**: Open GitHub release notes in default browser.
- **`Esc` / `q`**: Dismiss update prompt.
## Slash Commands

Type `/` in the search bar on the Home screen to trigger slash commands:

| Command | Mode | Action |
| :--- | :--- | :--- |
| **`/settings`** | All | Open Settings Hub |
| **`/config`** | Contextual | Open TV Playlist Manager (TV mode) or Addon Manager (Addons provider) |
| **`/browse`** | Streaming | Browse curated views (Trending, Popular, Addon catalogs) |
| **`/history`** | Streaming | View watch history and resume playback |
| **`/favorites`** | Streaming | View starred titles |
| **`/list`** | TV Mode | View all loaded live TV channels |
| **`/clear`** | All | Reset search input and results |
| **`/help`** | All | Open interactive help dialog |
| **`/exit`** | All | Quit application |

## Mouse Controls

- **Left Click**: Select and activate search cards, tabs, buttons, or playlist entries.
- **Scroll Wheel**: Scroll search results, seasons, episodes, and modal dialogs.
- **Click Outside**: Click anywhere outside an active dialog to dismiss it.

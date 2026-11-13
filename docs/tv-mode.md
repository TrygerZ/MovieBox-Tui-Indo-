# TV Mode

TV mode streams live television channels from M3U playlists. Playlists can be loaded from remote URLs or local files.

## Entering TV Mode

- Press **`Ctrl+T`** to switch to TV mode (and **`Ctrl+S`** to switch back to Streaming mode).
- If no playlists are loaded on entry, the playlist manager opens automatically.
- Type in the search bar to filter channels by name or group; press `Enter` to play.

## Managing Playlists

1. Enter **`/config`** while in TV mode to open the playlist manager.
2. Select **`+ Add playlist`**, enter the URL or local file path, and press `Enter`.
   - **URL**: `https://example.com/playlist.m3u`
   - **File**: `~/playlists/channels.m3u` (supports `~/` tilde expansion and Windows paths)
3. Press **`r`** inside the manager to reload all playlist sources.
4. Highlight an entry and press **`d`** (or `Delete`) to remove it.
5. Press **`Esc`** to close the manager.

Playlists are saved to `tv_config.json` in your configuration directory.

## Channel Parsing & Safeguards

- **Size Limits**: Files and downloads larger than 15 MB are rejected to prevent excessive memory consumption.
- **Deduplication**: Channels sharing identical stream URLs across multiple playlists are deduplicated.
- **Attributes**: Parses `#EXTINF:` tags for `tvg-id`, `tvg-logo`, `group-title`, and channel name.

## Playback & Commands

- **Playback**: Press `Enter` on a channel to launch your default media player. Live streams without fixed durations are exempted from watch history.
- **Commands**:
  - **`/list`**: View all loaded TV channels.
  - **`r`**: Reload all active playlists.
  - **`/config`**: Open the playlist manager.

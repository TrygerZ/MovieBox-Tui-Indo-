# Binary Cache & Storage

MovieBox-TUI uses a disk-backed binary MessagePack caching engine to minimize network requests while maintaining fast startup times.

## Cache Directory

- **Linux / macOS / Termux**: `~/.cache/moviebox-tui/`
- **Windows**: `%LOCALAPPDATA%\MovieBox-Tui\cache\`

Override with the `MOVIEBOX_CACHE_DIR` environment variable.

## Binary Cache Architecture

Cache files use a structured MessagePack envelope preceded by a 4-byte magic signature (`MBC1`):

```text
[ 0x4D 0x42 0x43 0x31 ] [ MessagePack Encoded Envelope ]
```

The envelope stores the expiration timestamp (`expires_at: u64`) alongside the serialized payload. On read, if the current time exceeds `expires_at` or the file age exceeds the TTL, the cache is invalidated and refetched.

## Cache TTLs

| Namespace | TTL | Description |
| :--- | :--- | :--- |
| `homepage` | 1 Hour | Trending, popular, and curated `/browse` feeds |
| `search` | 24 Hours | Search query results per provider |
| `details` | Dynamic (CloudFront TTL) | Subject details, cast, and signed streaming cookies (auto-adapts to upstream cookie expiration, min 1h, max 24h) |
| `streams` | Dynamic (Cookie Expiration) | Direct streaming and DASH manifest URLs (auto-adapts to `CloudFront-Policy` and `Edge-Cache-Cookie` expiration timestamp `:t=`, min 1m, max 2h) |
| `captions` | 24 Hours | Aggregated MovieBox external subtitle lists (primary + sibling dubs, background-prefetched on Details load) |
| `posters` | 30 Days | Downloaded poster image buffers |
| `tv` | 24 Hours | Remote M3U playlist text snapshots |

## Durability & Safety

- **Atomic File Writes**: All cache entries write to a temporary sidecar file (`.tmp-<pid>-<stamp>`) and rename atomically to the destination so interrupted writes never corrupt cache files.
- **Automatic Purging**: Stale cache entries older than 7 days are automatically removed by a background worker at startup.
- **Manual Purge**: Enter `/settings` → **Maintenance** → **Clear Disk Cache** to immediately wipe all cached files, images, and playlist snapshots.

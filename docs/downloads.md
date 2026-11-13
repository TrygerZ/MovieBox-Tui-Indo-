# Downloader

MovieBox-TUI includes a multi-segment HTTP chunked downloader supporting pause, resume, and authentication header forwarding.

## Overview

- **Storage Location**: Defaults to `~/Downloads/MovieBox-TUI/`. Configurable via `/settings` (General → Download Folder).
- **Multi-Segment Engine**: Files are partitioned into concurrent byte ranges using HTTP RFC 7233 `Range: bytes=X-Y` requests.
- **Single-Stream Fallback**: If an upstream server or CDN does not support range requests (returns HTTP `200 OK` instead of `206 Partial Content`) or terminates concurrent connections with HTTP 403 or 429, the engine falls back to single-stream sequential downloading.
## File Lifecycle & State Files

During download, files are saved with temporary extensions:

```text
destination.mp4.part       # Pre-allocated in-place byte buffer across all workers
destination.mp4.part.json  # Download state (ETag, Last-Modified, total size, per-segment byte offsets)
destination.mp4            # Final verified output (atomic rename on completion)
```

Upon completion, temporary sidecars are verified and renamed atomically to the target filename.

## Download Controls

- Press **`d`** on any stream in the Details screen to start downloading immediately.
- Press **`x`** or **`X`** during an active download to cancel or pause the transfer. Partial `.part` data is preserved on disk for resumption.
- Downloads run cooperatively in the background so you can browse or search while transfers finish.

## Header Forwarding

Authenticated streams (such as MovieBox DASH manifests or 4KHDHub mirrors) forward required headers (`User-Agent`, `Referer`, signed CloudFront cookies) to download workers so CDN transfers complete without `403 Forbidden` errors.

## DASH Streams (`yt-dlp`)

MovieBox DASH streams require `yt-dlp` and `ffmpeg` to download and mux adaptive video and audio representations:

- **Windows**: `winget install yt-dlp.yt-dlp Gyan.FFmpeg`
- **macOS**: `brew install yt-dlp ffmpeg`
- **Android (Termux)**: `pkg install yt-dlp ffmpeg`
- **Linux**: Install `yt-dlp` and `ffmpeg` via system package manager.

The downloader checks both binaries before starting the transfer and reports which binary is missing if either is absent. Fragments are pulled with `--concurrent-fragments 32` and `--http-chunk-size 95K` (keeping individual range sub-requests under Tengine's `limit_rate_after 96k` throttle boundary) alongside retry bounds (`--fragment-retries 10`, `--retries 5`, `--socket-timeout 30`). Subprocess errors during transfer report the underlying `yt-dlp` diagnostic in the failure notification.

All other providers (4KHDHub, Dramachi, BDIX, DhakaFlix, CircleFTP, Stremio Addons, TV mode) download directly through the internal multi-segment HTTP engine (4 to 16 concurrent workers with 1 MiB write buffers writing in-place to the pre-allocated `.part` file).

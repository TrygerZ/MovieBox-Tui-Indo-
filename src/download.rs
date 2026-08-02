use reqwest::{
    Client, StatusCode,
    header::{ACCEPT_RANGES, CONTENT_RANGE, ETAG, IF_RANGE, LAST_MODIFIED, RANGE},
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::io::AsyncWriteExt;

const MAX_ATTEMPTS: usize = 4;
const SEGMENT_THRESHOLD: u64 = 32 * 1024 * 1024;
const MAX_SEGMENTS: usize = 8;

pub fn safe_file_stem(value: &str) -> String {
    let mut stem = value
        .chars()
        .take(120)
        .map(|character| {
            if character.is_control()
                || character.is_whitespace()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    stem = stem.trim_matches(['.', ' ', '_']).to_string();
    if stem.is_empty() {
        return "MovieBox-Tui_Stream".into();
    }
    let upper = stem.to_ascii_uppercase();
    let reserved = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || upper
            .strip_prefix("COM")
            .or_else(|| upper.strip_prefix("LPT"))
            .is_some_and(|number| {
                number.len() == 1 && number.bytes().all(|byte| matches!(byte, b'1'..=b'9'))
            });
    if reserved {
        stem.push('_');
    }
    stem
}

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub bytes_per_second: f64,
    pub attempt: usize,
    pub workers: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadOutcome {
    Completed { bytes: u64 },
    Paused { bytes: u64 },
}

/// Cap reported bytes at the known total so the UI percentage never exceeds 100.
fn cap_at_total(downloaded: u64, total: Option<u64>) -> u64 {
    match total {
        Some(total) => downloaded.min(total),
        None => downloaded,
    }
}

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("server returned HTTP {0}")]
    Http(StatusCode),
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("file error: {0}")]
    File(#[from] std::io::Error),
    #[error("invalid partial response: {0}")]
    InvalidRange(String),
    #[error("download ended at {downloaded} of {expected} bytes")]
    Incomplete { downloaded: u64, expected: u64 },
    #[error("download paused")]
    Paused,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ResumeMetadata {
    etag: Option<String>,
    last_modified: Option<String>,
    total: Option<u64>,
    segments: Option<usize>,
}

pub async fn download<F>(
    client: &Client,
    url: &str,
    destination: &Path,
    cancel: Arc<AtomicBool>,
    mut report: F,
) -> Result<DownloadOutcome, DownloadError>
where
    F: FnMut(DownloadProgress),
{
    let partial = sidecar_path(destination, "part");
    let metadata_path = sidecar_path(destination, "part.json");
    let mut metadata = read_metadata(&metadata_path).await;
    let started = Instant::now();
    let mut last_report = Instant::now() - Duration::from_secs(1);
    let mut last_error = None;
    let mut segmented_disabled = false;

    for attempt in 1..=MAX_ATTEMPTS {
        if cancel.load(Ordering::Relaxed) {
            return Ok(DownloadOutcome::Paused {
                bytes: file_len(&partial).await,
            });
        }

        let mut offset = file_len(&partial).await;
        let mut request = client.get(url);
        if offset > 0 {
            request = request.header(RANGE, format!("bytes={offset}-"));
            if let Some(validator) = metadata.etag.as_ref().or(metadata.last_modified.as_ref()) {
                request = request.header(IF_RANGE, validator);
            }
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(error) => {
                last_error = Some(DownloadError::Network(error));
                retry_delay(attempt).await;
                continue;
            }
        };

        if response.status() == StatusCode::RANGE_NOT_SATISFIABLE
            && metadata.total.is_some_and(|total| offset > total)
        {
            // Remote file shrank below our partial; discard and start over.
            truncate(&partial).await?;
            metadata = ResumeMetadata::default();
            write_metadata(&metadata_path, &metadata).await?;
            last_error = Some(DownloadError::InvalidRange(
                "remote file size changed; partial reset".into(),
            ));
            retry_delay(attempt).await;
            continue;
        }
        if response.status() == StatusCode::RANGE_NOT_SATISFIABLE && metadata.total == Some(offset)
        {
            finalize(&partial, &metadata_path, destination).await?;
            return Ok(DownloadOutcome::Completed { bytes: offset });
        }
        if !response.status().is_success() {
            last_error = Some(DownloadError::Http(response.status()));
            retry_delay(attempt).await;
            continue;
        }

        if !segmented_disabled
            && offset == 0
            && response.status() == StatusCode::OK
            && response
                .headers()
                .get(ACCEPT_RANGES)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.eq_ignore_ascii_case("bytes"))
            && response
                .content_length()
                .is_some_and(|total| total >= SEGMENT_THRESHOLD)
        {
            let total = response.content_length().unwrap_or_default();
            let segments = segment_count(total);
            let current_metadata = ResumeMetadata {
                etag: header_string(&response, ETAG),
                last_modified: header_string(&response, LAST_MODIFIED),
                total: Some(total),
                segments: Some(segments),
            };
            if !metadata_matches(&metadata, &current_metadata) {
                remove_segment_files(destination).await;
            }
            write_metadata(&metadata_path, &current_metadata).await?;
            drop(response);
            match download_segmented(
                client,
                url,
                destination,
                &metadata_path,
                current_metadata,
                cancel.clone(),
                &mut report,
            )
            .await
            {
                Err(DownloadError::InvalidRange(_)) => {
                    remove_segment_files(destination).await;
                    metadata = ResumeMetadata::default();
                    write_metadata(&metadata_path, &metadata).await?;
                    segmented_disabled = true;
                    continue;
                }
                result => return result,
            }
        }

        let response_total = if response.status() == StatusCode::PARTIAL_CONTENT {
            let content_range = response
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| DownloadError::InvalidRange("Content-Range missing".into()))?;
            let (start, total) = parse_content_range(content_range)?;
            if start != offset {
                return Err(DownloadError::InvalidRange(format!(
                    "requested byte {offset}, received {start}"
                )));
            }
            total
        } else {
            if offset > 0 {
                truncate(&partial).await?;
                offset = 0;
            }
            response.content_length()
        };

        if offset > 0
            && let (Some(previous), Some(current)) = (metadata.total, response_total)
            && previous != current
        {
            truncate(&partial).await?;
            metadata = ResumeMetadata::default();
            write_metadata(&metadata_path, &metadata).await?;
            last_error = Some(DownloadError::InvalidRange(
                "remote file size changed; partial reset".into(),
            ));
            retry_delay(attempt).await;
            continue;
        }

        metadata.etag = header_string(&response, ETAG).or(metadata.etag);
        metadata.last_modified = header_string(&response, LAST_MODIFIED).or(metadata.last_modified);
        metadata.total = response_total.or(metadata.total);
        metadata.segments = None;
        write_metadata(&metadata_path, &metadata).await?;

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&partial)
            .await?;
        let mut response = response;
        let mut downloaded = offset;
        let transfer_started = Instant::now();

        loop {
            if cancel.load(Ordering::Relaxed) {
                file.flush().await?;
                return Ok(DownloadOutcome::Paused { bytes: downloaded });
            }

            match response.chunk().await {
                Ok(Some(chunk)) => {
                    file.write_all(&chunk).await?;
                    downloaded += chunk.len() as u64;
                    if last_report.elapsed() >= Duration::from_millis(200) {
                        let elapsed = started.elapsed().as_secs_f64();
                        let session_bytes = downloaded.saturating_sub(offset);
                        report(DownloadProgress {
                            downloaded: cap_at_total(downloaded, metadata.total),
                            total: metadata.total,
                            bytes_per_second: if elapsed > 0.0 {
                                session_bytes as f64 / elapsed
                            } else {
                                0.0
                            },
                            attempt,
                            workers: 1,
                        });
                        last_report = Instant::now();
                    }
                }
                Ok(None) => {
                    file.flush().await?;
                    file.sync_data().await?;
                    if let Some(expected) = metadata.total
                        && downloaded != expected
                    {
                        last_error = Some(DownloadError::Incomplete {
                            downloaded,
                            expected,
                        });
                        break;
                    }
                    report(DownloadProgress {
                        downloaded: cap_at_total(downloaded, metadata.total),
                        total: metadata.total.or(Some(downloaded)),
                        bytes_per_second: if transfer_started.elapsed().as_secs_f64() > 0.0 {
                            downloaded.saturating_sub(offset) as f64
                                / transfer_started.elapsed().as_secs_f64()
                        } else {
                            0.0
                        },
                        attempt,
                        workers: 1,
                    });
                    finalize(&partial, &metadata_path, destination).await?;
                    return Ok(DownloadOutcome::Completed { bytes: downloaded });
                }
                Err(error) => {
                    file.flush().await?;
                    last_error = Some(DownloadError::Network(error));
                    break;
                }
            }
        }

        retry_delay(attempt).await;
    }

    Err(last_error.unwrap_or(DownloadError::InvalidRange(
        "download failed without a response".into(),
    )))
}

async fn download_segmented<F>(
    client: &Client,
    url: &str,
    destination: &Path,
    metadata_path: &Path,
    metadata: ResumeMetadata,
    cancel: Arc<AtomicBool>,
    report: &mut F,
) -> Result<DownloadOutcome, DownloadError>
where
    F: FnMut(DownloadProgress),
{
    let total = metadata
        .total
        .ok_or_else(|| DownloadError::InvalidRange("segment total missing".into()))?;
    let segments = metadata.segments.unwrap_or_else(|| segment_count(total));
    let ranges = segment_ranges(total, segments);
    let mut initial = 0;

    for (index, (start, end)) in ranges.iter().copied().enumerate() {
        let path = segment_path(destination, index);
        let expected = end - start + 1;
        let length = file_len(&path).await;
        if length > expected {
            truncate(&path).await?;
        } else {
            initial += length;
        }
    }

    let downloaded = Arc::new(AtomicU64::new(initial));
    let (progress_sender, mut progress_receiver) =
        tokio::sync::mpsc::unbounded_channel::<(u64, usize)>();
    let mut tasks = tokio::task::JoinSet::new();
    let validator = metadata.etag.clone().or(metadata.last_modified.clone());

    for (index, (start, end)) in ranges.iter().copied().enumerate() {
        let client = client.clone();
        let url = url.to_string();
        let path = segment_path(destination, index);
        let cancel = cancel.clone();
        let progress_sender = progress_sender.clone();
        let validator = validator.clone();
        tasks.spawn(async move {
            download_segment(
                &client,
                &url,
                &path,
                start,
                end,
                total,
                validator,
                cancel,
                progress_sender,
            )
            .await
        });
    }
    drop(progress_sender);

    let started = Instant::now();
    let mut last_report = Instant::now() - Duration::from_secs(1);
    let mut finished = 0;
    while finished < segments {
        tokio::select! {
            progress = progress_receiver.recv() => {
                if let Some((bytes, attempt)) = progress {
                    let current = downloaded.fetch_add(bytes, Ordering::Relaxed) + bytes;
                    if last_report.elapsed() >= Duration::from_millis(200) {
                        let elapsed = started.elapsed().as_secs_f64();
                        report(DownloadProgress {
                            downloaded: current,
                            total: Some(total),
                            bytes_per_second: if elapsed > 0.0 {
                                current.saturating_sub(initial) as f64 / elapsed
                            } else {
                                0.0
                            },
                            attempt,
                            workers: segments,
                        });
                        last_report = Instant::now();
                    }
                }
            }
            result = tasks.join_next() => {
                match result {
                    Some(Ok(Ok(()))) => finished += 1,
                    Some(Ok(Err(DownloadError::Paused))) => {
                        tasks.abort_all();
                        return Ok(DownloadOutcome::Paused {
                            bytes: downloaded.load(Ordering::Relaxed),
                        });
                    }
                    Some(Ok(Err(error))) => {
                        tasks.abort_all();
                        return Err(error);
                    }
                    Some(Err(error)) => {
                        tasks.abort_all();
                        return Err(DownloadError::InvalidRange(format!(
                            "download worker stopped: {error}"
                        )));
                    }
                    None => break,
                }
            }
        }
    }

    if cancel.load(Ordering::Relaxed) {
        return Ok(DownloadOutcome::Paused {
            bytes: downloaded.load(Ordering::Relaxed),
        });
    }

    let assembly = sidecar_path(destination, "assembling");
    let mut output = tokio::fs::File::create(&assembly).await?;
    for index in 0..segments {
        let path = segment_path(destination, index);
        let mut part = tokio::fs::File::open(&path).await?;
        tokio::io::copy(&mut part, &mut output).await?;
    }
    output.flush().await?;
    output.sync_data().await?;
    if file_len(&assembly).await != total {
        return Err(DownloadError::Incomplete {
            downloaded: file_len(&assembly).await,
            expected: total,
        });
    }
    tokio::fs::rename(&assembly, destination).await?;
    for index in 0..segments {
        let _ = tokio::fs::remove_file(segment_path(destination, index)).await;
    }
    let _ = tokio::fs::remove_file(metadata_path).await;
    report(DownloadProgress {
        downloaded: total,
        total: Some(total),
        bytes_per_second: if started.elapsed().as_secs_f64() > 0.0 {
            total.saturating_sub(initial) as f64 / started.elapsed().as_secs_f64()
        } else {
            0.0
        },
        attempt: 1,
        workers: segments,
    });
    Ok(DownloadOutcome::Completed { bytes: total })
}

#[allow(clippy::too_many_arguments)]
async fn download_segment(
    client: &Client,
    url: &str,
    path: &Path,
    start: u64,
    end: u64,
    total: u64,
    validator: Option<String>,
    cancel: Arc<AtomicBool>,
    progress: tokio::sync::mpsc::UnboundedSender<(u64, usize)>,
) -> Result<(), DownloadError> {
    let expected = end - start + 1;
    let mut last_error = None;

    for attempt in 1..=MAX_ATTEMPTS {
        if cancel.load(Ordering::Relaxed) {
            return Err(DownloadError::Paused);
        }
        let existing = file_len(path).await.min(expected);
        if existing == expected {
            return Ok(());
        }
        let requested_start = start + existing;
        let mut request = client
            .get(url)
            .header(RANGE, format!("bytes={requested_start}-{end}"));
        if let Some(validator) = &validator {
            request = request.header(IF_RANGE, validator);
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(error) => {
                last_error = Some(DownloadError::Network(error));
                retry_delay(attempt).await;
                continue;
            }
        };
        if response.status() != StatusCode::PARTIAL_CONTENT {
            last_error = Some(DownloadError::InvalidRange(format!(
                "worker expected HTTP 206, received {}",
                response.status()
            )));
            retry_delay(attempt).await;
            continue;
        }
        let content_range = response
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| DownloadError::InvalidRange("Content-Range missing".into()))?;
        let (received_start, received_total) = parse_content_range(content_range)?;
        if received_start != requested_start || received_total != Some(total) {
            return Err(DownloadError::InvalidRange(format!(
                "worker requested {requested_start}-{end}/{total}, received {content_range}"
            )));
        }

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;
        let mut response = response;
        let mut written = existing;
        loop {
            if cancel.load(Ordering::Relaxed) {
                file.flush().await?;
                return Err(DownloadError::Paused);
            }
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    let remaining = expected - written;
                    let bytes = chunk.len().min(remaining as usize);
                    file.write_all(&chunk[..bytes]).await?;
                    written += bytes as u64;
                    progress.send((bytes as u64, attempt)).ok();
                    if written == expected {
                        file.flush().await?;
                        file.sync_data().await?;
                        return Ok(());
                    }
                }
                Ok(None) => {
                    file.flush().await?;
                    last_error = Some(DownloadError::Incomplete {
                        downloaded: written,
                        expected,
                    });
                    break;
                }
                Err(error) => {
                    file.flush().await?;
                    last_error = Some(DownloadError::Network(error));
                    break;
                }
            }
        }
        retry_delay(attempt).await;
    }

    Err(last_error.unwrap_or(DownloadError::Incomplete {
        downloaded: file_len(path).await,
        expected,
    }))
}

fn segment_count(total: u64) -> usize {
    if total < 256 * 1024 * 1024 {
        2
    } else if total < 2 * 1024 * 1024 * 1024 {
        4
    } else {
        MAX_SEGMENTS
    }
}

fn segment_ranges(total: u64, segments: usize) -> Vec<(u64, u64)> {
    let size = total / segments as u64;
    (0..segments)
        .map(|index| {
            let start = index as u64 * size;
            let end = if index + 1 == segments {
                total - 1
            } else {
                start + size - 1
            };
            (start, end)
        })
        .collect()
}

fn segment_path(destination: &Path, index: usize) -> PathBuf {
    sidecar_path(destination, &format!("part.{index}"))
}

fn metadata_matches(previous: &ResumeMetadata, current: &ResumeMetadata) -> bool {
    previous.total == current.total
        && previous.segments == current.segments
        && match (&previous.etag, &current.etag) {
            (Some(previous), Some(current)) => previous == current,
            _ => match (&previous.last_modified, &current.last_modified) {
                (Some(previous), Some(current)) => previous == current,
                _ => true,
            },
        }
}

async fn remove_segment_files(destination: &Path) {
    for index in 0..MAX_SEGMENTS {
        let _ = tokio::fs::remove_file(segment_path(destination, index)).await;
    }
    let _ = tokio::fs::remove_file(sidecar_path(destination, "assembling")).await;
}

fn parse_content_range(value: &str) -> Result<(u64, Option<u64>), DownloadError> {
    let value = value
        .strip_prefix("bytes ")
        .ok_or_else(|| DownloadError::InvalidRange(value.into()))?;
    let (range, total) = value
        .split_once('/')
        .ok_or_else(|| DownloadError::InvalidRange(value.into()))?;
    let (start, _) = range
        .split_once('-')
        .ok_or_else(|| DownloadError::InvalidRange(value.into()))?;
    let start = start
        .parse()
        .map_err(|_| DownloadError::InvalidRange(value.into()))?;
    let total = if total == "*" {
        None
    } else {
        Some(
            total
                .parse()
                .map_err(|_| DownloadError::InvalidRange(value.into()))?,
        )
    };
    Ok((start, total))
}

fn sidecar_path(destination: &Path, suffix: &str) -> PathBuf {
    let mut name = destination.as_os_str().to_os_string();
    name.push(".");
    name.push(suffix);
    PathBuf::from(name)
}

async fn file_len(path: &Path) -> u64 {
    tokio::fs::metadata(path)
        .await
        .map(|metadata| metadata.len())
        .unwrap_or_default()
}

async fn truncate(path: &Path) -> Result<(), std::io::Error> {
    tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .await
        .map(|_| ())
}

async fn finalize(
    partial: &Path,
    metadata: &Path,
    destination: &Path,
) -> Result<(), std::io::Error> {
    tokio::fs::rename(partial, destination).await?;
    let _ = tokio::fs::remove_file(metadata).await;
    Ok(())
}

async fn read_metadata(path: &Path) -> ResumeMetadata {
    let Ok(bytes) = tokio::fs::read(path).await else {
        return ResumeMetadata::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

async fn write_metadata(path: &Path, metadata: &ResumeMetadata) -> Result<(), std::io::Error> {
    let bytes = serde_json::to_vec(metadata).map_err(std::io::Error::other)?;
    tokio::fs::write(path, bytes).await
}

fn header_string(
    response: &reqwest::Response,
    name: reqwest::header::HeaderName,
) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

async fn retry_delay(attempt: usize) {
    if attempt < MAX_ATTEMPTS {
        tokio::time::sleep(Duration::from_secs(1 << (attempt - 1))).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::sync::Mutex;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("moviebox-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Minimal blocking mock HTTP server: sends the headers immediately, then
    /// (after `body_delay`) the body, so the progress-report loop in `download`
    /// has time to emit mid-stream reports. Serves up to `MAX_ATTEMPTS`
    /// connections and exits when idle past its deadline.
    fn spawn_mock_server(
        responder: impl Fn(&str) -> (String, Vec<u8>) + Send + 'static,
        body_delay: Duration,
    ) -> std::net::SocketAddr {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            listener.set_nonblocking(true).unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(15);
            for _ in 0..MAX_ATTEMPTS {
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(conn) => break conn,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            if std::time::Instant::now() > deadline {
                                return;
                            }
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(_) => return,
                    }
                };
                let mut buf = [0u8; 8192];
                let Ok(n) = stream.read(&mut buf) else { continue };
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                let (head, body) = responder(&request);
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.flush();
                std::thread::sleep(body_delay);
                let _ = stream.write_all(&body);
                let _ = stream.flush();
            }
        });
        addr
    }

    #[tokio::test]
    async fn resume_speed_uses_session_bytes_not_total() {
        let dir = temp_dir("resume_speed");
        let dest = dir.join("movie.bin");
        // Pre-existing partial: 1 MiB on disk, metadata records 1 MiB + 4096.
        tokio::fs::write(dir.join("movie.bin.part"), vec![b'a'; 1_000_000])
            .await
            .unwrap();
        tokio::fs::write(dir.join("movie.bin.part.json"), br#"{"total":1004096}"#)
            .await
            .unwrap();

        let body = vec![b'b'; 4096];
        let addr = spawn_mock_server(
            move |_request: &str| {
                (
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 1000000-1004095/1004096\r\nContent-Length: 4096\r\nConnection: close\r\n\r\n"
                        .to_string(),
                    body.clone(),
                )
            },
            Duration::from_millis(400),
        );

        let client = reqwest::Client::new();
        let cancel = Arc::new(AtomicBool::new(false));
        let reports = Arc::new(Mutex::new(Vec::new()));
        let reporter = reports.clone();
        let outcome = download(
            &client,
            &format!("http://{addr}/movie.bin"),
            &dest,
            cancel,
            move |progress| reporter.lock().unwrap().push(progress),
        )
        .await
        .unwrap();

        assert_eq!(outcome, DownloadOutcome::Completed { bytes: 1_004_096 });
        let offset = 1_000_000u64;
        let reports = reports.lock().unwrap();
        assert!(!reports.is_empty());
        for progress in reports.iter() {
            // Speed must be session bytes / elapsed, never total / elapsed.
            // The server holds the body for 400 ms, so elapsed >= 0.1 s: fixed
            // code yields <= session * 10 B/s; the old bug divided offset +
            // session by elapsed, producing ~MB/s.
            let session = progress.downloaded.saturating_sub(offset);
            assert!(
                progress.bytes_per_second <= session as f64 * 10.0,
                "speed {:.0} B/s for session {session} B — inflated by the resumed offset",
                progress.bytes_per_second
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn shrunk_remote_file_resets_partial_and_restarts() {
        let dir = temp_dir("shrunk_remote");
        let dest = dir.join("movie.bin");
        // Partial (100 B) is larger than the recorded total (50 B) and the
        // remote has since shrunk to 30 B → first request gets a 416.
        tokio::fs::write(dir.join("movie.bin.part"), vec![b'a'; 100])
            .await
            .unwrap();
        tokio::fs::write(dir.join("movie.bin.part.json"), br#"{"total":50}"#)
            .await
            .unwrap();

        let body = vec![b'c'; 30];
        let addr = spawn_mock_server(
            move |request: &str| {
                if request.contains("Range: bytes=100-") {
                    (
                        "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_string(),
                        Vec::new(),
                    )
                } else {
                    (
                        "HTTP/1.1 200 OK\r\nContent-Length: 30\r\nConnection: close\r\n\r\n"
                            .to_string(),
                        body.clone(),
                    )
                }
            },
            Duration::from_millis(100),
        );

        let client = reqwest::Client::new();
        let cancel = Arc::new(AtomicBool::new(false));
        let outcome = download(
            &client,
            &format!("http://{addr}/movie.bin"),
            &dest,
            cancel,
            |_| {},
        )
        .await
        .unwrap();

        assert_eq!(outcome, DownloadOutcome::Completed { bytes: 30 });
        let final_len = tokio::fs::metadata(&dest).await.unwrap().len();
        assert_eq!(final_len, 30, "partial must be discarded before restart");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn progress_never_exceeds_total() {
        let dir = temp_dir("clamped_progress");
        let dest = dir.join("movie.bin");
        // Server claims a 1000-byte range total but delivers 1200 bytes.
        let body = vec![b'x'; 1200];
        let addr = spawn_mock_server(
            move |_request: &str| {
                (
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-999/1000\r\nContent-Length: 1200\r\nConnection: close\r\n\r\n"
                        .to_string(),
                    body.clone(),
                )
            },
            Duration::from_millis(300),
        );

        let client = reqwest::Client::new();
        let cancel = Arc::new(AtomicBool::new(false));
        let reports = Arc::new(Mutex::new(Vec::new()));
        let reporter = reports.clone();
        let result = download(
            &client,
            &format!("http://{addr}/movie.bin"),
            &dest,
            cancel,
            move |progress| reporter.lock().unwrap().push(progress),
        )
        .await;

        // Over-delivery must fail the transfer (Incomplete), never report >100%.
        assert!(result.is_err(), "expected failure, got {result:?}");
        let reports = reports.lock().unwrap();
        assert!(!reports.is_empty());
        for progress in reports.iter() {
            if let Some(total) = progress.total {
                assert!(
                    progress.downloaded <= total,
                    "reported {} > total {}",
                    progress.downloaded,
                    total
                );
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

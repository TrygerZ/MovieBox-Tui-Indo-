use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

pub fn compute_sha256(path: &Path) -> Result<String, String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("failed to open file for hashing: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|e| format!("failed to read file for hashing: {e}"))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    let hash = hasher.finalize();
    let mut hex_str = String::with_capacity(hash.len() * 2);
    for byte in hash {
        use std::fmt::Write;
        let _ = write!(hex_str, "{byte:02x}");
    }
    Ok(hex_str)
}

pub fn parse_sha256sums(content: &str, expected_filename: &str) -> Result<String, String> {
    let target = expected_filename.trim();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut parts = line.split_whitespace();
        let hash = match parts.next() {
            Some(h) if h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()) => h,
            _ => continue,
        };

        let filename_part = match parts.next() {
            Some(f) => f.trim_start_matches('*'),
            None => continue,
        };

        if filename_part == target || filename_part.ends_with(&format!("/{target}")) {
            return Ok(hash.to_ascii_lowercase());
        }
    }

    Err(format!(
        "checksum for {expected_filename} not found in SHA256SUMS"
    ))
}

pub fn verify_checksum(
    file_path: &Path,
    sha256sums_content: &str,
    expected_filename: &str,
) -> Result<(), String> {
    let expected_hash = parse_sha256sums(sha256sums_content, expected_filename)?;
    let actual_hash = compute_sha256(file_path)?;

    if actual_hash.to_ascii_lowercase() != expected_hash {
        return Err(format!(
            "checksum mismatch for {expected_filename}: expected {expected_hash}, got {actual_hash}"
        ));
    }

    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_compute_sha256() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"hello world\n").unwrap();
        let hash = compute_sha256(file.path()).unwrap();
        assert_eq!(
            hash,
            "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447"
        );
    }

    #[test]
    fn test_verify_checksum() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"hello world\n").unwrap();
        let sha_file =
            "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447  test.tar.gz\n";
        assert!(verify_checksum(file.path(), sha_file, "test.tar.gz").is_ok());
        assert!(verify_checksum(file.path(), sha_file, "wrong.tar.gz").is_err());
    }
}

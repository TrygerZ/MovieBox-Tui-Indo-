use moviebox_tui::tui::player::{PlayerKind, command, supports_headers};
use std::process::Command;

fn get_cmd_args(cmd: &Command) -> Vec<String> {
    cmd.get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn test_vlc_win32_path_contract_never_emits_forward_slashes_for_local_subtitles() {
    let windows_drive_paths = [
        r"C:\Users\Default\AppData\Local\MovieBox-Tui\subs\Inception_a1b2c3d4.srt",
        "C:/Users/Default/AppData/Local/MovieBox-Tui/subs/Inception_a1b2c3d4.srt",
        r"D:\Media\Subtitles\Sub.ass",
        "D:/Media/Subtitles/Sub.ass",
    ];

    for path in windows_drive_paths {
        let cmd = command(
            PlayerKind::Vlc,
            "https://cdn.example.com/video.mp4",
            Some(path),
            &[],
            None,
            None,
            None,
            None,
        );
        let args = get_cmd_args(&cmd);
        let sub_arg = args
            .iter()
            .find(|a| a.starts_with("--sub-file="))
            .expect("VLC command must contain --sub-file argument");

        let sub_value = sub_arg.strip_prefix("--sub-file=").unwrap();
        assert!(
            !sub_value.contains('/'),
            "VLC Win32 contract violation: local Windows drive path must not contain forward slashes, got: {sub_value}"
        );
        assert!(
            sub_value.contains('\\'),
            "VLC Win32 contract violation: local Windows drive path must contain backslashes, got: {sub_value}"
        );
        assert!(
            sub_value.starts_with(r"C:\") || sub_value.starts_with(r"D:\"),
            "VLC Win32 contract violation: expected drive prefix, got: {sub_value}"
        );
    }
}

#[test]
fn test_vlc_unc_path_contract_preserves_network_share_backslashes() {
    let unc_paths = [
        r"\\nas\media\subtitles\movie.srt",
        "//nas/media/subtitles/movie.srt",
    ];

    for path in unc_paths {
        let cmd = command(
            PlayerKind::Vlc,
            "https://cdn.example.com/video.mp4",
            Some(path),
            &[],
            None,
            None,
            None,
            None,
        );
        let args = get_cmd_args(&cmd);
        let sub_arg = args
            .iter()
            .find(|a| a.starts_with("--sub-file="))
            .expect("VLC command must contain --sub-file argument");

        let sub_value = sub_arg.strip_prefix("--sub-file=").unwrap();
        assert!(
            sub_value.starts_with(r"\\"),
            "VLC UNC contract violation: path must start with \\\\, got: {sub_value}"
        );
        assert!(
            !sub_value[2..].contains('/'),
            "VLC UNC contract violation: path must not contain forward slashes, got: {sub_value}"
        );
    }
}

#[test]
fn test_vlc_http_remote_subtitle_contract_preserves_url_scheme() {
    let remote_urls = [
        "https://cdn.example.com/subtitles/movie.vtt?token=abc",
        "http://127.0.0.1:8080/sub/https%3A%2F%2Fcdn.example.com%2Fsub.srt",
    ];

    for url in remote_urls {
        let cmd = command(
            PlayerKind::Vlc,
            "https://cdn.example.com/video.mp4",
            Some(url),
            &[],
            None,
            None,
            None,
            None,
        );
        let args = get_cmd_args(&cmd);
        let sub_arg = args
            .iter()
            .find(|a| a.starts_with("--sub-file="))
            .expect("VLC command must contain --sub-file argument");

        let sub_value = sub_arg.strip_prefix("--sub-file=").unwrap();
        assert!(
            sub_value.starts_with("http://") || sub_value.starts_with("https://"),
            "VLC remote URL contract violation: URL scheme was altered, got: {sub_value}"
        );
        assert_eq!(
            sub_value, url,
            "VLC remote URL contract violation: URL was modified unexpectedly"
        );
    }
}

#[test]
fn test_iina_cli_contract_uses_plural_sub_files_and_filters_scripts() {
    let cmd = command(
        PlayerKind::Iina,
        "https://cdn.example.com/video.mp4",
        Some("/tmp/moviebox_subs/show_s01e01.vtt"),
        &[("Referer".to_string(), "https://example.com".to_string())],
        Some((1280, 720)),
        Some(120),
        Some(("moviebox", "subj_42", 1, 1)),
        Some(1080),
    );
    let args = get_cmd_args(&cmd);

    assert!(
        args.contains(&"--mpv-sub-files=/tmp/moviebox_subs/show_s01e01.vtt".to_string()),
        "IINA contract violation: iina-cli requires plural --mpv-sub-files, but argument was missing. Found args: {args:?}"
    );

    assert!(
        !args.iter().any(|a| a.starts_with("--mpv-sub-file=")),
        "IINA contract violation: singular alias --mpv-sub-file is rejected by iina-cli CLIParser.swift"
    );

    #[cfg(target_os = "macos")]
    assert!(
        !args.iter().any(|a| a.starts_with("--mpv-script=")),
        "IINA contract violation: external Lua tracker script is not executable via iina-cli CLI"
    );
}

#[test]
fn test_mpv_contract_uses_sub_file_and_sanitizes_script_opts() {
    let script_opts = moviebox_tui::tui::player::format_mpv_script_opts(
        "addons,malicious",
        "subj,evil=1",
        2,
        5,
        std::path::Path::new("/tmp/movie,state/test.json"),
    );

    assert!(
        !script_opts.contains("moviebox-provider=addons,malicious"),
        "mpv --script-opts contract violation: commas must be sanitized to prevent key-value option injection"
    );
    assert!(
        script_opts.contains("moviebox-provider=addons_malicious"),
        "mpv --script-opts contract violation: provider comma was not sanitized: {script_opts}"
    );
    assert!(
        script_opts.contains("moviebox-subject_id=subj_evil=1"),
        "mpv --script-opts contract violation: subject_id comma was not sanitized: {script_opts}"
    );
}

#[test]
fn test_vlc_single_instance_flag_contract_on_non_macos() {
    let cmd = command(
        PlayerKind::Vlc,
        "https://cdn.example.com/video.mp4",
        Some("/tmp/sub.srt"),
        &[("User-Agent".to_string(), "MovieBox".to_string())],
        None,
        Some(30),
        None,
        None,
    );
    let args = get_cmd_args(&cmd);

    #[cfg(not(target_os = "macos"))]
    assert!(
        args.contains(&"--no-one-instance".to_string()),
        "VLC contract violation on non-macOS: --no-one-instance required so running VLC windows do not strip subtitles or resume offset"
    );

    #[cfg(target_os = "macos")]
    assert!(
        !args.contains(&"--no-one-instance".to_string()),
        "VLC contract violation on macOS: VLC for macOS does not support --no-one-instance"
    );
}

#[test]
fn test_player_supports_headers_contract_matrix() {
    let dummy_headers = vec![("Authorization".to_string(), "Bearer secret".to_string())];

    assert!(
        supports_headers(PlayerKind::Mpv, &dummy_headers),
        "mpv must support custom headers"
    );
    assert!(
        supports_headers(PlayerKind::Iina, &dummy_headers),
        "IINA must support custom headers"
    );
    assert!(
        supports_headers(PlayerKind::Vlc, &dummy_headers),
        "VLC must support custom headers (via sidecar proxy or --http-*)"
    );
    assert!(
        supports_headers(PlayerKind::AndroidIntent, &dummy_headers),
        "AndroidIntent must support headers (via StreamRelay loopback)"
    );
}

#[test]
fn test_player_display_labels_contract() {
    assert_eq!(PlayerKind::Mpv.label(), "MPV");
    assert_eq!(PlayerKind::Iina.label(), "IINA");
    assert_eq!(PlayerKind::Vlc.label(), "VLC");
    assert_eq!(PlayerKind::AndroidIntent.label(), "Android Player");
}

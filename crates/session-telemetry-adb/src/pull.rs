/// Parses the line-per-path output of `adb shell run-as <package> find ...` into individual
/// paths — trims the trailing `\r` adb's shell output leaves on each line and drops empty
/// lines, but otherwise doesn't interpret the paths at all.
pub fn parse_find_output(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|line| line.trim_end_matches('\r').trim())
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Resolves `requested` — either the literal `"latest"` or a directory name under
/// `rnst-sessions/` — against the on-device session directories `parse_find_output` discovered,
/// returning the matching full path. Directory names are millisecond Unix timestamps (see
/// `SessionWriterModule.kt`'s `ensureOpen`), which are all the same digit count for a very long
/// time yet (into the year 2286), so comparing them as plain strings sorts them chronologically
/// without needing to actually parse them as numbers.
pub fn resolve_session_dir<'a>(requested: &str, available: &'a [String]) -> Option<&'a str> {
    if requested == "latest" {
        return available
            .iter()
            .max_by_key(|path| basename(path))
            .map(String::as_str);
    }
    available
        .iter()
        .find(|path| basename(path) == requested)
        .map(String::as_str)
}

/// Resolves the most recently sealed chunk in a session directory, for `watch` to tail. Chunk
/// filenames are a zero-padded, monotonically increasing index (`chunk-00000.rnst`,
/// `chunk-00001.rnst`, ...; see `write_chunk` in session-telemetry-android), so the
/// lexicographically greatest filename is always the most recent — no remote file timestamps
/// needed, same trick `resolve_session_dir` uses one level up.
pub fn resolve_latest_chunk_file(files: &[String]) -> Option<&str> {
    files
        .iter()
        .max_by_key(|path| basename(path))
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_find_output_trims_carriage_returns_and_drops_blank_lines() {
        let output = "files/rnst-sessions/1\r\nfiles/rnst-sessions/2\r\n\r\n";

        assert_eq!(
            parse_find_output(output),
            vec!["files/rnst-sessions/1", "files/rnst-sessions/2"]
        );
    }

    #[test]
    fn parse_find_output_returns_empty_for_no_matches() {
        assert_eq!(parse_find_output(""), Vec::<String>::new());
    }

    #[test]
    fn resolve_session_dir_picks_the_lexicographically_greatest_for_latest() {
        let available = vec![
            "files/rnst-sessions/1700000000000".to_string(),
            "files/rnst-sessions/1789277657612".to_string(),
            "files/rnst-sessions/1750000000000".to_string(),
        ];

        assert_eq!(
            resolve_session_dir("latest", &available),
            Some("files/rnst-sessions/1789277657612")
        );
    }

    #[test]
    fn resolve_session_dir_matches_an_exact_directory_name() {
        let available = vec![
            "files/rnst-sessions/1700000000000".to_string(),
            "files/rnst-sessions/1789277657612".to_string(),
        ];

        assert_eq!(
            resolve_session_dir("1700000000000", &available),
            Some("files/rnst-sessions/1700000000000")
        );
    }

    #[test]
    fn resolve_session_dir_returns_none_for_an_unknown_name() {
        let available = vec!["files/rnst-sessions/1700000000000".to_string()];

        assert_eq!(resolve_session_dir("not-a-real-session", &available), None);
    }

    #[test]
    fn resolve_session_dir_returns_none_for_latest_when_nothing_is_available() {
        assert_eq!(resolve_session_dir("latest", &[]), None);
    }

    #[test]
    fn resolve_latest_chunk_file_picks_the_highest_index() {
        let files = vec![
            "files/rnst-sessions/1789284955282/chunk-00000.rnst".to_string(),
            "files/rnst-sessions/1789284955282/chunk-00002.rnst".to_string(),
            "files/rnst-sessions/1789284955282/chunk-00001.rnst".to_string(),
        ];

        assert_eq!(
            resolve_latest_chunk_file(&files),
            Some("files/rnst-sessions/1789284955282/chunk-00002.rnst")
        );
    }

    #[test]
    fn resolve_latest_chunk_file_returns_none_when_empty() {
        assert_eq!(resolve_latest_chunk_file(&[]), None);
    }
}

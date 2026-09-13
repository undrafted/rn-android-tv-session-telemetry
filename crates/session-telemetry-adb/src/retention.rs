use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Given known sealed sessions (id + a recency timestamp) and how many to keep, returns the
/// ids to prune — the oldest ones beyond `keep_count`, retaining a bounded number of sealed
/// sessions for recovery. Pure decision logic, deliberately separated from real filesystem
/// I/O below so it's testable without relying on OS mtime resolution/timing, which is exactly
/// the kind of thing that makes tests flaky.
pub fn sessions_to_prune(sessions: &[(String, SystemTime)], keep_count: usize) -> Vec<&str> {
    let mut newest_first: Vec<&(String, SystemTime)> = sessions.iter().collect();
    newest_first.sort_by_key(|entry| std::cmp::Reverse(entry.1));

    newest_first
        .into_iter()
        .skip(keep_count)
        .map(|(id, _)| id.as_str())
        .collect()
}

/// Applies `sessions_to_prune` to real pulled-session directories under `sessions_dir` (each
/// entry's modified time as its recency indicator), deleting the ones beyond `keep_count` and
/// returning their paths. Not unit tested itself — see `sessions_to_prune` for the tested
/// decision logic; this is just the untested I/O boundary around it, same pattern as
/// `parse_devices_output` vs. actually invoking `adb`.
pub fn prune_sealed_sessions_dir(
    sessions_dir: &Path,
    keep_count: usize,
) -> io::Result<Vec<PathBuf>> {
    let mut sessions = Vec::new();
    let mut paths_by_id = std::collections::HashMap::new();

    for entry in fs::read_dir(sessions_dir)? {
        let entry = entry?;
        let path = entry.path();
        let modified = entry.metadata()?.modified()?;
        let id = entry.file_name().to_string_lossy().into_owned();
        paths_by_id.insert(id.clone(), path);
        sessions.push((id, modified));
    }

    let mut removed = Vec::new();
    for id in sessions_to_prune(&sessions, keep_count) {
        let Some(path) = paths_by_id.get(id) else {
            continue;
        };
        if path.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
        removed.push(path.clone());
    }

    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn keeps_everything_when_under_the_limit() {
        let sessions = vec![("a".to_string(), at(1)), ("b".to_string(), at(2))];

        assert_eq!(sessions_to_prune(&sessions, 5), Vec::<&str>::new());
    }

    #[test]
    fn keeps_everything_at_exactly_the_limit() {
        let sessions = vec![("a".to_string(), at(1)), ("b".to_string(), at(2))];

        assert_eq!(sessions_to_prune(&sessions, 2), Vec::<&str>::new());
    }

    #[test]
    fn prunes_the_oldest_sessions_beyond_the_limit() {
        let sessions = vec![
            ("oldest".to_string(), at(1)),
            ("middle".to_string(), at(2)),
            ("newest".to_string(), at(3)),
        ];

        assert_eq!(sessions_to_prune(&sessions, 2), vec!["oldest"]);
    }

    #[test]
    fn a_keep_count_of_zero_prunes_everything() {
        let sessions = vec![("a".to_string(), at(1)), ("b".to_string(), at(2))];

        let mut pruned = sessions_to_prune(&sessions, 0);
        pruned.sort_unstable();
        assert_eq!(pruned, vec!["a", "b"]);
    }
}

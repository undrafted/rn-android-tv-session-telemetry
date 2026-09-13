use clap::Parser;
use session_telemetry_adb::{
    DeviceState, parse_devices_output, parse_find_output, prune_sealed_sessions_dir,
    resolve_latest_chunk_file, resolve_session_dir,
};
use session_telemetry_analysis::{
    ClockMap, Finding, QaBookmark, build_interaction_windows, clock_sync_samples_from_events,
    create_bookmark, detect_excessive_commits_during_rapid_focus_movement,
    detect_high_latency_focus_changes, detect_high_latency_visible_updates,
    detect_js_stalls_overlapping_interactions, detect_network_completions_followed_by_commits,
    detect_react_commits_overlapping_delayed_frames, detect_repeated_network_requests,
    detect_repeated_redux_dispatches, detect_repeated_selector_recomputation,
    detect_unstable_selector_references,
};
use session_telemetry_cli::{
    Bookmark, BookmarkFile, Cli, Command, RecordMode, ReportFormat, SessionState, format_bookmarks,
    format_devices, format_event_loss, format_findings, format_mark_confirmation,
    format_record_started, format_status, format_stop_summary, format_watch_line, opener_command,
    resolve_latest_session_file,
};
use session_telemetry_report::{Report, SessionSummary, render_html, write_report_bundle};
use session_telemetry_session::Chunk;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, exit};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Doctor => run_doctor(),
        Command::Devices => run_devices(),
        Command::Record {
            mode,
            device,
            name,
            package,
        } => run_record(mode, device, name, package),
        Command::Stop => run_stop(),
        Command::Mark { label } => run_mark(&label),
        Command::Status => run_status(),
        Command::Watch { interval_secs } => run_watch(interval_secs),
        Command::Analyze { session } => run_analyze(&session),
        Command::Report {
            session,
            open,
            format,
        } => run_report(&session, open, format),
        Command::Pull {
            session,
            device,
            package,
            out,
            keep,
        } => run_pull(&session, &device, &package, out, keep),
    }
}

fn run_doctor() {
    match ProcessCommand::new("adb").arg("version").output() {
        Ok(output) if output.status.success() => println!("adb: found"),
        Ok(_) => println!("adb: found, but `adb version` exited non-zero"),
        Err(_) => println!("adb: not found on PATH"),
    }
}

fn run_devices() {
    match ProcessCommand::new("adb").args(["devices", "-l"]).output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("{}", format_devices(&parse_devices_output(&stdout)));
        }
        Ok(output) => {
            eprintln!(
                "adb exited with an error:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            exit(1);
        }
        Err(_) => {
            eprintln!("adb not found on PATH. Run `session-telemetry doctor` for details.");
            exit(1);
        }
    }
}

/// `~/.session-telemetry/active-session.json` — consistent regardless of which directory the
/// CLI is invoked from. Falls back to the current directory if `HOME` isn't set, so this still
/// works (just less conveniently) rather than panicking.
fn state_file_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".session-telemetry")
        .join("active-session.json")
}

fn load_session_state() -> Option<SessionState> {
    let contents = std::fs::read_to_string(state_file_path()).ok()?;
    SessionState::from_json(&contents).ok()
}

fn save_session_state(state: &SessionState) {
    let path = state_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = state.to_json() {
        let _ = std::fs::write(&path, json);
    }
}

fn clear_session_state() {
    let _ = std::fs::remove_file(state_file_path());
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// True if `adb devices -l` lists `device` as connected and authorized — `record` validates
/// this up front rather than writing session state for a device that was never reachable.
fn device_is_connected(device: &str) -> bool {
    let Ok(output) = ProcessCommand::new("adb").args(["devices", "-l"]).output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_devices_output(&stdout)
        .iter()
        .any(|info| info.serial == device && info.state == DeviceState::Device)
}

// Mirrored exactly in SessionWriterModule.kt's ACTION_START_SESSION/ACTION_STOP_SESSION - no
// shared-constant mechanism across Kotlin and Rust, so keep both sides in sync by hand.
const ACTION_START_SESSION: &str = "com.rnsessiontelemetry.reactnative.action.START_SESSION";
const ACTION_STOP_SESSION: &str = "com.rnsessiontelemetry.reactnative.action.STOP_SESSION";

/// Sends a session-control broadcast to `package` on `device`. Best-effort by design, not
/// just by accident: `record` is commonly run *before* the app has even launched, so there's
/// often nothing alive yet to receive it - the app's own autonomous start
/// (SessionTelemetry.install() on launch) covers that case regardless. Returns whether the
/// `adb` invocation itself succeeded, which only confirms the broadcast was sent, not that
/// anything was listening for it.
fn send_broadcast(device: &str, package: &str, action: &str) -> bool {
    ProcessCommand::new("adb")
        .args([
            "-s",
            device,
            "shell",
            "am",
            "broadcast",
            "-a",
            action,
            "-p",
            package,
        ])
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Reads the device's own wall clock (second precision) via `adb shell date +%s` and returns
/// its offset from the workstation's wall clock, in milliseconds (device minus workstation).
/// Captured once at `record` time so bookmark timestamps (workstation-side, see `run_mark`) can
/// be corrected onto the device's own wall-clock domain before being mapped onto the session
/// timeline — two machines with merely different system clocks (not necessarily wrong ones)
/// would otherwise silently place bookmarks at the wrong moment. `None` (not a fatal error) if
/// `adb` or the shell command fails, or its output doesn't parse — mapping then falls back to
/// assuming the two clocks already agree. Second precision (not `%3N` milliseconds) because
/// `date`'s exact flag support varies across Android/toybox versions; the ADB round-trip itself
/// already costs more latency than sub-second precision would recover.
fn device_clock_offset_ms(device: &str) -> Option<i64> {
    let workstation_now = now_unix_ms() as i64;
    let output = ProcessCommand::new("adb")
        .args(["-s", device, "shell", "date", "+%s"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let device_now_secs: i64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .ok()?;
    Some(device_now_secs * 1000 - workstation_now)
}

fn run_record(mode: RecordMode, device: String, name: String, package: String) {
    if load_session_state().is_some() {
        eprintln!(
            "A recording is already active. Run `session-telemetry stop` first, or `session-telemetry status` to see what's running."
        );
        exit(1);
    }
    if !device_is_connected(&device) {
        eprintln!(
            "Device {device} is not connected (or not authorized). Run `session-telemetry devices` to check."
        );
        exit(1);
    }

    send_broadcast(&device, &package, ACTION_START_SESSION);
    let device_clock_offset_ms = device_clock_offset_ms(&device);

    let state = SessionState {
        name,
        mode,
        device,
        package,
        started_at_unix_ms: now_unix_ms(),
        bookmarks: Vec::new(),
        device_clock_offset_ms,
    };
    save_session_state(&state);
    println!("{}", format_record_started(&state));
}

/// `~/.session-telemetry/bookmarks/<name>-<started_at_unix_ms>.json` — durable, unlike
/// `active-session.json` which `stop` deletes. Keyed by name + start time (not just name) so
/// re-recording under the same name doesn't silently overwrite an earlier run's bookmarks.
fn bookmarks_file_path(state: &SessionState) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".session-telemetry")
        .join("bookmarks")
        .join(format!("{}-{}.json", state.name, state.started_at_unix_ms))
}

/// The most recently modified file under `~/.session-telemetry/bookmarks/`, if any. `pull`
/// copies it alongside the chunks it just pulled as a best-effort association: only one
/// recording can be active at a time (`run_record` refuses to start a second one), so in the
/// normal record → mark → stop → pull workflow, the most recently stopped session's bookmarks
/// are exactly the ones the very next pull is for. There's no stronger link available — the CLI
/// never learns the on-device session directory name `stop` actually sealed, only `pull` does.
fn latest_bookmarks_file() -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home)
        .join(".session-telemetry")
        .join("bookmarks");
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .max_by_key(|entry| entry.metadata().and_then(|m| m.modified()).ok())
        .map(|entry| entry.path())
}

fn run_mark(label: &str) {
    let Some(mut state) = load_session_state() else {
        eprintln!("No active recording. Run `session-telemetry record` first.");
        exit(1);
    };
    state.bookmarks.push(Bookmark {
        label: label.to_string(),
        workstation_timestamp_unix_ms: now_unix_ms(),
    });
    let bookmark_count = state.bookmarks.len();
    save_session_state(&state);
    println!("{}", format_mark_confirmation(label, bookmark_count));
}

fn run_stop() {
    match load_session_state() {
        Some(state) => {
            send_broadcast(&state.device, &state.package, ACTION_STOP_SESSION);

            if !state.bookmarks.is_empty() {
                let path = bookmarks_file_path(&state);
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let file = BookmarkFile {
                    device_clock_offset_ms: state.device_clock_offset_ms,
                    bookmarks: state.bookmarks.clone(),
                };
                if let Ok(json) = file.to_json()
                    && std::fs::write(&path, json).is_ok()
                {
                    println!("Bookmarks saved to {}", path.display());
                }
            }

            println!("{}", format_stop_summary(&state, now_unix_ms()));
            clear_session_state();
        }
        None => {
            eprintln!("No active recording.");
            exit(1);
        }
    }
}

fn run_status() {
    println!(
        "{}",
        format_status(load_session_state().as_ref(), now_unix_ms())
    );
}

/// Tails the active recording's sealed chunks without waiting for `stop`/`pull` using a
/// read-only poll over the same `run-as find`/`cat` primitives `pull` already uses, rather
/// than a socket. Never touches the chunk currently being written (only ever
/// `resolve_latest_chunk_file`, which only sees sealed files), so a dropped `watch` process
/// or ADB connection can't affect the recording itself — completed chunks stay the recovery
/// source of truth regardless, same as `pull`.
fn run_watch(interval_secs: u64) {
    let Some(state) = load_session_state() else {
        eprintln!("No active recording. Run `session-telemetry record` first.");
        exit(1);
    };
    let interval = interval_secs.max(1);

    println!(
        "Watching \"{}\" on {} (polling every {interval}s, Ctrl-C to stop)...",
        state.name, state.device
    );

    let mut last_chunk_path: Option<String> = None;
    let mut printed_waiting = false;

    loop {
        if load_session_state().is_none() {
            println!("Recording stopped.");
            return;
        }

        let session_dirs = find_on_device(
            &state.device,
            &state.package,
            &[
                "files/rnst-sessions",
                "-mindepth",
                "1",
                "-maxdepth",
                "1",
                "-type",
                "d",
            ],
        );
        let latest_session_dir = resolve_session_dir("latest", &session_dirs).map(str::to_string);

        if let Some(session_dir) = latest_session_dir {
            let chunk_files = find_on_device(
                &state.device,
                &state.package,
                &[session_dir.as_str(), "-type", "f", "-name", "*.rnst"],
            );

            match resolve_latest_chunk_file(&chunk_files) {
                Some(latest_chunk) if last_chunk_path.as_deref() != Some(latest_chunk) => {
                    if let Some(bytes) = cat_on_device(&state.device, &state.package, latest_chunk)
                        && let Ok(chunk) = Chunk::decode(&bytes)
                    {
                        println!(
                            "{}",
                            format_watch_line(chunk_files.len(), Some(&chunk.manifest))
                        );
                    }
                    last_chunk_path = Some(latest_chunk.to_string());
                }
                None if !printed_waiting => {
                    println!("{}", format_watch_line(0, None));
                    printed_waiting = true;
                }
                _ => {}
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(interval));
    }
}

/// Runs `find` inside the target app's private storage via `adb shell run-as` — the mechanism
/// that actually works for a debuggable app's private files, unlike plain `adb pull` (which
/// runs as the `shell` user and can't read another app's `/data/user/0/<package>/...` without
/// root). Returns the parsed paths, or an empty list if the device/package/adb itself isn't
/// reachable — every call site treats "found nothing" and "couldn't ask" the same way.
fn find_on_device(device: &str, package: &str, find_args: &[&str]) -> Vec<String> {
    let mut args = vec!["-s", device, "shell", "run-as", package, "find"];
    args.extend(find_args);
    let Ok(output) = ProcessCommand::new("adb").args(&args).output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    parse_find_output(&String::from_utf8_lossy(&output.stdout))
}

/// Same `run-as` mechanism as `find_on_device`, but `cat`s one file's raw bytes back rather
/// than listing paths.
fn cat_on_device(device: &str, package: &str, remote_path: &str) -> Option<Vec<u8>> {
    let output = ProcessCommand::new("adb")
        .args(["-s", device, "shell", "run-as", package, "cat", remote_path])
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn run_pull(session: &str, device: &str, package: &str, out: Option<String>, keep: usize) {
    let session_dirs = find_on_device(
        device,
        package,
        &[
            "files/rnst-sessions",
            "-mindepth",
            "1",
            "-maxdepth",
            "1",
            "-type",
            "d",
        ],
    );
    if session_dirs.is_empty() {
        eprintln!(
            "No sessions found on {device} for {package}. Is a profiling build installed and recording?"
        );
        exit(1);
    }

    let Some(session_dir) = resolve_session_dir(session, &session_dirs) else {
        let available: Vec<&str> = session_dirs
            .iter()
            .map(|dir| dir.rsplit('/').next().unwrap_or(dir))
            .collect();
        eprintln!(
            "No session \"{session}\" found on {device} for {package}. Available: {}",
            available.join(", ")
        );
        exit(1);
    };
    let session_dir = session_dir.to_string();
    let session_name = session_dir.rsplit('/').next().unwrap_or(&session_dir);

    let remote_files = find_on_device(
        device,
        package,
        &[&session_dir, "-type", "f", "-name", "*.rnst"],
    );
    if remote_files.is_empty() {
        eprintln!("Session {session_name} has no sealed .rnst chunks yet.");
        exit(1);
    }

    let out_dir = out.unwrap_or_else(|| format!("pulled-sessions/{session_name}"));
    if let Err(err) = std::fs::create_dir_all(&out_dir) {
        eprintln!("could not create {out_dir}: {err}");
        exit(1);
    }

    let mut pulled_count = 0;
    for remote_file in &remote_files {
        let Some(bytes) = cat_on_device(device, package, remote_file) else {
            eprintln!("could not pull {remote_file}, skipping");
            continue;
        };
        let file_name = remote_file.rsplit('/').next().unwrap_or(remote_file);
        let local_path = PathBuf::from(&out_dir).join(file_name);
        if let Err(err) = std::fs::write(&local_path, bytes) {
            eprintln!("could not write {}: {err}", local_path.display());
            continue;
        }
        pulled_count += 1;
    }

    println!("Pulled {pulled_count} chunk(s) from {session_name} into {out_dir}/");

    if let Some(path) = latest_bookmarks_file()
        && let Ok(bytes) = std::fs::read(&path)
        && std::fs::write(Path::new(&out_dir).join("bookmarks.json"), bytes).is_ok()
    {
        println!(
            "Copied bookmarks from {} into {out_dir}/bookmarks.json",
            path.display()
        );
    }

    // Siblings of out_dir under the same parent - the default `pulled-sessions/<session>` shape
    // and a custom `--out` both generalize the same way, so pruning isn't tied to the default
    // location specifically.
    if let Some(sessions_dir) = Path::new(&out_dir).parent()
        && let Ok(removed) = prune_sealed_sessions_dir(sessions_dir, keep)
        && !removed.is_empty()
    {
        println!(
            "Pruned {} older pulled session(s), keeping the {keep} most recent.",
            removed.len()
        );
    }
}

const PULLED_SESSIONS_DIR: &str = "pulled-sessions";

/// Recursively collects every `.rnst` file under `dir` with its modification time — the
/// candidate pool `resolve_latest_session_file` picks from. Missing/unreadable entries are
/// skipped rather than failing the whole walk, since `PULLED_SESSIONS_DIR` may not exist yet
/// (nothing pulled) or a single file may be mid-write.
fn collect_rnst_files(dir: &Path, candidates: &mut Vec<(String, std::time::SystemTime)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rnst_files(&path, candidates);
            continue;
        }
        if path.extension().is_none_or(|ext| ext != "rnst") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        candidates.push((path.to_string_lossy().into_owned(), modified));
    }
}

/// Resolves `session` to an actual chunk file path: passed through unchanged unless it's the
/// literal string "latest", in which case it becomes the most recently modified `.rnst` file
/// under `./pulled-sessions/` (where `pull` writes by default).
fn resolve_session_path(session: &str) -> String {
    if session != "latest" {
        return session.to_string();
    }

    let mut candidates = Vec::new();
    collect_rnst_files(Path::new(PULLED_SESSIONS_DIR), &mut candidates);

    resolve_latest_session_file(&candidates)
        .map(str::to_string)
        .unwrap_or_else(|| {
            eprintln!(
                "No pulled sessions found under {PULLED_SESSIONS_DIR}/. Run `session-telemetry pull` first, or pass a direct file path."
            );
            exit(1);
        })
}

fn load_chunk(session: &str) -> Chunk {
    let bytes = std::fs::read(session).unwrap_or_else(|err| {
        eprintln!("could not read {session}: {err}");
        exit(1);
    });

    Chunk::decode(&bytes).unwrap_or_else(|err| {
        eprintln!("could not decode {session}: {err}");
        exit(1);
    })
}

fn collect_findings(chunk: &Chunk) -> Vec<Finding> {
    let windows = build_interaction_windows(&chunk.events);

    let mut findings = detect_high_latency_focus_changes(&windows);
    findings.extend(detect_high_latency_visible_updates(&windows));
    findings.extend(detect_repeated_redux_dispatches(&windows));
    findings.extend(detect_js_stalls_overlapping_interactions(&windows));
    findings.extend(detect_repeated_network_requests(&windows));
    findings.extend(detect_react_commits_overlapping_delayed_frames(&windows));
    findings.extend(detect_network_completions_followed_by_commits(&windows));
    findings.extend(detect_excessive_commits_during_rapid_focus_movement(
        &windows,
    ));
    findings.extend(detect_repeated_selector_recomputation(&windows));
    findings.extend(detect_unstable_selector_references(&windows));
    findings.sort_by_key(|finding| finding.sequence_start);
    findings
}

/// Looks for a `bookmarks.json` next to the resolved chunk file (see `run_pull`'s copy step)
/// and, if found, maps each bookmark onto this session's own monotonic timeline using clock-sync
/// samples decoded from the chunk itself, corrected by the device/workstation clock offset
/// captured at `record` time. Prints a warning (not a hard failure — the rest of analyze/report
/// still runs) when bookmarks exist but the chunk carries no clock-sync samples to map them
/// with, e.g. a very short or otherwise idle session that never triggered one.
fn load_mapped_bookmarks(session_path: &str, chunk: &Chunk) -> Vec<QaBookmark> {
    let Some(dir) = Path::new(session_path).parent() else {
        return Vec::new();
    };
    let Ok(json) = std::fs::read_to_string(dir.join("bookmarks.json")) else {
        return Vec::new();
    };
    let Ok(file) = BookmarkFile::from_json(&json) else {
        return Vec::new();
    };
    if file.bookmarks.is_empty() {
        return Vec::new();
    }

    let samples = clock_sync_samples_from_events(&chunk.events);
    let Some(clock_map) = ClockMap::from_samples(&samples) else {
        eprintln!(
            "{} bookmark(s) found but this session has no clock-sync samples to map them with - skipping.",
            file.bookmarks.len()
        );
        return Vec::new();
    };

    file.bookmarks
        .iter()
        .map(|bookmark| {
            let corrected = bookmark.workstation_timestamp_unix_ms as f64
                + file.device_clock_offset_ms.unwrap_or(0) as f64;
            create_bookmark(&clock_map, corrected, bookmark.label.clone())
        })
        .collect()
}

fn run_analyze(session: &str) {
    let session = resolve_session_path(session);
    let chunk = load_chunk(&session);
    println!("{}", format_findings(&collect_findings(&chunk)));
    println!("{}", format_event_loss(chunk.manifest.loss_count));

    let bookmarks = load_mapped_bookmarks(&session, &chunk);
    if !bookmarks.is_empty() {
        println!("{}", format_bookmarks(&bookmarks));
    }
}

fn run_report(session: &str, open: bool, format: ReportFormat) {
    let session = resolve_session_path(session);
    let chunk = load_chunk(&session);
    let mut summary = SessionSummary::from_events(&chunk.events);
    summary.loss_count = chunk.manifest.loss_count;
    let findings = collect_findings(&chunk);
    let bookmarks = load_mapped_bookmarks(&session, &chunk);
    let clock_uncertainty_ms =
        ClockMap::from_samples(&clock_sync_samples_from_events(&chunk.events))
            .map(|clock_map| clock_map.uncertainty_ms);

    let report = Report::new(
        &summary,
        &findings,
        &chunk.events,
        &bookmarks,
        clock_uncertainty_ms,
    );

    let json_path = format!("{session}.json");
    if let Err(err) = write_report_bundle(&report, Path::new(&json_path)) {
        eprintln!("could not write report bundle: {err}");
        exit(1);
    }
    println!("JSON report index written to {json_path}");
    let _ = format; // Both formats retain complete detail in paged JSON.

    let html_path = format!("{session}.html");
    let json_name = Path::new(&json_path).file_name().unwrap().to_string_lossy();
    let href: String = json_name
        .as_bytes()
        .iter()
        .map(|byte| format!("%{byte:02X}"))
        .collect();
    let html = render_html(&report).replacen("<h1>Session summary</h1>", &format!("<p><a href=\"{href}\">Complete JSON index and detail pages</a>. Keep the index and its pages directory together.</p><h1>Session summary</h1>"), 1);
    if let Err(err) = std::fs::write(&html_path, html) {
        eprintln!("could not write HTML report to {html_path}: {err}");
        exit(1);
    }
    println!("HTML report written to {html_path}");

    if open
        && let Err(err) = ProcessCommand::new(opener_command())
            .arg(&html_path)
            .status()
    {
        eprintln!("could not open {html_path}: {err}");
    }
}

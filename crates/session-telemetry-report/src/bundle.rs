use crate::Report;
use serde::Serialize;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const PAGE_SIZE: usize = 1000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    file: String,
    count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    sequence_start: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sequence_end: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp_start: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp_end: Option<f64>,
}

fn write_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let mut writer = BufWriter::new(File::create(path)?);
    serde_json::to_writer(&mut writer, value)?;
    writer.flush()
}

/// Writes each event once and publishes an index only after all pages are complete.
/// Generation directories keep an interrupted replacement from damaging an existing export.
/// Input events and analysis still reside in memory; this bounds export buffers, not capture loading.
pub fn write_report_bundle(report: &Report, index_path: &Path) -> io::Result<()> {
    let parent = index_path.parent().unwrap_or(Path::new("."));
    let file_name = index_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "report path needs a UTF-8 filename",
            )
        })?;
    let generation = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let directory = format!("{file_name}.pages-{}-{generation}", std::process::id());
    let directory_path = parent.join(&directory);
    fs::create_dir(&directory_path)?;
    let temporary_index = directory_path.join("index.tmp");
    let result = (|| {
        let mut events: Vec<_> = report.events.iter().collect();
        events.sort_by_key(|e| e.sequence());
        if events
            .windows(2)
            .any(|pair| pair[0].sequence() == pair[1].sequence())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate event sequence; evidence references would be ambiguous",
            ));
        }
        let mut event_pages = Vec::new();
        for (index, chunk) in events.chunks(PAGE_SIZE).enumerate() {
            let name = format!("events-{index:05}.json");
            write_json(&directory_path.join(&name), &chunk)?;
            event_pages.push(Page {
                file: format!("{directory}/{name}"),
                count: chunk.len(),
                sequence_start: chunk.first().map(|e| e.sequence()),
                sequence_end: chunk.last().map(|e| e.sequence()),
                timestamp_start: chunk
                    .iter()
                    .map(|e| e.timestamp())
                    .filter(|t| t.is_finite())
                    .reduce(f64::min),
                timestamp_end: chunk
                    .iter()
                    .map(|e| e.timestamp())
                    .filter(|t| t.is_finite())
                    .reduce(f64::max),
            });
        }
        fn pages<T: Serialize>(
            items: &[T],
            prefix: &str,
            parent: &Path,
            directory: &str,
        ) -> io::Result<Vec<Page>> {
            items
                .chunks(PAGE_SIZE)
                .enumerate()
                .map(|(index, chunk)| {
                    let name = format!("{prefix}-{index:05}.json");
                    write_json(&parent.join(&name), &chunk)?;
                    Ok(Page {
                        file: format!("{directory}/{name}"),
                        count: chunk.len(),
                        sequence_start: None,
                        sequence_end: None,
                        timestamp_start: None,
                        timestamp_end: None,
                    })
                })
                .collect()
        }
        let findings = pages(&report.findings, "findings", &directory_path, &directory)?;
        let interactions = pages(
            &report.react_summary.interactions,
            "interactions",
            &directory_path,
            &directory,
        )?;
        let selectors = pages(
            &report.selector_stats,
            "selectors",
            &directory_path,
            &directory,
        )?;
        let bookmarks = pages(report.bookmarks, "bookmarks", &directory_path, &directory)?;
        let profilers: Vec<_> = report
            .react_summary
            .by_profiler
            .iter()
            .map(|(id, stats)| serde_json::json!({"profilerId":id,"stats":stats}))
            .collect();
        let profilers = pages(&profilers, "profilers", &directory_path, &directory)?;
        let index = serde_json::json!({
            "summary": report.summary,
            "reactCommitCapture": report.react_commit_capture,
            "clockUncertaintyMs": report.clock_uncertainty_ms,
            "deviceMetadata": report.device_metadata,
            // Not paged, unlike findings/interactions/selectors: an application opens/closes a
            // resource-sampling window deliberately, never automatically, so even a multi-hour QA
            // session is expected to produce far fewer of these than events or interactions -
            // same "small, always fully included" treatment as deviceMetadata/reactCommitCapture
            // above.
            "resourceSamplingWindows": report.resource_sampling_windows,
            "evidenceLookup": "Findings reference inclusive sequenceStart/sequenceEnd ranges in event pages. Page paths are relative to this index. Event pages include timestamp bounds for time-window selection.",
            "pageSize": PAGE_SIZE,
            "counts": { "events": events.len(), "findings": report.findings.len(), "interactions": report.react_summary.interactions.len(), "selectors": report.selector_stats.len(), "bookmarks": report.bookmarks.len(), "profilers": report.react_summary.by_profiler.len(), "resourceSamplingWindows": report.resource_sampling_windows.len() },
            "pages": { "events":event_pages,"findings":findings,"interactions":interactions,"selectors":selectors,"bookmarks":bookmarks,"profilers":profilers }
        });
        write_json(&temporary_index, &index)?;
        fs::rename(&temporary_index, index_path)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&directory_path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SessionSummary;
    use session_telemetry_analysis::{Finding, Severity};
    use session_telemetry_protocol::{Event, RemoteInputEvent};

    #[test]
    fn pages_preserve_all_events_and_ranges_without_duplicate_evidence() {
        let dir = std::env::temp_dir().join(format!(
            "rnst-pages-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("report.json");
        let mut events: Vec<_> = (0..2005)
            .map(|sequence| {
                Event::RemoteInput(RemoteInputEvent {
                    sequence,
                    timestamp: sequence as f64 * 100.0,
                    key: "right".into(),
                })
            })
            .collect();
        events.push(Event::ReactCommit(
            session_telemetry_protocol::ReactCommitEvent {
                sequence: 2005,
                timestamp: 200500.0,
                profiler_id: "App".into(),
                phase: session_telemetry_protocol::ReactCommitPhase::Update,
                actual_duration_ms: 1.0,
                base_duration_ms: 1.0,
                render_start_ms: None,
                commit_time_ms: None,
            },
        ));
        let findings: Vec<_> = (0..1002)
            .map(|i| Finding {
                id: format!("overlap-{i}"),
                detector: "test",
                severity: Severity::Warning,
                sequence_start: 999,
                sequence_end: 2001,
                value: 1.0,
                unit: "events",
                thresholds: vec![],
                summary: "overlap".into(),
            })
            .collect();
        let summary = SessionSummary::from_events(&events);
        let report = Report::new(&summary, &findings, &events, &[], None);
        assert_eq!(report.findings[0].evidence.len(), 20);
        assert_eq!(report.findings[0].evidence_count, 1003);
        assert!(report.findings[100].evidence.is_empty());
        write_report_bundle(&report, &path).unwrap();
        let read = |path: &Path| -> serde_json::Value {
            serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
        };
        let index = read(&path);
        for (kind, expected) in [("events", 2006), ("findings", 1002), ("interactions", 2005)] {
            let mut count = 0;
            for page in index["pages"][kind].as_array().unwrap() {
                let contents = read(&dir.join(page["file"].as_str().unwrap()));
                let rows = contents.as_array().unwrap();
                assert!(rows.len() <= PAGE_SIZE);
                count += rows.len();
                if kind == "findings" {
                    assert!(rows.iter().all(|row| row.get("evidence").is_none()));
                }
            }
            assert_eq!(count, expected);
        }
        let pages = index["pages"]["events"].as_array().unwrap();
        assert_eq!(pages[1]["sequenceStart"], 1000);
        assert_eq!(pages[1]["timestampStart"], 100000.0);
        let mut selected = Vec::new();
        for page in pages {
            if page["sequenceEnd"].as_u64().unwrap() < 999
                || page["sequenceStart"].as_u64().unwrap() > 2001
            {
                continue;
            }
            selected.extend(
                read(&dir.join(page["file"].as_str().unwrap()))
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|e| (999..=2001).contains(&e["sequence"].as_u64().unwrap()))
                    .cloned(),
            );
        }
        assert_eq!(selected.len(), 1003);
        let html = crate::render_html(&report);
        assert!(html.contains("Showing 100 of 1002 findings"));
        assert!(html.contains("Showing 20 of 1003 evidence events"));
        assert!(html.contains("first 200 interactions"));
        let original = fs::read(&path).unwrap();
        let bad = vec![events[0].clone(), events[0].clone()];
        let bad_report = Report::new(&summary, &[], &bad, &[], None);
        assert!(write_report_bundle(&bad_report, &path).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn resource_sampling_windows_are_included_in_the_index_unpaged() {
        let dir = std::env::temp_dir().join(format!(
            "rnst-resource-sampling-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let events = vec![
            Event::ResourceSamplingStarted(
                session_telemetry_protocol::ResourceSamplingStartedEvent {
                    sequence: 0,
                    timestamp: 0.0,
                    interval_ms: 500.0,
                },
            ),
            Event::ResourceSample(session_telemetry_protocol::ResourceSampleEvent {
                sequence: 1,
                timestamp: 500.0,
                cpu_utilization_percent: 80.0,
                native_heap_kb: 1000,
                java_heap_kb: 1000,
            }),
            Event::ResourceSamplingStopped(
                session_telemetry_protocol::ResourceSamplingStoppedEvent {
                    sequence: 2,
                    timestamp: 1000.0,
                },
            ),
        ];
        let summary = SessionSummary::from_events(&events);
        let report = Report::new(&summary, &[], &events, &[], None);
        let path = dir.join("report.json");

        write_report_bundle(&report, &path).unwrap();

        let index: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(index["counts"]["resourceSamplingWindows"], 1);
        assert_eq!(
            index["resourceSamplingWindows"][0]["averageCpuUtilizationPercent"],
            80.0
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn empty_export_has_empty_page_lists() {
        let dir = std::env::temp_dir().join(format!(
            "rnst-empty-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let summary = SessionSummary::from_events(&[]);
        write_report_bundle(
            &Report::new(&summary, &[], &[], &[], None),
            &dir.join("report.json"),
        )
        .unwrap();
        let index: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("report.json")).unwrap()).unwrap();
        assert_eq!(index["counts"]["events"], 0);
        assert_eq!(index["pages"]["events"], serde_json::json!([]));
        fs::remove_dir_all(dir).unwrap();
    }
}

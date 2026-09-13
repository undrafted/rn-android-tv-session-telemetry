//! Synthetic 30-minute export stress case: ten events/second and overlapping findings.
//! Run with an output directory, then measure process RSS externally if needed.
use session_telemetry_analysis::{Finding, Severity};
use session_telemetry_protocol::{Event, InteractionMarkerEvent, RemoteInputEvent};
use session_telemetry_report::{Report, SessionSummary, render_html, write_report_bundle};
use std::{fs, path::PathBuf, time::Instant};

fn main() {
    let directory = PathBuf::from(std::env::args().nth(1).expect("output directory required"));
    fs::create_dir_all(&directory).unwrap();
    let events: Vec<_> = (0..18_000)
        .map(|sequence| {
            let timestamp = sequence as f64 * 100.0;
            if sequence % 10 == 0 {
                Event::RemoteInput(RemoteInputEvent {
                    sequence,
                    timestamp,
                    key: "right".into(),
                })
            } else {
                Event::InteractionMarker(InteractionMarkerEvent {
                    sequence,
                    timestamp,
                    name: "synthetic".into(),
                })
            }
        })
        .collect();
    // Deliberately extreme overlap to expose evidence duplication, not a typical finding rate.
    let findings: Vec<_> = (0..1800)
        .map(|i| Finding {
            id: format!("synthetic-{i}"),
            detector: "synthetic-overlap",
            severity: Severity::Warning,
            sequence_start: 0,
            sequence_end: 17999,
            value: 1.0,
            unit: "events",
            thresholds: vec![],
            summary: "Synthetic overlapping range".into(),
        })
        .collect();
    let previous_evidence_bytes = serde_json::to_vec(&events).unwrap().len() * findings.len();
    let start = Instant::now();
    let summary = SessionSummary::from_events(&events);
    let report = Report::new(&summary, &findings, &events, &[], None);
    write_report_bundle(&report, &directory.join("report.json")).unwrap();
    fs::write(directory.join("report.html"), render_html(&report)).unwrap();
    let index: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join("report.json")).unwrap()).unwrap();
    let page_bytes: u64 = index["pages"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|pages| pages.as_array().unwrap())
        .map(|page| {
            fs::metadata(directory.join(page["file"].as_str().unwrap()))
                .unwrap()
                .len()
        })
        .sum();
    let export_bytes = page_bytes
        + fs::metadata(directory.join("report.json")).unwrap().len()
        + fs::metadata(directory.join("report.html")).unwrap().len();
    println!(
        "{}",
        serde_json::json!({"events":events.len(),"findings":findings.len(),"interactions":report.react_summary.interactions.len(),"elapsedMs":start.elapsed().as_secs_f64()*1000.0,"exportBytes":export_bytes,"indexBytes":fs::metadata(directory.join("report.json")).unwrap().len(),"htmlBytes":fs::metadata(directory.join("report.html")).unwrap().len(),"previousRepeatedEvidenceBytesEstimate":previous_evidence_bytes})
    );
}

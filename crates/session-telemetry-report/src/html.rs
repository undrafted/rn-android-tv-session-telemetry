use crate::summary::SessionSummary;
use session_telemetry_analysis::{Finding, QaBookmark, Severity};
use session_telemetry_protocol::{Event, ReactCommitPhase};

/// Renders a static HTML report from a session summary, its findings, the session's raw events,
/// and any QA bookmarks mapped onto the session timeline. A long session's report must open
/// with a summary, not attempt to render every event at once — consistent with that, the events
/// are only ever used to render each finding's own evidence timeline (the events between its
/// `sequence_start`/`sequence_end`), never dumped in full. A finding's window is inherently
/// bounded (one remote-input interaction), so this stays bounded regardless of how long the
/// overall session was.
pub fn render_html(
    summary: &SessionSummary,
    findings: &[Finding],
    events: &[Event],
    bookmarks: &[QaBookmark],
) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>RN Session Telemetry Report</title>
<style>{CSS}</style>
</head>
<body>
<h1>Session summary</h1>
{summary_html}
<h1>Findings</h1>
{findings_html}
{bookmarks_html}
{timeline_html}
<script>{TIMELINE_FILTER_JS}</script>
</body>
</html>
"#,
        summary_html = render_summary(summary),
        findings_html = render_findings(findings, events),
        bookmarks_html = render_bookmarks(bookmarks),
        timeline_html = render_timeline(events),
    )
}

fn render_summary(summary: &SessionSummary) -> String {
    let duration = match summary.duration_ms() {
        Some(duration_ms) => format!("{duration_ms} ms"),
        None => "n/a".to_string(),
    };

    format!(
        "<dl class=\"summary\">\n\
         <dt>Events</dt><dd>{}</dd>\n\
         <dt>Remote input</dt><dd>{}</dd>\n\
         <dt>Focus changes</dt><dd>{}</dd>\n\
         <dt>Interaction markers</dt><dd>{}</dd>\n\
         <dt>Redux dispatches</dt><dd>{}</dd>\n\
         <dt>Network requests</dt><dd>{}</dd>\n\
         <dt>JS stalls</dt><dd>{}</dd>\n\
         <dt>React commits</dt><dd>{}</dd>\n\
         <dt>Delayed frames</dt><dd>{}</dd>\n\
         <dt>Duration</dt><dd>{duration}</dd>\n\
         </dl>",
        summary.event_count,
        summary.remote_input_count,
        summary.focus_count,
        summary.interaction_marker_count,
        summary.redux_dispatch_count,
        summary.network_count,
        summary.js_stall_count,
        summary.react_commit_count,
        summary.frame_timing_count,
    )
}

fn render_findings(findings: &[Finding], events: &[Event]) -> String {
    if findings.is_empty() {
        return "<p class=\"no-findings\">No findings.</p>".to_string();
    }

    let rows = findings
        .iter()
        .map(|finding| render_finding_rows(finding, events))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "<table class=\"findings\">\n\
         <thead><tr><th>Severity</th><th>Detector</th><th>Sequence range</th><th>Value</th></tr></thead>\n\
         <tbody>\n{rows}\n</tbody>\n\
         </table>"
    )
}

/// One finding is two table rows: the finding itself, then a full-width row underneath holding
/// its evidence timeline (empty and omitted if none of the passed `events` fall in its range —
/// e.g. when a report is rendered from just a `Finding` slice without the events that produced
/// it, as most of this file's own tests do).
fn render_finding_rows(finding: &Finding, events: &[Event]) -> String {
    let (severity_label, severity_class) = match finding.severity {
        Severity::Warning => ("warning", "warning"),
        Severity::Critical => ("critical", "critical"),
    };

    let finding_row = format!(
        "<tr class=\"{severity_class}\">\
         <td>{severity_label}</td>\
         <td>{} (v{})</td>\
         <td><a href=\"#event-{start}\">{start}–{end}</a></td>\
         <td>{} {}</td>\
         </tr>",
        finding.detector,
        finding.detector_version,
        finding.value,
        finding.unit,
        start = finding.sequence_start,
        end = finding.sequence_end,
    );

    let evidence = render_evidence_timeline(finding, events);
    if evidence.is_empty() {
        return finding_row;
    }

    format!("{finding_row}\n<tr class=\"evidence-row\"><td colspan=\"4\">{evidence}</td></tr>")
}

/// The events between a finding's `sequence_start` and `sequence_end`, rendered as an
/// elapsed-time list — the same shape as plan.md section 1's example trace ("4 ms Redux action:
/// catalog/itemFocused"). Elapsed time is relative to the window's own first event, not the
/// session start, so each finding's timeline reads on its own.
fn render_evidence_timeline(finding: &Finding, events: &[Event]) -> String {
    let mut window_events: Vec<&Event> = events
        .iter()
        .filter(|event| {
            let sequence = event.sequence();
            sequence >= finding.sequence_start && sequence <= finding.sequence_end
        })
        .collect();
    window_events.sort_by_key(|event| event.sequence());

    if window_events.is_empty() {
        return String::new();
    }

    let start_timestamp = window_events[0].timestamp();
    let rows = window_events
        .iter()
        .map(|event| {
            format!(
                "<li><span class=\"elapsed\">{:.0} ms</span> {}</li>",
                event.timestamp() - start_timestamp,
                describe_event(event)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("<ol class=\"evidence\">\n{rows}\n</ol>")
}

/// QA bookmarks (`session-telemetry mark`), already mapped onto the session's monotonic
/// timeline via `ClockMap` by the caller — this function only renders them. Omitted entirely
/// (no heading) when there are none, same as the findings table isn't forced to exist for an
/// empty session.
fn render_bookmarks(bookmarks: &[QaBookmark]) -> String {
    if bookmarks.is_empty() {
        return String::new();
    }

    let rows = bookmarks
        .iter()
        .map(|bookmark| {
            format!(
                "<li><span class=\"elapsed\">{:.0} ms</span> {}</li>",
                bookmark.session_timestamp,
                escape_html(&bookmark.label)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "<h1>QA bookmarks</h1>\n\
         <p class=\"bookmark-note\">Mapped from workstation timestamps onto the session's own \
         timeline via on-device clock-sync samples (and, if captured, a device/workstation clock \
         offset from record time) — treat placement as approximate, not frame-accurate.</p>\n\
         <ol class=\"evidence\">\n{rows}\n</ol>"
    )
}

/// A long QA capture could otherwise produce a report with tens of thousands of timeline rows —
/// this caps it and discloses the truncation rather than silently dropping the tail or letting
/// the file balloon unbounded.
const MAX_TIMELINE_EVENTS: usize = 5_000;

/// `(wire type tag, checkbox label)` for every event variant, in the order the filter checkboxes
/// render — the tag matches `describe_event`/the protocol's own `type` field exactly, so the
/// inline filter script (`TIMELINE_FILTER_JS`) can match a checkbox to its `<li data-type>` by
/// simple string equality.
const EVENT_TYPE_FILTERS: &[(&str, &str)] = &[
    ("remote-input", "Remote input"),
    ("focus", "Focus"),
    ("interaction-marker", "Marker"),
    ("redux-dispatch", "Redux dispatch"),
    ("network", "Network"),
    ("js-stall", "JS stall"),
    ("react-commit", "React commit"),
    ("frame-timing", "Delayed frame"),
    ("clock-sync", "Clock sync"),
];

fn event_type_tag(event: &Event) -> &'static str {
    match event {
        Event::RemoteInput(_) => "remote-input",
        Event::Focus(_) => "focus",
        Event::InteractionMarker(_) => "interaction-marker",
        Event::ReduxDispatch(_) => "redux-dispatch",
        Event::Network(_) => "network",
        Event::JsStall(_) => "js-stall",
        Event::ReactCommit(_) => "react-commit",
        Event::FrameTiming(_) => "frame-timing",
        Event::ClockSync(_) => "clock-sync",
    }
}

/// The whole session as one chronological, filterable list — behind a closed-by-default
/// `<details>` disclosure so a long session's report still *opens* with just the summary
/// (plan.md success criterion #13: don't render every long-session event at once) while the
/// complete trace stays one click away. Each finding's sequence-range link
/// (`render_finding_rows`) jumps straight to its first event here via `#event-{sequence}`.
/// Omitted entirely for an empty session, same as `render_bookmarks`.
fn render_timeline(events: &[Event]) -> String {
    if events.is_empty() {
        return String::new();
    }

    let mut sorted: Vec<&Event> = events.iter().collect();
    sorted.sort_by_key(|event| event.sequence());

    let total = sorted.len();
    let shown = &sorted[..total.min(MAX_TIMELINE_EVENTS)];
    let start_timestamp = shown[0].timestamp();

    let rows = shown
        .iter()
        .map(|event| {
            let sequence = event.sequence();
            let event_type = event_type_tag(event);
            format!(
                "<li id=\"event-{sequence}\" data-type=\"{event_type}\">\
                 <span class=\"elapsed\">{:.0} ms</span> {}</li>",
                event.timestamp() - start_timestamp,
                describe_event(event)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let filters = EVENT_TYPE_FILTERS
        .iter()
        .map(|(event_type, label)| {
            format!(
                "<label><input type=\"checkbox\" class=\"timeline-filter\" \
                 data-type=\"{event_type}\" checked> {label}</label>"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let truncation_note = if total > shown.len() {
        format!(
            "<p class=\"note\">Showing the first {} of {total} events.</p>",
            shown.len()
        )
    } else {
        String::new()
    };

    format!(
        "<details class=\"timeline\">\n\
         <summary>Session timeline ({total} events)</summary>\n\
         <div class=\"timeline-filters\">\n{filters}\n</div>\n\
         {truncation_note}\n\
         <ol id=\"timeline-list\" class=\"evidence\">\n{rows}\n</ol>\n\
         </details>"
    )
}

/// Vanilla JS, no dependencies — the report is a single static file meant to be opened directly
/// from disk, not served, so there's nothing to bundle. Toggles `hidden` on timeline rows based
/// on which `.timeline-filter` checkboxes are checked; a no-op (empty `NodeList`s) on a report
/// with no timeline.
const TIMELINE_FILTER_JS: &str = "
(function () {
  var checkboxes = document.querySelectorAll('.timeline-filter');
  function applyFilters() {
    var active = new Set();
    checkboxes.forEach(function (checkbox) {
      if (checkbox.checked) { active.add(checkbox.dataset.type); }
    });
    document.querySelectorAll('#timeline-list > li').forEach(function (row) {
      row.hidden = !active.has(row.dataset.type);
    });
  }
  checkboxes.forEach(function (checkbox) {
    checkbox.addEventListener('change', applyFilters);
  });
})();
";

/// Human-readable one-liner for one event, for the evidence timeline. Every free-form,
/// app-controlled string here (action types, URLs, focus target ids, marker names, profiler
/// ids) goes through `escape_html` — this is the first place this crate renders anything that
/// didn't originate from our own `&'static str` constants.
fn describe_event(event: &Event) -> String {
    match event {
        Event::RemoteInput(event) => format!("Remote input: {}", escape_html(&event.key)),
        Event::Focus(event) => format!("Focus: {}", escape_html(&event.target_id)),
        Event::InteractionMarker(event) => format!("Marker: {}", escape_html(&event.name)),
        Event::ReduxDispatch(event) => format!(
            "Redux dispatch: {} ({} ms)",
            escape_html(&event.action_type),
            event.duration_ms
        ),
        Event::Network(event) => format!(
            "{} {} → {} ({} ms)",
            escape_html(&event.method),
            escape_html(&event.url),
            event.status,
            event.duration_ms
        ),
        Event::JsStall(event) => format!("JS stall ({} ms)", event.duration_ms),
        Event::ReactCommit(event) => format!(
            "React commit: {} ({}, {} ms)",
            escape_html(&event.profiler_id),
            describe_phase(event.phase),
            event.actual_duration_ms
        ),
        Event::FrameTiming(event) => format!("Delayed frame ({} ms)", event.duration_ms),
        Event::ClockSync(_) => "Clock sync sample".to_string(),
    }
}

fn describe_phase(phase: ReactCommitPhase) -> &'static str {
    match phase {
        ReactCommitPhase::Mount => "mount",
        ReactCommitPhase::Update => "update",
        ReactCommitPhase::NestedUpdate => "nested update",
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

const CSS: &str = "
  body { font-family: system-ui, sans-serif; max-width: 720px; margin: 2rem auto; color: #1a1a1a; }
  dl.summary { display: grid; grid-template-columns: max-content 1fr; gap: 0.25rem 1rem; }
  dl.summary dt { font-weight: 600; }
  table.findings { border-collapse: collapse; width: 100%; }
  table.findings th, table.findings td { text-align: left; padding: 0.4rem 0.6rem; border-bottom: 1px solid #ddd; }
  tr.warning td:first-child { color: #a15c00; }
  tr.critical td:first-child { color: #b3261e; font-weight: 600; }
  tr.evidence-row td { padding: 0 0.6rem 0.75rem 0.6rem; border-bottom: 1px solid #ddd; }
  ol.evidence { margin: 0; padding-left: 1.25rem; color: #444; font-size: 0.9em; }
  ol.evidence .elapsed { display: inline-block; min-width: 4.5em; color: #777; font-variant-numeric: tabular-nums; }
  details.timeline { margin-top: 1.5rem; border: 1px solid #ddd; border-radius: 6px; padding: 0.75rem 1rem; }
  details.timeline summary { cursor: pointer; font-weight: 600; font-size: 1.3em; }
  details.timeline[open] summary { margin-bottom: 0.75rem; }
  .timeline-filters { display: flex; flex-wrap: wrap; gap: 0.4rem 1rem; margin-bottom: 0.75rem; font-size: 0.85em; color: #444; }
  .timeline-filters label { display: inline-flex; align-items: center; gap: 0.3rem; cursor: pointer; }
  #timeline-list li[hidden] { display: none; }
  #timeline-list li:target { background: #fff6d8; }
  p.note { color: #666; font-size: 0.85em; }
";

#[cfg(test)]
mod tests {
    use super::*;
    use session_telemetry_protocol::{
        FocusEvent, FrameTimingEvent, InteractionMarkerEvent, RemoteInputEvent,
    };

    #[test]
    fn renders_summary_numbers() {
        let events = vec![
            Event::RemoteInput(RemoteInputEvent {
                sequence: 0,
                timestamp: 0.0,
                key: "right".to_string(),
            }),
            Event::Focus(FocusEvent {
                sequence: 1,
                timestamp: 214.0,
                target_id: "card-2".to_string(),
                previous_target_id: None,
            }),
        ];
        let summary = SessionSummary::from_events(&events);

        let html = render_html(&summary, &[], &events, &[]);

        assert!(html.contains("<dt>Events</dt><dd>2</dd>"));
        assert!(html.contains("<dt>Duration</dt><dd>214 ms</dd>"));
    }

    #[test]
    fn renders_the_newer_signal_counts() {
        let events = vec![Event::FrameTiming(FrameTimingEvent {
            sequence: 0,
            timestamp: 0.0,
            duration_ms: 48.2,
        })];
        let summary = SessionSummary::from_events(&events);

        let html = render_html(&summary, &[], &events, &[]);

        assert!(html.contains("<dt>Delayed frames</dt><dd>1</dd>"));
        assert!(html.contains("<dt>Network requests</dt><dd>0</dd>"));
        assert!(html.contains("<dt>JS stalls</dt><dd>0</dd>"));
        assert!(html.contains("<dt>React commits</dt><dd>0</dd>"));
    }

    #[test]
    fn reports_duration_as_na_for_an_empty_session() {
        let html = render_html(&SessionSummary::from_events(&[]), &[], &[], &[]);

        assert!(html.contains("<dt>Duration</dt><dd>n/a</dd>"));
    }

    #[test]
    fn renders_no_findings_message_when_empty() {
        let html = render_html(&SessionSummary::from_events(&[]), &[], &[], &[]);

        assert!(html.contains("No findings."));
        assert!(!html.contains("<table"));
    }

    #[test]
    fn renders_a_finding_row_with_severity_class() {
        let findings = vec![Finding {
            detector: "high-latency-focus-change",
            detector_version: 1,
            severity: Severity::Warning,
            sequence_start: 3,
            sequence_end: 7,
            value: 214.0,
            unit: "ms",
        }];

        let html = render_html(&SessionSummary::from_events(&[]), &findings, &[], &[]);

        assert!(html.contains("tr class=\"warning\""));
        assert!(html.contains("high-latency-focus-change (v1)"));
        assert!(html.contains("3–7"));
        assert!(html.contains("214 ms"));
    }

    #[test]
    fn renders_a_findings_evidence_timeline_with_elapsed_time() {
        let events = vec![
            Event::RemoteInput(RemoteInputEvent {
                sequence: 3,
                timestamp: 500.0,
                key: "right".to_string(),
            }),
            Event::InteractionMarker(InteractionMarkerEvent {
                sequence: 4,
                timestamp: 504.0,
                name: "demo:card-select".to_string(),
            }),
            Event::Focus(FocusEvent {
                sequence: 5,
                timestamp: 714.0,
                target_id: "card-2".to_string(),
                previous_target_id: Some("card-1".to_string()),
            }),
        ];
        let findings = vec![Finding {
            detector: "high-latency-focus-change",
            detector_version: 1,
            severity: Severity::Warning,
            sequence_start: 3,
            sequence_end: 5,
            value: 214.0,
            unit: "ms",
        }];

        let html = render_html(
            &SessionSummary::from_events(&events),
            &findings,
            &events,
            &[],
        );

        assert!(html.contains("ol class=\"evidence\""));
        assert!(html.contains("0 ms</span> Remote input: right"));
        assert!(html.contains("4 ms</span> Marker: demo:card-select"));
        assert!(html.contains("214 ms</span> Focus: card-2"));
    }

    #[test]
    fn omits_the_evidence_row_when_no_events_fall_in_the_findings_range() {
        let findings = vec![Finding {
            detector: "high-latency-focus-change",
            detector_version: 1,
            severity: Severity::Warning,
            sequence_start: 3,
            sequence_end: 7,
            value: 214.0,
            unit: "ms",
        }];

        let html = render_html(&SessionSummary::from_events(&[]), &findings, &[], &[]);

        assert!(!html.contains("<tr class=\"evidence-row\""));
    }

    #[test]
    fn escapes_free_form_strings_in_the_evidence_timeline() {
        let events = vec![
            Event::RemoteInput(RemoteInputEvent {
                sequence: 0,
                timestamp: 0.0,
                key: "right".to_string(),
            }),
            Event::InteractionMarker(InteractionMarkerEvent {
                sequence: 1,
                timestamp: 1.0,
                name: "<script>alert(1)</script>".to_string(),
            }),
        ];
        let findings = vec![Finding {
            detector: "high-latency-focus-change",
            detector_version: 1,
            severity: Severity::Warning,
            sequence_start: 0,
            sequence_end: 1,
            value: 1.0,
            unit: "ms",
        }];

        let html = render_html(
            &SessionSummary::from_events(&events),
            &findings,
            &events,
            &[],
        );

        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    }

    #[test]
    fn omits_the_bookmarks_section_when_there_are_none() {
        let html = render_html(&SessionSummary::from_events(&[]), &[], &[], &[]);

        assert!(!html.contains("QA bookmarks"));
    }

    #[test]
    fn renders_mapped_bookmarks_with_their_session_timestamp_and_label() {
        let bookmarks = vec![QaBookmark {
            workstation_timestamp: 1_700_000_000_500.0,
            session_timestamp: 4_200.0,
            label: "carousel stopped responding".to_string(),
        }];

        let html = render_html(&SessionSummary::from_events(&[]), &[], &[], &bookmarks);

        assert!(html.contains("QA bookmarks"));
        assert!(html.contains("4200 ms</span> carousel stopped responding"));
    }

    #[test]
    fn omits_the_timeline_when_there_are_no_events() {
        let html = render_html(&SessionSummary::from_events(&[]), &[], &[], &[]);

        assert!(!html.contains("Session timeline"));
        assert!(!html.contains("<details"));
    }

    #[test]
    fn renders_the_full_session_as_a_filterable_timeline() {
        let events = vec![
            Event::RemoteInput(RemoteInputEvent {
                sequence: 0,
                timestamp: 0.0,
                key: "right".to_string(),
            }),
            Event::Focus(FocusEvent {
                sequence: 1,
                timestamp: 214.0,
                target_id: "card-2".to_string(),
                previous_target_id: None,
            }),
        ];

        let html = render_html(&SessionSummary::from_events(&events), &[], &events, &[]);

        assert!(html.contains("<summary>Session timeline (2 events)</summary>"));
        assert!(html.contains("id=\"event-0\" data-type=\"remote-input\""));
        assert!(html.contains("id=\"event-1\" data-type=\"focus\""));
        assert!(html.contains("class=\"timeline-filter\" data-type=\"remote-input\""));
        assert!(html.contains("0 ms</span> Remote input: right"));
        assert!(html.contains("214 ms</span> Focus: card-2"));
    }

    #[test]
    fn truncates_a_pathologically_long_session_and_discloses_it() {
        let events: Vec<Event> = (0..MAX_TIMELINE_EVENTS + 10)
            .map(|sequence| {
                Event::RemoteInput(RemoteInputEvent {
                    sequence: sequence as u64,
                    timestamp: sequence as f64,
                    key: "right".to_string(),
                })
            })
            .collect();

        let html = render_timeline(&events);

        assert!(html.contains(&format!(
            "Showing the first {MAX_TIMELINE_EVENTS} of {} events.",
            MAX_TIMELINE_EVENTS + 10
        )));
        assert!(html.contains(&format!("id=\"event-{}\"", MAX_TIMELINE_EVENTS - 1)));
        assert!(!html.contains(&format!("id=\"event-{MAX_TIMELINE_EVENTS}\"")));
    }

    #[test]
    fn links_a_findings_sequence_range_to_its_timeline_anchor() {
        let findings = vec![Finding {
            detector: "high-latency-focus-change",
            detector_version: 1,
            severity: Severity::Warning,
            sequence_start: 3,
            sequence_end: 7,
            value: 214.0,
            unit: "ms",
        }];

        let html = render_html(&SessionSummary::from_events(&[]), &findings, &[], &[]);

        assert!(html.contains("<a href=\"#event-3\">3–7</a>"));
    }

    #[test]
    fn escapes_free_form_strings_in_bookmark_labels() {
        let bookmarks = vec![QaBookmark {
            workstation_timestamp: 0.0,
            session_timestamp: 0.0,
            label: "<script>alert(1)</script>".to_string(),
        }];

        let html = render_html(&SessionSummary::from_events(&[]), &[], &[], &bookmarks);

        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    }
}

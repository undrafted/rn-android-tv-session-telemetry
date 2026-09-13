use crate::summary::SessionSummary;
use session_telemetry_analysis::{Finding, Severity};

/// Renders a static HTML report from a session summary and its findings. A long session's
/// report must open with a summary, not attempt to render every event at once — consistent
/// with that, this takes a `SessionSummary` and `Finding`s, not a raw event list, so there's no
/// way to accidentally dump a whole session into the page.
///
/// Every value rendered here is either a number or one of our own `&'static str` constants
/// (detector names, units) — nothing free-form/user-controlled goes into this HTML yet, so
/// there's no escaping helper. If a future version renders free-form strings (action types,
/// marker names, focus target ids), it needs one.
pub fn render_html(summary: &SessionSummary, findings: &[Finding]) -> String {
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
</body>
</html>
"#,
        summary_html = render_summary(summary),
        findings_html = render_findings(findings),
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
         <dt>Duration</dt><dd>{duration}</dd>\n\
         </dl>",
        summary.event_count,
        summary.remote_input_count,
        summary.focus_count,
        summary.interaction_marker_count,
        summary.redux_dispatch_count,
    )
}

fn render_findings(findings: &[Finding]) -> String {
    if findings.is_empty() {
        return "<p class=\"no-findings\">No findings.</p>".to_string();
    }

    let rows = findings
        .iter()
        .map(render_finding_row)
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "<table class=\"findings\">\n\
         <thead><tr><th>Severity</th><th>Detector</th><th>Sequence range</th><th>Value</th></tr></thead>\n\
         <tbody>\n{rows}\n</tbody>\n\
         </table>"
    )
}

fn render_finding_row(finding: &Finding) -> String {
    let (severity_label, severity_class) = match finding.severity {
        Severity::Warning => ("warning", "warning"),
        Severity::Critical => ("critical", "critical"),
    };

    format!(
        "<tr class=\"{severity_class}\">\
         <td>{severity_label}</td>\
         <td>{} (v{})</td>\
         <td>{}–{}</td>\
         <td>{} {}</td>\
         </tr>",
        finding.detector,
        finding.detector_version,
        finding.sequence_start,
        finding.sequence_end,
        finding.value,
        finding.unit,
    )
}

const CSS: &str = "
  body { font-family: system-ui, sans-serif; max-width: 720px; margin: 2rem auto; color: #1a1a1a; }
  dl.summary { display: grid; grid-template-columns: max-content 1fr; gap: 0.25rem 1rem; }
  dl.summary dt { font-weight: 600; }
  table.findings { border-collapse: collapse; width: 100%; }
  table.findings th, table.findings td { text-align: left; padding: 0.4rem 0.6rem; border-bottom: 1px solid #ddd; }
  tr.warning td:first-child { color: #a15c00; }
  tr.critical td:first-child { color: #b3261e; font-weight: 600; }
";

#[cfg(test)]
mod tests {
    use super::*;
    use session_telemetry_protocol::{Event, FocusEvent, RemoteInputEvent};

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

        let html = render_html(&summary, &[]);

        assert!(html.contains("<dt>Events</dt><dd>2</dd>"));
        assert!(html.contains("<dt>Duration</dt><dd>214 ms</dd>"));
    }

    #[test]
    fn reports_duration_as_na_for_an_empty_session() {
        let html = render_html(&SessionSummary::from_events(&[]), &[]);

        assert!(html.contains("<dt>Duration</dt><dd>n/a</dd>"));
    }

    #[test]
    fn renders_no_findings_message_when_empty() {
        let html = render_html(&SessionSummary::from_events(&[]), &[]);

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

        let html = render_html(&SessionSummary::from_events(&[]), &findings);

        assert!(html.contains("tr class=\"warning\""));
        assert!(html.contains("high-latency-focus-change (v1)"));
        assert!(html.contains("3–7"));
        assert!(html.contains("214 ms"));
    }
}

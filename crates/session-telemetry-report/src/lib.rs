//! JSON and HTML report generation from analyzed sessions.

mod html;
mod report;
mod summary;

pub use html::render_html;
pub use report::{FindingWithEvidence, REPORT_SCHEMA_VERSION, Report};
pub use summary::SessionSummary;

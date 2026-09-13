//! JSON and HTML report generation from analyzed sessions.

mod html;
mod react;
mod report;
mod summary;

pub use html::render_html;
pub use report::{FindingWithEvidence, Report};
pub use summary::SessionSummary;

pub use react::{ReactInteractionStats, ReactRenderStats, ReactSummary};

mod bundle;
pub use bundle::write_report_bundle;

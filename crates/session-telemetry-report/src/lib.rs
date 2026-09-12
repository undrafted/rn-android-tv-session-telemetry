//! JSON and HTML report generation from analyzed sessions.

mod html;
mod summary;

pub use html::render_html;
pub use summary::SessionSummary;

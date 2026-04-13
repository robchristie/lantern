pub mod cdp;
pub mod console;
pub mod dom;
pub mod endpoint;
pub mod navigation;
pub mod redaction;
pub mod screenshot;
pub mod wait;

pub const PROJECT_NAME: &str = "Lantern";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSummary {
    pub name: &'static str,
    pub summary: &'static str,
}

pub fn bootstrap_summary() -> ProjectSummary {
    ProjectSummary {
        name: PROJECT_NAME,
        summary: "Rust-first local CLI shim over Chromium CDP for agentic frontend development.",
    }
}

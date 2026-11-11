//! Deterministic native registration module discovery, shared with its contract tests.
use std::path::Path;

pub fn declarations(_directory: &Path) -> Result<String, String> {
    Ok(String::new())
}

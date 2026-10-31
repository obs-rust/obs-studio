//! obs-mcp tests, pulled into `obs-tests-core`.

mod framing;
mod outputs;
mod scenes;
mod server;
mod sources;

use obs_mcp::serve;
use serde_json::Value;

/// Feeds `input` lines to [`serve`] and returns the parsed output lines.
pub(crate) fn roundtrip(input: &str) -> Vec<Value> {
    let mut out = Vec::new();
    serve(input.as_bytes(), &mut out).expect("serve");
    String::from_utf8(out)
        .expect("utf8")
        .lines()
        .map(|l| serde_json::from_str(l).expect("output line is JSON"))
        .collect()
}

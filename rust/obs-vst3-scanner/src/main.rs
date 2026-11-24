use std::env;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cache = env::args_os().nth(1);
    obs_vst3_scanner::command(cache.as_deref().map(Path::new))
}

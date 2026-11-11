//! Host boundary for local control sockets.
use std::path::PathBuf;
#[derive(Clone, Debug, Default)]
pub struct EndpointOptions {
    pub socket: Option<PathBuf>,
}
std::cfg_select! {
 unix => { mod unix; pub use unix::*; }
 windows => { mod windows; pub use windows::*; }
 _ => { mod unsupported; pub use unsupported::*; }
}

//! Host boundary for local control sockets.
use std::{io, path::PathBuf};
#[derive(Clone, Debug, Default)]
pub struct EndpointOptions {
    pub socket: Option<PathBuf>,
}
/// Resolve an explicit endpoint or the per-user runtime endpoint.
/// # Errors
/// Fails if no runtime directory is configured.
pub fn endpoint(options: &EndpointOptions) -> io::Result<PathBuf> {
    options.socket.clone().map_or_else(
        || {
            std::env::var_os("XDG_RUNTIME_DIR")
                .map(|p| PathBuf::from(p).join("obs-studio-rust/control.sock"))
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        "XDG_RUNTIME_DIR is unset; provide a socket path",
                    )
                })
        },
        Ok,
    )
}
std::cfg_select! {
 unix => { mod unix; pub use unix::*; }
 _ => { mod unsupported; pub use unsupported::*; }
}

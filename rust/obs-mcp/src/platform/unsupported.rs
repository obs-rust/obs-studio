//! Windows named-pipe transport is tracked in obs-rust/obs-studio#4455.
use super::EndpointOptions;
use std::io;
pub struct Connection {
    pub reader: Box<dyn io::BufRead + Send>,
    pub writer: Box<dyn io::Write + Send>,
}
fn unavailable() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "local control transport unavailable; Windows named pipe: https://github.com/obs-rust/obs-studio/issues/4455",
    )
}
pub fn connect(_: &EndpointOptions) -> io::Result<Connection> {
    Err(unavailable())
}
pub fn proxy(_: &EndpointOptions) -> io::Result<()> {
    Err(unavailable())
}
#[must_use]
pub fn default_graphics_module() -> String {
    "libobs-d3d11".to_owned()
}

pub struct SocketServer;
impl SocketServer {
    pub fn bind(_: &EndpointOptions) -> io::Result<Self> {
        Err(unavailable())
    }
    pub fn run(
        self,
        _: &std::sync::Arc<std::sync::atomic::AtomicBool>,
        _: impl FnMut() -> crate::server::Server,
    ) -> io::Result<()> {
        Err(unavailable())
    }
}

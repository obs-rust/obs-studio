//! Host entry point: libobs lifecycle plus the stdio server (#3289).

use std::io;
use std::rc::Rc;

use crate::backend::ObsBackend;
use crate::ffi::{self, Libobs};

/// Starts a headless libobs and serves obs-mcp on stdin/stdout until stdin
/// reaches end of input, then shuts libobs down.
///
/// Descriptor 1 is pointed at stderr first, so libobs logs and anything else
/// that prints cannot corrupt the protocol stream.
///
/// # Errors
///
/// Returns an error when libobs fails to start or stdio fails.
pub fn serve_stdio(record: Option<&str>, stream: Option<&str>) -> io::Result<()> {
    let mut protocol = ffi::claim_stdout()?;
    // Declared first, so it is dropped last: after the server and the backend.
    let _libobs = Libobs::start().map_err(io::Error::other)?;
    let backend = Rc::new(ObsBackend::new().with_output_names(record, stream));
    let mut server = ObsBackend::server(&backend);
    server.serve(io::stdin().lock(), &mut protocol)
}

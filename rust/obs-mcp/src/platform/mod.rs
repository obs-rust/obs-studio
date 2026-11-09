//! Owner-only local control transport.
use std::{io, path::PathBuf};
#[derive(Clone, Debug, Default)]
pub struct EndpointOptions { pub socket: Option<PathBuf> }
pub fn endpoint(options: &EndpointOptions) -> io::Result<PathBuf> { todo!("endpoint: {options:?}") }

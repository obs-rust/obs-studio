//! Synchronous typed local control client.
use crate::{platform::Connection, server::ToolError, tools::control::{StartInput,VcamStatus}};
#[derive(Debug)]
pub enum Error { Io(std::io::Error), Protocol(String), Domain(ToolError) }
pub struct Client;
impl Client {
    /// # Errors
    /// Fails when the peer cannot initialize.
    pub fn from_connection(_: Connection)->Result<Self,Error>{todo!("client handshake")}
    #[must_use] pub fn vcam(&self)->Vcam<'_>{Vcam(self)}
}
pub struct Vcam<'a>(&'a Client);
impl Vcam<'_>{
    /// # Errors
    /// Returns the peer's structured domain failure.
    pub fn start(&self,_:StartInput)->Result<VcamStatus,Error>{let _=self.0;todo!("typed vcam start")}
}

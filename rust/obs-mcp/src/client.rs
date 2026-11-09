//! Synchronous typed client for an existing OBS control socket.
pub use crate::tools::control::{MediaInput, StartInput, StatsSnapshot, VcamStatus, VmicStatus};
pub use crate::tools::waiting::Condition;
use crate::{
    platform::{self, Connection, EndpointOptions},
    server::ToolError,
    tools::{self, EmptyInput as Empty},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fmt,
    io::{self, Write},
    sync::Mutex,
};
/// Connection, protocol or structured domain failure.
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Protocol(String),
    Domain(ToolError),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Protocol(e) => f.write_str(e),
            Self::Domain(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
fn protocol(e: impl fmt::Display) -> Error {
    Error::Protocol(e.to_string())
}
struct State {
    connection: Connection,
    next_id: u64,
}
impl State {
    fn request(&mut self, method: &str, params: Value) -> Result<Value, Error> {
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| protocol("request ID exhausted"))?;
        let id = self.next_id;
        serde_json::to_writer(
            &mut self.connection.writer,
            &json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
        )
        .map_err(protocol)?;
        self.connection.writer.write_all(b"\n")?;
        self.connection.writer.flush()?;
        let mut line = Vec::new();
        // Bound reads even for a peer that never sends a newline.
        loop {
            let buffer = self.connection.reader.fill_buf()?;
            if buffer.is_empty() {
                return Err(protocol("control socket closed before response"));
            }
            let count = buffer
                .iter()
                .position(|b| *b == b'\n')
                .map_or(buffer.len(), |n| n + 1);
            if line.len().saturating_add(count) > 32 * 1024 * 1024 {
                return Err(protocol("control response exceeds 32 MiB"));
            }
            line.extend_from_slice(&buffer[..count]);
            self.connection.reader.consume(count);
            if line.last() == Some(&b'\n') {
                break;
            }
        }
        let response: Value = serde_json::from_slice(&line).map_err(protocol)?;
        if response["jsonrpc"] != "2.0" || response["id"] != id {
            return Err(protocol("invalid JSON-RPC version or response ID"));
        }
        if let Some(error) = response.get("error") {
            return Err(protocol(format!("JSON-RPC error: {error}")));
        }
        response
            .get("result")
            .cloned()
            .ok_or_else(|| protocol("response has no result"))
    }
}
/// Fluent, synchronous client. A mutex serializes requests and response IDs.
pub struct Client {
    state: Mutex<State>,
}
impl Client {
    /// Connect to an existing instance. This never spawns OBS.
    /// # Errors
    /// Returns endpoint, handshake or protocol failures.
    pub fn attach(options: EndpointOptions) -> Result<Self, Error> {
        Self::from_connection(platform::connect(&options)?)
    }
    /// Initialize an already-connected byte stream (also useful for embedded peers).
    /// # Errors
    /// Returns handshake or I/O failures.
    pub fn from_connection(connection: Connection) -> Result<Self, Error> {
        let mut state = State {
            connection,
            next_id: 0,
        };
        state.request("initialize",json!({"protocolVersion":crate::server::PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"obs-mcp-rust","version":env!("CARGO_PKG_VERSION")}}))?;
        serde_json::to_writer(
            &mut state.connection.writer,
            &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        )
        .map_err(protocol)?;
        state.connection.writer.write_all(b"\n")?;
        state.connection.writer.flush()?;
        Ok(Self {
            state: Mutex::new(state),
        })
    }
    fn call<I: Serialize, O: DeserializeOwned>(&self, name: &str, input: I) -> Result<O, Error> {
        let params =
            json!({"name":name,"arguments":serde_json::to_value(input).map_err(protocol)?});
        let result = self
            .state
            .lock()
            .map_err(protocol)?
            .request("tools/call", params)?;
        let content = result
            .get("structuredContent")
            .ok_or_else(|| protocol("tool response has no structuredContent"))?;
        if result["isError"] == true {
            return Err(Error::Domain(
                serde_json::from_value(content.clone()).map_err(protocol)?,
            ));
        }
        serde_json::from_value(content.clone()).map_err(protocol)
    }
    #[must_use]
    pub fn vcam(&self) -> Vcam<'_> {
        Vcam(self)
    }
    #[must_use]
    pub fn vmic(&self) -> Vmic<'_> {
        Vmic(self)
    }
    #[must_use]
    pub fn media(&self) -> Media<'_> {
        Media(self)
    }
    #[must_use]
    pub fn stats(&self) -> Stats<'_> {
        Stats(self)
    }
    /// Build a typed wait with the default five-second deadline.
    #[must_use]
    pub fn wait_for(&self, condition: tools::waiting::Condition) -> Wait<'_> {
        Wait {
            client: self,
            input: tools::waiting::WaitInput {
                condition,
                timeout_ms: tools::waiting::DEFAULT_TIMEOUT_MS,
            },
        }
    }
    /// Read the complete control pipeline.
    /// # Errors
    /// Returns a peer or domain failure.
    pub fn pipeline_dump(&self) -> Result<tools::control::PipelineDump, Error> {
        self.call("pipeline_dump", Empty {})
    }
    /// A no-side-effect transport probe.
    /// # Errors
    /// Returns a protocol or I/O failure.
    pub fn ping(&self) -> Result<(), Error> {
        self.state
            .lock()
            .map_err(protocol)?
            .request("ping", json!({}))?;
        Ok(())
    }
}
pub struct Vcam<'a>(&'a Client);
impl Vcam<'_> {
    /// # Errors
    /// Returns a peer or domain failure.
    pub fn start(
        &self,
        input: tools::control::StartInput,
    ) -> Result<tools::control::VcamStatus, Error> {
        self.0.call("vcam_start", input)
    }
    /// # Errors
    /// Returns a peer or domain failure.
    pub fn stop(&self) -> Result<tools::control::VcamStatus, Error> {
        self.0.call("vcam_stop", Empty {})
    }
    /// # Errors
    /// Returns a peer or domain failure.
    pub fn status(&self) -> Result<tools::control::VcamStatus, Error> {
        self.0.call("vcam_status", Empty {})
    }
}
pub struct Vmic<'a>(&'a Client);
impl Vmic<'_> {
    /// # Errors
    /// Returns a peer or domain failure.
    pub fn status(&self) -> Result<tools::control::VmicStatus, Error> {
        self.0.call("vmic_status", Empty {})
    }
}
pub struct Media<'a>(&'a Client);
impl Media<'_> {
    /// # Errors
    /// Returns a peer or domain failure.
    pub fn restart(
        &self,
        input: tools::control::MediaInput,
    ) -> Result<tools::control::MediaStatus, Error> {
        self.0.call("media_restart", input)
    }
    /// # Errors
    /// Returns a peer or domain failure.
    pub fn status(
        &self,
        input: tools::control::MediaInput,
    ) -> Result<tools::control::MediaStatus, Error> {
        self.0.call("media_status", input)
    }
}
pub struct Stats<'a>(&'a Client);
impl Stats<'_> {
    /// # Errors
    /// Returns a peer or domain failure.
    pub fn get(&self) -> Result<tools::control::StatsSnapshot, Error> {
        self.0.call("stats_get", Empty {})
    }
    /// # Errors
    /// Returns a peer or domain failure.
    pub fn reset(&self) -> Result<tools::control::StatsSnapshot, Error> {
        self.0.call("stats_reset", Empty {})
    }
}
pub struct Wait<'a> {
    client: &'a Client,
    input: tools::waiting::WaitInput,
}
impl Wait<'_> {
    #[must_use]
    pub fn timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.input.timeout_ms = timeout_ms;
        self
    }
    /// # Errors
    /// Returns a peer or domain failure. Timeout is a normal unsatisfied response.
    pub fn call(self) -> Result<tools::waiting::WaitOutput, Error> {
        self.client.call("wait_for", self.input)
    }
}

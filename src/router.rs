use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::schema::Message;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterSocket {
    path: PathBuf,
}

impl RouterSocket {
    pub fn from_environment() -> Option<Self> {
        std::env::var_os("PERSONA_ROUTER_SOCKET").map(Self::from_path)
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn client(&self) -> MessageRouterClient {
        MessageRouterClient {
            socket: self.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRouterClient {
    socket: RouterSocket,
}

impl MessageRouterClient {
    pub fn route(&self, message: &Message) -> Result<RouterReply> {
        let mut stream = UnixStream::connect(&self.socket.path)?;
        let input = RouterInput::from_message(message)?;
        writeln!(stream, "{}", input.as_str())?;
        stream.flush()?;

        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line)?;
        if line.trim().is_empty() {
            return Err(Error::RouterResponseEmpty);
        }
        Ok(RouterReply::from_text(line.trim().to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RouterInput {
    text: String,
}

impl RouterInput {
    fn from_message(message: &Message) -> Result<Self> {
        Ok(Self {
            text: format!("(RouteMessage {})", message.to_nota()?),
        })
    }

    fn as_str(&self) -> &str {
        self.text.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterReply {
    text: String,
}

impl RouterReply {
    pub fn from_text(text: String) -> Self {
        Self { text }
    }

    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }
}

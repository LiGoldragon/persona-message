use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use signal_core::{FrameBody, Request};
use signal_persona_message::{Frame, MessageReply, MessageRequest};

use crate::error::{Error, Result};
use crate::schema::Message;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalRouterSocket {
    path: PathBuf,
}

impl SignalRouterSocket {
    pub fn from_environment() -> Option<Self> {
        std::env::var_os("PERSONA_MESSAGE_ROUTER_SOCKET").map(Self::from_path)
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn client(&self) -> SignalRouterClient {
        SignalRouterClient::from_socket(self.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalRouterClient {
    socket: SignalRouterSocket,
    codec: SignalRouterFrameCodec,
}

impl SignalRouterClient {
    pub fn from_socket(socket: SignalRouterSocket) -> Self {
        Self {
            socket,
            codec: SignalRouterFrameCodec::default(),
        }
    }

    pub fn submit(&self, request: MessageRequest) -> Result<MessageReply> {
        let mut stream = UnixStream::connect(&self.socket.path)?;
        let frame = self.codec.request_frame(request);
        self.codec.write_frame(&mut stream, &frame)?;
        let reply = self.codec.read_frame(&mut stream)?;
        self.codec.reply_from_frame(reply)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalRouterFrameCodec {
    maximum_frame_bytes: usize,
}

impl SignalRouterFrameCodec {
    pub const fn new(maximum_frame_bytes: usize) -> Self {
        Self {
            maximum_frame_bytes,
        }
    }

    pub fn read_frame(&self, stream: &mut UnixStream) -> Result<Frame> {
        let mut prefix = [0_u8; 4];
        stream.read_exact(&mut prefix)?;
        let length = u32::from_be_bytes(prefix) as usize;
        if length > self.maximum_frame_bytes {
            return Err(Error::DaemonFrameTooLarge { bytes: length });
        }
        let mut bytes = Vec::with_capacity(4 + length);
        bytes.extend_from_slice(&prefix);
        bytes.resize(4 + length, 0);
        stream.read_exact(&mut bytes[4..])?;
        Ok(Frame::decode_length_prefixed(&bytes)?)
    }

    pub fn write_frame(&self, stream: &mut UnixStream, frame: &Frame) -> Result<()> {
        let bytes = frame.encode_length_prefixed()?;
        stream.write_all(&bytes)?;
        stream.flush()?;
        Ok(())
    }

    pub fn request_frame(&self, request: MessageRequest) -> Frame {
        Frame::new(FrameBody::Request(Request::assert(request)))
    }

    pub fn reply_from_frame(&self, frame: Frame) -> Result<MessageReply> {
        match frame.into_body() {
            FrameBody::Reply(signal_core::Reply::Operation(reply)) => Ok(reply),
            other => Err(Error::UnexpectedRouterReply {
                got: format!("{other:?}"),
            }),
        }
    }
}

impl Default for SignalRouterFrameCodec {
    fn default() -> Self {
        Self::new(1024 * 1024)
    }
}

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

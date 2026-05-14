use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use signal_core::{FrameBody, Request};
use signal_persona_message::{Frame, MessageReply, MessageRequest};

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalRouterSocket {
    path: PathBuf,
}

impl SignalRouterSocket {
    pub fn from_environment() -> Option<Self> {
        std::env::var_os("PERSONA_MESSAGE_ROUTER_SOCKET").map(Self::from_path)
    }

    pub fn from_peer_environment() -> Option<Self> {
        PeerSocketEnvironment::from_environment().router_socket()
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn client(&self) -> SignalRouterClient {
        SignalRouterClient::from_socket(self.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalMessageSocket {
    path: PathBuf,
}

impl SignalMessageSocket {
    pub fn from_environment() -> Option<Self> {
        std::env::var_os("PERSONA_MESSAGE_SOCKET")
            .or_else(|| std::env::var_os("PERSONA_SOCKET_PATH"))
            .map(Self::from_path)
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn client(&self) -> SignalMessageClient {
        SignalMessageClient::from_socket(self.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalMessageClient {
    socket: SignalMessageSocket,
    codec: SignalRouterFrameCodec,
}

impl SignalMessageClient {
    pub fn from_socket(socket: SignalMessageSocket) -> Self {
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

    pub fn read_frame(&self, stream: &mut impl Read) -> Result<Frame> {
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
        Frame::new(FrameBody::Request(Request::from_payload(request)))
    }

    pub fn request_from_frame(&self, frame: Frame) -> Result<MessageRequest> {
        match frame.into_body() {
            FrameBody::Request(request) => {
                request
                    .into_payload_checked()
                    .map_err(|error| Error::UnexpectedDaemonInput {
                        got: error.to_string(),
                    })
            }
            other => Err(Error::UnexpectedDaemonInput {
                got: format!("{other:?}"),
            }),
        }
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
pub struct PeerSocketEnvironment {
    peers: Vec<PeerSocket>,
}

impl PeerSocketEnvironment {
    pub fn from_environment() -> Self {
        let count = std::env::var("PERSONA_PEER_SOCKET_COUNT")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        let peers = (0..count)
            .filter_map(|index| PeerSocket::from_environment(index))
            .collect();
        Self { peers }
    }

    pub fn router_socket(&self) -> Option<SignalRouterSocket> {
        self.peers
            .iter()
            .find(|peer| peer.component == "router")
            .map(|peer| SignalRouterSocket::from_path(peer.socket_path.clone()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerSocket {
    component: String,
    socket_path: PathBuf,
}

impl PeerSocket {
    fn from_environment(index: usize) -> Option<Self> {
        let component = std::env::var(format!("PERSONA_PEER_{index}_COMPONENT")).ok()?;
        let socket_path = std::env::var_os(format!("PERSONA_PEER_{index}_SOCKET_PATH"))?;
        Some(Self {
            component,
            socket_path: PathBuf::from(socket_path),
        })
    }
}

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use nota_codec::{Decoder, Encoder, NotaDecode, NotaEncode, NotaRecord};

use crate::command::{Accepted, InboxMessages, Input, Output, Send};
use crate::error::{Error, Result};
use crate::resolver::ProcessAncestry;
use crate::schema::{ActorId, expect_end};
use crate::store::{MessageStore, StorePath};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonSocket {
    path: PathBuf,
}

impl DaemonSocket {
    pub fn from_environment() -> Option<Self> {
        std::env::var_os("PERSONA_MESSAGE_DAEMON").map(Self::from_path)
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }
}

#[derive(Debug, Clone)]
pub struct MessageDaemon {
    socket: DaemonSocket,
    store: MessageStore,
}

impl MessageDaemon {
    pub fn from_socket(socket: DaemonSocket, store: MessageStore) -> Self {
        Self { socket, store }
    }

    pub fn run(&self) -> Result<()> {
        if let Some(parent) = self.socket.path().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let _ = std::fs::remove_file(self.socket.path());
        let listener = UnixListener::bind(self.socket.path())?;
        for stream in listener.incoming() {
            let stream = stream?;
            self.handle_stream(stream)?;
        }
        Ok(())
    }

    fn handle_stream(&self, stream: UnixStream) -> Result<()> {
        let reader = stream.try_clone()?;
        let mut reader = BufReader::new(reader);
        let mut request = String::new();
        reader.read_line(&mut request)?;
        let response = DaemonRequest::from_nota(&request)?.execute(&self.store)?;
        let mut writer = stream;
        writeln!(writer, "{}", response.to_nota()?)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDaemonClient {
    socket: DaemonSocket,
}

impl MessageDaemonClient {
    pub fn from_socket(socket: DaemonSocket) -> Self {
        Self { socket }
    }

    pub fn submit(&self, input: Input) -> Result<String> {
        let request = DaemonRequest::from_input(std::process::id(), input);
        let mut stream = UnixStream::connect(self.socket.path())?;
        writeln!(stream, "{}", request.to_nota()?)?;
        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader.read_line(&mut response)?;
        if response.trim().is_empty() {
            return Err(Error::InvalidDaemonResponse { got: response });
        }
        Ok(response)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonRequest {
    Send(DaemonSend),
    Inbox(DaemonInbox),
}

impl DaemonRequest {
    pub fn from_input(pid: u32, input: Input) -> Self {
        match input {
            Input::Send(send) => Self::Send(DaemonSend {
                pid,
                recipient: send.recipient,
                body: send.body,
            }),
            Input::Inbox(inbox) => Self::Inbox(DaemonInbox {
                recipient: inbox.recipient,
            }),
            Input::Tail(_) => Self::Inbox(DaemonInbox {
                recipient: ActorId::new("operator"),
            }),
        }
    }

    pub fn from_nota(text: &str) -> Result<Self> {
        let mut decoder = Decoder::nota(text);
        let request = Self::decode(&mut decoder)?;
        expect_end(&mut decoder)?;
        Ok(request)
    }

    pub fn to_nota(&self) -> Result<String> {
        let mut encoder = Encoder::nota();
        self.encode(&mut encoder)?;
        Ok(encoder.into_string())
    }

    pub fn execute(self, store: &MessageStore) -> Result<Output> {
        match self {
            Self::Send(send) => send.execute(store),
            Self::Inbox(inbox) => Ok(Output::InboxMessages(InboxMessages {
                recipient: inbox.recipient.clone(),
                messages: store.inbox(&inbox.recipient)?,
            })),
        }
    }
}

impl NotaEncode for DaemonRequest {
    fn encode(&self, encoder: &mut Encoder) -> nota_codec::Result<()> {
        match self {
            Self::Send(request) => request.encode(encoder),
            Self::Inbox(request) => request.encode(encoder),
        }
    }
}

impl NotaDecode for DaemonRequest {
    fn decode(decoder: &mut Decoder<'_>) -> nota_codec::Result<Self> {
        let head = decoder.peek_record_head()?;
        match head.as_str() {
            "DaemonSend" => Ok(Self::Send(DaemonSend::decode(decoder)?)),
            "DaemonInbox" => Ok(Self::Inbox(DaemonInbox::decode(decoder)?)),
            other => Err(nota_codec::Error::UnknownKindForVerb {
                verb: "DaemonRequest",
                got: other.to_string(),
            }),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct DaemonSend {
    pub pid: u32,
    pub recipient: ActorId,
    pub body: String,
}

impl DaemonSend {
    pub fn execute(self, store: &MessageStore) -> Result<Output> {
        let ancestry = ProcessAncestry::from_process(self.pid)?;
        let sender = store.resolve_sender_from_ancestry(&ancestry)?;
        let message = Send {
            recipient: self.recipient,
            body: self.body,
        }
        .into_message(sender, store.next_sequence()?);
        store.append(&message)?;
        store.deliver(&message)?;
        Ok(Output::Accepted(Accepted { message }))
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct DaemonInbox {
    pub recipient: ActorId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonCommandLine {
    arguments: Vec<String>,
}

impl DaemonCommandLine {
    pub fn from_env() -> Self {
        Self {
            arguments: std::env::args().skip(1).collect(),
        }
    }

    pub fn run(&self) -> Result<()> {
        let socket = self
            .arguments
            .first()
            .cloned()
            .map(DaemonSocket::from_path)
            .ok_or(Error::MissingDaemonSocket)?;
        let store = MessageStore::from_path(StorePath::from_environment());
        MessageDaemon::from_socket(socket, store).run()
    }
}

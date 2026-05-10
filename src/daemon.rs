use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use kameo::actor::{Actor, ActorRef, Spawn};
use kameo::error::Infallible;
use kameo::message::{Context, Message};
use nota_codec::{Decoder, Encoder, NotaDecode, NotaEncode, NotaRecord};
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

use crate::actors::ledger;
use crate::command::{Accepted, InboxMessages, Input, KnownActors, Output, Registered};
use crate::error::{Error, Result};
use crate::resolver::ProcessAncestry;
use crate::schema::expect_end;
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
        let runtime = tokio::runtime::Runtime::new()?;
        let actor = runtime.block_on(DaemonRoot::start(self.store.clone()));
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => Self::handle_stream(&runtime, &actor, stream),
                Err(error) => Err(error.into()),
            }
            .unwrap_or_else(|error| eprintln!("message-daemon client error: {error}"));
        }
        Ok(())
    }

    fn handle_stream(
        runtime: &tokio::runtime::Runtime,
        actor: &ActorRef<DaemonRoot>,
        stream: UnixStream,
    ) -> Result<()> {
        let envelope = DaemonFrame::from_stream(&stream)?.decode()?;
        let response = runtime
            .block_on(actor.ask(ExecuteEnvelope { envelope }).send())
            .map_err(|error| Error::ActorCall {
                detail: error.to_string(),
            })?;
        let mut writer = stream;
        DaemonFrame::from_envelope(&response)?.write_to(&mut writer)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct DaemonRoot {
    ledger: ActorRef<ledger::Ledger>,
    executed_request_count: u64,
    emitted_response_count: u64,
}

impl DaemonRoot {
    pub async fn start(store: MessageStore) -> ActorRef<Self> {
        let actor_reference = Self::spawn(store);
        actor_reference.wait_for_startup().await;
        actor_reference
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecuteEnvelope {
    pub envelope: DaemonEnvelope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestCountProbe {
    minimum_expected: u64,
}

impl RequestCountProbe {
    pub fn expecting_at_least(minimum_expected: u64) -> Self {
        Self { minimum_expected }
    }

    pub fn inspect(self, observed: u64) -> RequestCount {
        RequestCount {
            observed,
            minimum_expected: self.minimum_expected,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, kameo::Reply)]
pub struct RequestCount {
    observed: u64,
    minimum_expected: u64,
}

impl RequestCount {
    pub fn observed(&self) -> u64 {
        self.observed
    }

    pub fn satisfied(&self) -> bool {
        self.observed >= self.minimum_expected
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadRootRequestCount {
    pub probe: RequestCountProbe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadLedgerRequestCount {
    pub probe: RequestCountProbe,
}

impl Actor for DaemonRoot {
    type Args = MessageStore;
    type Error = Infallible;

    async fn on_start(
        store: Self::Args,
        actor_reference: ActorRef<Self>,
    ) -> std::result::Result<Self, Self::Error> {
        let ledger = ledger::Ledger::supervise(&actor_reference, store)
            .spawn()
            .await;
        Ok(Self {
            ledger,
            executed_request_count: 0,
            emitted_response_count: 0,
        })
    }
}

impl Message<ExecuteEnvelope> for DaemonRoot {
    type Reply = Result<DaemonEnvelope>;

    async fn handle(
        &mut self,
        message: ExecuteEnvelope,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        if matches!(message.envelope, DaemonEnvelope::Request(_)) {
            self.executed_request_count = self.executed_request_count.saturating_add(1);
        }
        let response = self
            .ledger
            .ask(ledger::ExecuteEnvelope {
                envelope: message.envelope,
            })
            .await
            .map_err(|error| Error::ActorCall {
                detail: error.to_string(),
            })?;
        self.emitted_response_count = self.emitted_response_count.saturating_add(1);
        Ok(response)
    }
}

impl Message<ReadRootRequestCount> for DaemonRoot {
    type Reply = RequestCount;

    async fn handle(
        &mut self,
        message: ReadRootRequestCount,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        message.probe.inspect(self.executed_request_count)
    }
}

impl Message<ReadLedgerRequestCount> for DaemonRoot {
    type Reply = Result<RequestCount>;

    async fn handle(
        &mut self,
        message: ReadLedgerRequestCount,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.ledger
            .ask(ledger::ReadRequestCount {
                probe: message.probe,
            })
            .await
            .map_err(|error| Error::ActorCall {
                detail: error.to_string(),
            })
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
        DaemonFrame::from_envelope(&DaemonEnvelope::Request(request))?.write_to(&mut stream)?;
        let output = DaemonFrame::from_stream(&stream)?.decode()?.into_output()?;
        let mut text = output.to_nota()?;
        text.push('\n');
        Ok(text)
    }
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum DaemonEnvelope {
    Request(DaemonRequest),
    Response(Output),
}

impl DaemonEnvelope {
    pub fn execute(self, store: &MessageStore) -> Result<Self> {
        match self {
            Self::Request(request) => Ok(Self::Response(request.execute(store)?)),
            Self::Response(output) => Ok(Self::Response(output)),
        }
    }

    pub fn into_output(self) -> Result<Output> {
        match self {
            Self::Response(output) => Ok(output),
            Self::Request(_) => Err(Error::InvalidDaemonResponse {
                got: "request envelope received where response was expected".to_string(),
            }),
        }
    }
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum DaemonRequest {
    Input(DaemonInput),
}

impl DaemonRequest {
    pub fn from_input(pid: u32, input: Input) -> Self {
        Self::Input(DaemonInput { pid, input })
    }

    pub fn from_nota(text: &str) -> Result<Self> {
        let mut decoder = Decoder::new(text);
        let request = Self::decode(&mut decoder)?;
        expect_end(&mut decoder)?;
        Ok(request)
    }

    pub fn to_nota(&self) -> Result<String> {
        let mut encoder = Encoder::new();
        self.encode(&mut encoder)?;
        Ok(encoder.into_string())
    }

    pub fn execute(self, store: &MessageStore) -> Result<Output> {
        match self {
            Self::Input(input) => input.execute(store),
        }
    }
}

impl NotaEncode for DaemonRequest {
    fn encode(&self, encoder: &mut Encoder) -> nota_codec::Result<()> {
        match self {
            Self::Input(input) => input.encode(encoder),
        }
    }
}

impl NotaDecode for DaemonRequest {
    fn decode(decoder: &mut Decoder<'_>) -> nota_codec::Result<Self> {
        let head = decoder.peek_record_head()?;
        match head.as_str() {
            "DaemonInput" => Ok(Self::Input(DaemonInput::decode(decoder)?)),
            other => Err(nota_codec::Error::UnknownKindForVerb {
                verb: "DaemonRequest",
                got: other.to_string(),
            }),
        }
    }
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct DaemonInput {
    pub pid: u32,
    pub input: Input,
}

impl DaemonInput {
    pub fn execute(self, store: &MessageStore) -> Result<Output> {
        let ancestry = ProcessAncestry::from_process(self.pid)?;
        match self.input {
            Input::Send(send) => {
                let sender = store.resolve_sender_from_ancestry(&ancestry)?;
                let message = send.into_message(sender, store.next_sequence()?);
                store.append(&message)?;
                store.deliver(&message)?;
                Ok(Output::Accepted(Accepted { message }))
            }
            Input::Inbox(inbox) => Ok(Output::InboxMessages(InboxMessages {
                recipient: inbox.recipient.clone(),
                messages: store.inbox(&inbox.recipient)?,
            })),
            Input::Tail(_) => {
                let sender = store.resolve_sender_from_ancestry(&ancestry)?;
                Ok(Output::InboxMessages(InboxMessages {
                    recipient: sender.clone(),
                    messages: store.inbox(&sender)?,
                }))
            }
            Input::Register(register) => {
                let actor = crate::schema::Actor {
                    name: register.name,
                    pid: ancestry.registration_pid()?,
                    endpoint: register.endpoint,
                };
                store.register(&actor)?;
                Ok(Output::Registered(Registered { actor }))
            }
            Input::Agents(_) => Ok(Output::KnownActors(KnownActors {
                actors: store.actors()?.actors().to_vec(),
            })),
            Input::Flush(_) => {
                let report = store.flush()?;
                Ok(Output::Flushed(crate::command::Flushed {
                    delivered: report.delivered as u64,
                    deferred: report.deferred.len() as u64,
                }))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonFrame {
    bytes: Vec<u8>,
}

impl DaemonFrame {
    const MAX_BYTES: usize = 16 * 1024 * 1024;

    pub fn from_envelope(envelope: &DaemonEnvelope) -> Result<Self> {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(envelope).map_err(|error| {
            Error::DaemonCodec {
                detail: error.to_string(),
            }
        })?;
        Ok(Self {
            bytes: bytes.into(),
        })
    }

    pub fn from_stream(mut stream: &UnixStream) -> Result<Self> {
        let mut header = [0_u8; 4];
        stream.read_exact(&mut header)?;
        let length = u32::from_le_bytes(header) as usize;
        if length > Self::MAX_BYTES {
            return Err(Error::DaemonFrameTooLarge { bytes: length });
        }
        let mut bytes = vec![0_u8; length];
        stream.read_exact(&mut bytes)?;
        Ok(Self { bytes })
    }

    pub fn write_to(&self, stream: &mut UnixStream) -> Result<()> {
        if self.bytes.len() > Self::MAX_BYTES {
            return Err(Error::DaemonFrameTooLarge {
                bytes: self.bytes.len(),
            });
        }
        stream.write_all(&(self.bytes.len() as u32).to_le_bytes())?;
        stream.write_all(&self.bytes)?;
        Ok(())
    }

    pub fn decode(&self) -> Result<DaemonEnvelope> {
        rkyv::from_bytes::<DaemonEnvelope, rkyv::rancor::Error>(&self.bytes).map_err(|error| {
            Error::DaemonCodec {
                detail: error.to_string(),
            }
        })
    }
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

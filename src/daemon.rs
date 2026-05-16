use std::io::BufReader;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use kameo::actor::{Actor, ActorRef, Spawn};
use kameo::error::{Infallible, SendError};
use kameo::message::{Context, Message};
use nota_codec::{Decoder, NotaDecode};
use signal_persona::TimestampNanos;
use signal_persona_auth::{ConnectionClass, MessageOrigin, OwnerIdentity, UnixUserId};
use signal_persona_message::{
    MessageDaemonConfiguration, MessageOperationKind, MessageReply, MessageRequest,
    MessageRequestUnimplemented, MessageUnimplementedReason, StampedMessageSubmission,
};

use crate::error::{Error, Result};
use crate::router::{
    ReceivedMessageRequest, SignalMessageSocket, SignalRouterClient, SignalRouterFrameCodec,
    SignalRouterSocket,
};
use crate::supervision::{SupervisionListener, SupervisionProfile, SupervisionSocketMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDaemon {
    message_socket: SignalMessageSocket,
    message_socket_mode: SocketMode,
    router_socket: SignalRouterSocket,
    supervision_socket_path: PathBuf,
    supervision_socket_mode: SupervisionSocketMode,
    owner_identity: OwnerIdentity,
}

impl MessageDaemon {
    /// Canonical constructor — every production launch reads typed
    /// `MessageDaemonConfiguration` from argv via `nota-config` and
    /// hands the record here.
    pub fn from_configuration(configuration: MessageDaemonConfiguration) -> Self {
        Self {
            message_socket: SignalMessageSocket::from_path(configuration.message_socket_path.as_str()),
            message_socket_mode: SocketMode::from_octal(configuration.message_socket_mode.into_u32()),
            router_socket: SignalRouterSocket::from_path(configuration.router_socket_path.as_str()),
            supervision_socket_path: PathBuf::from(configuration.supervision_socket_path.as_str()),
            supervision_socket_mode: SupervisionSocketMode::from_octal(
                configuration.supervision_socket_mode.into_u32(),
            ),
            owner_identity: configuration.owner_identity,
        }
    }

    /// In-process constructor — every input field explicit. Tests
    /// use this when they build the daemon directly without going
    /// through a configuration file.
    pub fn from_input(input: MessageDaemonInput) -> Self {
        Self {
            message_socket: input.message_socket,
            message_socket_mode: input.message_socket_mode,
            router_socket: input.router_socket,
            supervision_socket_path: input.supervision_socket_path,
            supervision_socket_mode: input.supervision_socket_mode,
            owner_identity: input.owner_identity,
        }
    }

    pub fn run(self) -> Result<()> {
        let listener = self.bind_listener()?;
        let _supervision = SupervisionListener::new(
            SupervisionProfile::message(),
            self.supervision_socket_path.clone(),
            self.supervision_socket_mode,
        )
        .spawn()?;
        let runtime = tokio::runtime::Runtime::new()?;
        let stamper = MessageOriginStamper::from_owner_identity(self.owner_identity.clone());
        let root = runtime.block_on(MessageDaemonRoot::start_root(MessageDaemonRootInput {
            router_socket: self.router_socket,
            stamper,
        }));
        eprintln!(
            "persona-message-daemon socket={}",
            self.message_socket.path().display()
        );
        for stream in listener.incoming() {
            let stream = stream?;
            Self::handle_connection(&runtime, &root, stream)?;
        }
        Ok(())
    }

    pub fn bind_listener(&self) -> Result<UnixListener> {
        if let Some(parent) = self.message_socket.path().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let _ = std::fs::remove_file(self.message_socket.path());
        let listener = UnixListener::bind(self.message_socket.path())?;
        std::fs::set_permissions(
            self.message_socket.path(),
            std::fs::Permissions::from_mode(self.message_socket_mode.as_octal()),
        )?;
        Ok(listener)
    }

    fn handle_connection(
        runtime: &tokio::runtime::Runtime,
        root: &ActorRef<MessageDaemonRoot>,
        stream: UnixStream,
    ) -> Result<()> {
        let mut connection = MessageDaemonConnection::from_stream(stream)?;
        let peer = connection.peer_credentials();
        let received = connection.read_request()?;
        let request = received.request.clone();
        let reply = match runtime
            .block_on(async { root.ask(ForwardMessageRequest { request, peer }).await })
        {
            Ok(reply) => reply,
            Err(SendError::HandlerError(error)) => return Err(error),
            Err(error) => {
                return Err(Error::Actor {
                    operation: "forward message request",
                    detail: format!("{error:?}"),
                });
            }
        };
        connection.write_reply(received, reply)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketMode(u32);

impl SocketMode {
    pub const fn from_octal(value: u32) -> Self {
        Self(value)
    }

    pub const fn as_octal(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDaemonInput {
    pub message_socket: SignalMessageSocket,
    pub message_socket_mode: SocketMode,
    pub router_socket: SignalRouterSocket,
    pub supervision_socket_path: PathBuf,
    pub supervision_socket_mode: SupervisionSocketMode,
    pub owner_identity: OwnerIdentity,
}

pub struct MessageDaemonConnection {
    stream: BufReader<UnixStream>,
    codec: SignalRouterFrameCodec,
    peer_credentials: PeerCredentials,
}

impl MessageDaemonConnection {
    pub fn from_stream(stream: UnixStream) -> Result<Self> {
        let peer_credentials = PeerCredentials::from_stream(&stream)?;
        Ok(Self {
            stream: BufReader::new(stream),
            codec: SignalRouterFrameCodec::default(),
            peer_credentials,
        })
    }

    pub fn peer_credentials(&self) -> PeerCredentials {
        self.peer_credentials
    }

    pub fn read_request(&mut self) -> Result<ReceivedMessageRequest> {
        let frame = self.codec.read_frame(&mut self.stream)?;
        self.codec.request_from_frame(frame)
    }

    pub fn write_reply(
        &mut self,
        request: ReceivedMessageRequest,
        reply: MessageReply,
    ) -> Result<()> {
        let frame = self
            .codec
            .reply_frame(request.exchange, request.verb, reply);
        self.codec.write_frame(self.stream.get_mut(), &frame)
    }
}

#[derive(Debug)]
pub struct MessageDaemonRoot {
    router: SignalRouterClient,
    stamper: MessageOriginStamper,
    forwarded_count: u64,
}

impl MessageDaemonRoot {
    pub fn new(input: MessageDaemonRootInput) -> Self {
        Self {
            router: input.router_socket.client(),
            stamper: input.stamper,
            forwarded_count: 0,
        }
    }

    pub async fn start_root(input: MessageDaemonRootInput) -> ActorRef<Self> {
        Self::spawn(Self::new(input))
    }

    fn forward(
        &mut self,
        request: MessageRequest,
        peer: PeerCredentials,
    ) -> Result<MessageReply> {
        match self.stamper.stamp_request(request, peer)? {
            ForwardDecision::Forward(request) => {
                let reply = self.router.submit(request)?;
                self.forwarded_count = self.forwarded_count.saturating_add(1);
                Ok(reply)
            }
            ForwardDecision::Reply(reply) => Ok(reply),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDaemonRootInput {
    pub router_socket: SignalRouterSocket,
    pub stamper: MessageOriginStamper,
}

impl Actor for MessageDaemonRoot {
    type Args = Self;
    type Error = Infallible;

    async fn on_start(
        state: Self::Args,
        _actor_ref: ActorRef<Self>,
    ) -> std::result::Result<Self, Self::Error> {
        Ok(state)
    }
}

pub struct ForwardMessageRequest {
    request: MessageRequest,
    peer: PeerCredentials,
}

impl Message<ForwardMessageRequest> for MessageDaemonRoot {
    type Reply = Result<MessageReply>;

    async fn handle(
        &mut self,
        message: ForwardMessageRequest,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.forward(message.request, message.peer)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerCredentials {
    user_id: UnixUserId,
}

impl PeerCredentials {
    pub fn from_user_id(user_id: UnixUserId) -> Self {
        Self { user_id }
    }

    pub fn from_stream(stream: &UnixStream) -> Result<Self> {
        let mut credentials = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                std::ptr::addr_of_mut!(credentials).cast(),
                std::ptr::addr_of_mut!(length),
            )
        };
        if result != 0 {
            return Err(Error::PeerCredentials);
        }
        Ok(Self {
            user_id: UnixUserId::new(credentials.uid),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageOriginStamper {
    engine_owner_identity: OwnerIdentity,
}

impl MessageOriginStamper {
    pub fn from_spawn_envelope_path(path: impl AsRef<Path>) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let mut decoder = Decoder::new(&text);
        let envelope = signal_persona::SpawnEnvelope::decode(&mut decoder)?;
        Ok(Self::from_spawn_envelope(envelope))
    }

    pub fn from_spawn_envelope(envelope: signal_persona::SpawnEnvelope) -> Self {
        Self::from_owner_identity(envelope.owner_identity)
    }

    pub fn from_owner_identity(engine_owner_identity: OwnerIdentity) -> Self {
        Self {
            engine_owner_identity,
        }
    }

    pub fn stamp_request(
        &self,
        request: MessageRequest,
        peer: PeerCredentials,
    ) -> Result<ForwardDecision> {
        match request {
            MessageRequest::MessageSubmission(submission) => Ok(ForwardDecision::Forward(
                MessageRequest::StampedMessageSubmission(StampedMessageSubmission {
                    submission,
                    origin: self.origin_for_peer(peer),
                    stamped_at: Self::timestamp_now()?,
                }),
            )),
            MessageRequest::StampedMessageSubmission(_) => Ok(ForwardDecision::Reply(
                MessageReply::MessageRequestUnimplemented(MessageRequestUnimplemented {
                    operation: MessageOperationKind::StampedMessageSubmission,
                    reason: MessageUnimplementedReason::NotInPrototypeScope,
                }),
            )),
            MessageRequest::InboxQuery(query) => {
                Ok(ForwardDecision::Forward(MessageRequest::InboxQuery(query)))
            }
        }
    }

    fn origin_for_peer(&self, peer: PeerCredentials) -> MessageOrigin {
        match &self.engine_owner_identity {
            OwnerIdentity::UnixUser(user_id) if peer.user_id == *user_id => {
                MessageOrigin::External(ConnectionClass::Owner)
            }
            _ => MessageOrigin::External(ConnectionClass::NonOwnerUser(peer.user_id)),
        }
    }

    fn timestamp_now() -> Result<TimestampNanos> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::ClockBeforeUnixEpoch)?;
        let nanos = duration.as_nanos().min(u128::from(u64::MAX)) as u64;
        Ok(TimestampNanos::new(nanos))
    }
}

pub enum ForwardDecision {
    Forward(MessageRequest),
    Reply(MessageReply),
}

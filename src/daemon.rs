use std::ffi::OsString;
use std::io::BufReader;
use std::os::unix::net::{UnixListener, UnixStream};

use kameo::actor::{Actor, ActorRef, Spawn};
use kameo::error::{Infallible, SendError};
use kameo::message::{Context, Message};
use signal_persona_message::{MessageReply, MessageRequest};

use crate::error::{Error, Result};
use crate::router::{
    SignalMessageSocket, SignalRouterClient, SignalRouterFrameCodec, SignalRouterSocket,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDaemonCommandLine {
    arguments: Vec<OsString>,
}

impl MessageDaemonCommandLine {
    pub fn from_env() -> Self {
        Self::from_arguments(std::env::args_os().skip(1))
    }

    pub fn from_arguments<I, S>(arguments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        Self {
            arguments: arguments.into_iter().map(Into::into).collect(),
        }
    }

    pub fn daemon(&self) -> Result<MessageDaemon> {
        self.reject_extra_arguments()?;
        let message_socket = self.message_socket()?;
        let router_socket = self.router_socket()?;
        Ok(MessageDaemon::from_input(MessageDaemonInput {
            message_socket,
            router_socket,
        }))
    }

    pub fn run(&self) -> Result<()> {
        self.daemon()?.run()
    }

    fn message_socket(&self) -> Result<SignalMessageSocket> {
        if let Some(argument) = self.arguments.first() {
            return Ok(SignalMessageSocket::from_path(argument));
        }
        SignalMessageSocket::from_environment().ok_or(Error::SignalMessageSocketMissing)
    }

    fn router_socket(&self) -> Result<SignalRouterSocket> {
        if let Some(argument) = self.arguments.get(1) {
            return Ok(SignalRouterSocket::from_path(argument));
        }
        SignalRouterSocket::from_environment()
            .or_else(SignalRouterSocket::from_peer_environment)
            .ok_or(Error::SignalRouterSocketMissing)
    }

    fn reject_extra_arguments(&self) -> Result<()> {
        if let Some(argument) = self.arguments.get(2) {
            return Err(Error::UnexpectedArgument {
                got: argument.to_string_lossy().to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDaemon {
    message_socket: SignalMessageSocket,
    router_socket: SignalRouterSocket,
}

impl MessageDaemon {
    pub fn from_input(input: MessageDaemonInput) -> Self {
        Self {
            message_socket: input.message_socket,
            router_socket: input.router_socket,
        }
    }

    pub fn run(self) -> Result<()> {
        if let Some(parent) = self.message_socket.path().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let _ = std::fs::remove_file(self.message_socket.path());
        let listener = UnixListener::bind(self.message_socket.path())?;
        let runtime = tokio::runtime::Runtime::new()?;
        let root = runtime.block_on(MessageDaemonRoot::start_root(MessageDaemonRootInput {
            router_socket: self.router_socket,
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

    fn handle_connection(
        runtime: &tokio::runtime::Runtime,
        root: &ActorRef<MessageDaemonRoot>,
        stream: UnixStream,
    ) -> Result<()> {
        let mut connection = MessageDaemonConnection::from_stream(stream);
        let request = connection.read_request()?;
        let reply =
            match runtime.block_on(async { root.ask(ForwardMessageRequest { request }).await }) {
                Ok(reply) => reply,
                Err(SendError::HandlerError(error)) => return Err(error),
                Err(error) => {
                    return Err(Error::Actor {
                        operation: "forward message request",
                        detail: format!("{error:?}"),
                    });
                }
            };
        connection.write_reply(reply)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDaemonInput {
    pub message_socket: SignalMessageSocket,
    pub router_socket: SignalRouterSocket,
}

pub struct MessageDaemonConnection {
    stream: BufReader<UnixStream>,
    codec: SignalRouterFrameCodec,
}

impl MessageDaemonConnection {
    pub fn from_stream(stream: UnixStream) -> Self {
        Self {
            stream: BufReader::new(stream),
            codec: SignalRouterFrameCodec::default(),
        }
    }

    pub fn read_request(&mut self) -> Result<MessageRequest> {
        let frame = self.codec.read_frame(&mut self.stream)?;
        self.codec.request_from_frame(frame)
    }

    pub fn write_reply(&mut self, reply: MessageReply) -> Result<()> {
        let frame = signal_persona_message::Frame::new(signal_core::FrameBody::Reply(
            signal_core::Reply::operation(reply),
        ));
        self.codec.write_frame(self.stream.get_mut(), &frame)
    }
}

#[derive(Debug)]
pub struct MessageDaemonRoot {
    router: SignalRouterClient,
    forwarded_count: u64,
}

impl MessageDaemonRoot {
    pub fn new(input: MessageDaemonRootInput) -> Self {
        Self {
            router: input.router_socket.client(),
            forwarded_count: 0,
        }
    }

    pub async fn start_root(input: MessageDaemonRootInput) -> ActorRef<Self> {
        Self::spawn(Self::new(input))
    }

    fn forward(&mut self, request: MessageRequest) -> Result<MessageReply> {
        let reply = self.router.submit(request)?;
        self.forwarded_count = self.forwarded_count.saturating_add(1);
        Ok(reply)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDaemonRootInput {
    pub router_socket: SignalRouterSocket,
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
}

impl Message<ForwardMessageRequest> for MessageDaemonRoot {
    type Reply = Result<MessageReply>;

    async fn handle(
        &mut self,
        message: ForwardMessageRequest,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.forward(message.request)
    }
}

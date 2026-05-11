use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;

use nota_codec::{Decoder, Encoder, NotaDecode, NotaEncode, NotaRecord};
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use signal_persona_message::{
    InboxQuery as SignalInboxQuery, MessageBody as SignalMessageBody,
    MessageRecipient as SignalMessageRecipient, MessageReply, MessageRequest, MessageSubmission,
    SubmissionRejectionReason as SignalSubmissionRejectionReason,
};

use crate::daemon::{DaemonSocket, MessageDaemonClient};
use crate::error::{Error, Result};
use crate::router::{RouterSocket, SignalRouterSocket};
use crate::schema::{Actor, ActorId, EndpointTransport, Message, MessageId, ThreadId, expect_end};
use crate::store::MessageStore;

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Send {
    pub recipient: ActorId,
    pub body: String,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Inbox {
    pub recipient: ActorId,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Tail {}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Register {
    pub name: ActorId,
    pub endpoint: Option<EndpointTransport>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Agents {}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Flush {}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Send(Send),
    Inbox(Inbox),
    Tail(Tail),
    Register(Register),
    Agents(Agents),
    Flush(Flush),
}

impl Input {
    pub fn from_nota(text: &str) -> Result<Self> {
        let mut decoder = Decoder::new(text);
        let input = Self::decode(&mut decoder)?;
        expect_end(&mut decoder)?;
        Ok(input)
    }

    pub fn execute(self, store: &MessageStore) -> Result<Output> {
        match self {
            Self::Send(send) => {
                let sender = store.resolve_sender()?;
                let message = send.into_message(sender, store.next_sequence()?);
                store.append(&message)?;
                store.deliver(&message)?;
                Ok(Output::Accepted(Accepted { message }))
            }
            Self::Inbox(inbox) => {
                let messages = store.inbox(&inbox.recipient)?;
                Ok(Output::InboxMessages(InboxMessages {
                    recipient: inbox.recipient,
                    messages,
                }))
            }
            Self::Tail(_) => {
                let recipient = store.resolve_sender()?;
                let stdout = std::io::stdout();
                store.tail(&recipient, stdout.lock())?;
                unreachable!("tail returns only on error")
            }
            Self::Register(register) => {
                let actor = Actor {
                    name: register.name,
                    pid: store.registration_pid()?,
                    endpoint: register.endpoint,
                };
                store.register(&actor)?;
                Ok(Output::Registered(Registered { actor }))
            }
            Self::Agents(_) => Ok(Output::KnownActors(KnownActors {
                actors: store.actors()?.actors().to_vec(),
            })),
            Self::Flush(_) => {
                let report = store.flush()?;
                Ok(Output::Flushed(Flushed {
                    delivered: report.delivered as u64,
                    deferred: report.deferred.len() as u64,
                }))
            }
        }
    }

    pub fn run(mut self, store: &MessageStore, mut output: impl Write) -> Result<()> {
        if matches!(&self, Self::Tail(_)) {
            let recipient = store.resolve_sender()?;
            return store.tail(&recipient, output);
        }
        if let Some(socket) = DaemonSocket::from_environment() {
            let response = MessageDaemonClient::from_socket(socket).submit(self)?;
            write!(output, "{response}")?;
            return Ok(());
        }
        if let Some(socket) = SignalRouterSocket::from_environment() {
            match self {
                Self::Send(send) => {
                    let sender = store.resolve_sender()?;
                    let reply = socket
                        .client()
                        .submit(&sender, send.into_message_request())?;
                    writeln!(output, "{}", Output::from_router_reply(reply)?.to_nota()?)?;
                    return Ok(());
                }
                Self::Inbox(inbox) => {
                    let sender = store.resolve_sender()?;
                    let reply = socket
                        .client()
                        .submit(&sender, inbox.into_message_request())?;
                    writeln!(output, "{}", Output::from_router_reply(reply)?.to_nota()?)?;
                    return Ok(());
                }
                other => {
                    self = other;
                }
            }
        }
        if let Some(socket) = RouterSocket::from_environment() {
            if let Self::Send(send) = self.clone() {
                let sender = store.resolve_sender()?;
                let message = send.into_message(sender, store.next_sequence()?);
                let _reply = socket.client().route(&message)?;
                store.append(&message)?;
                writeln!(
                    output,
                    "{}",
                    Output::Accepted(Accepted { message }).to_nota()?
                )?;
                return Ok(());
            }
        }
        match self {
            Self::Tail(_) => unreachable!("tail returns before daemon routing"),
            input => {
                let output_record = input.execute(store)?;
                writeln!(output, "{}", output_record.to_nota()?)?;
                Ok(())
            }
        }
    }
}

impl Send {
    pub fn into_message(self, sender: ActorId, sequence: u64) -> Message {
        let thread = ThreadId::new(format!(
            "direct-{}-{}",
            sender.as_str(),
            self.recipient.as_str()
        ));
        let id = MessageId::from_parts(
            sequence,
            &thread,
            &sender,
            &self.recipient,
            self.body.as_str(),
        );
        Message {
            id,
            thread,
            from: sender,
            to: self.recipient,
            body: self.body,
            attachments: Vec::new(),
        }
    }

    pub fn into_message_request(self) -> MessageRequest {
        MessageRequest::MessageSubmission(MessageSubmission {
            recipient: SignalMessageRecipient::new(self.recipient.as_str()),
            body: SignalMessageBody::new(self.body),
        })
    }
}

impl Inbox {
    pub fn into_message_request(self) -> MessageRequest {
        MessageRequest::InboxQuery(SignalInboxQuery {
            recipient: SignalMessageRecipient::new(self.recipient.as_str()),
        })
    }
}

impl NotaEncode for Input {
    fn encode(&self, encoder: &mut Encoder) -> nota_codec::Result<()> {
        match self {
            Self::Send(input) => input.encode(encoder),
            Self::Inbox(input) => input.encode(encoder),
            Self::Tail(input) => input.encode(encoder),
            Self::Register(input) => input.encode(encoder),
            Self::Agents(input) => input.encode(encoder),
            Self::Flush(input) => input.encode(encoder),
        }
    }
}

impl NotaDecode for Input {
    fn decode(decoder: &mut Decoder<'_>) -> nota_codec::Result<Self> {
        let head = decoder.peek_record_head()?;
        match head.as_str() {
            "Send" => Ok(Self::Send(Send::decode(decoder)?)),
            "Inbox" => Ok(Self::Inbox(Inbox::decode(decoder)?)),
            "Tail" => Ok(Self::Tail(Tail::decode(decoder)?)),
            "Register" => Ok(Self::Register(Register::decode(decoder)?)),
            "Agents" => Ok(Self::Agents(Agents::decode(decoder)?)),
            "Flush" => Ok(Self::Flush(Flush::decode(decoder)?)),
            other => Err(nota_codec::Error::UnknownKindForVerb {
                verb: "Input",
                got: other.to_string(),
            }),
        }
    }
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Accepted {
    pub message: Message,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct InboxMessages {
    pub recipient: ActorId,
    pub messages: Vec<Message>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Registered {
    pub actor: Actor,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct KnownActors {
    pub actors: Vec<Actor>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Flushed {
    pub delivered: u64,
    pub deferred: u64,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct SubmissionAccepted {
    pub message_slot: u64,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct SubmissionRejected {
    pub reason: SubmissionRejectionReason,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum SubmissionRejectionReason {
    StoreRejected,
    RecipientNotFound,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct RouterInboxListing {
    pub messages: Vec<RouterInboxEntry>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct RouterInboxEntry {
    pub message_slot: u64,
    pub sender: ActorId,
    pub body: String,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum Output {
    Accepted(Accepted),
    InboxMessages(InboxMessages),
    Registered(Registered),
    KnownActors(KnownActors),
    Flushed(Flushed),
    SubmissionAccepted(SubmissionAccepted),
    SubmissionRejected(SubmissionRejected),
    RouterInboxListing(RouterInboxListing),
}

impl Output {
    pub fn to_nota(&self) -> Result<String> {
        let mut encoder = Encoder::new();
        self.encode(&mut encoder)?;
        Ok(encoder.into_string())
    }

    pub fn from_router_reply(reply: MessageReply) -> Result<Self> {
        match reply {
            MessageReply::SubmissionAccepted(acceptance) => {
                Ok(Self::SubmissionAccepted(SubmissionAccepted {
                    message_slot: acceptance.message_slot.into_u64(),
                }))
            }
            MessageReply::SubmissionRejected(rejection) => {
                Ok(Self::SubmissionRejected(SubmissionRejected {
                    reason: SubmissionRejectionReason::from_signal(rejection.reason),
                }))
            }
            MessageReply::InboxListing(listing) => {
                Ok(Self::RouterInboxListing(RouterInboxListing {
                    messages: listing
                        .messages
                        .into_iter()
                        .map(RouterInboxEntry::from_signal)
                        .collect(),
                }))
            }
        }
    }
}

impl NotaEncode for Output {
    fn encode(&self, encoder: &mut Encoder) -> nota_codec::Result<()> {
        match self {
            Self::Accepted(output) => output.encode(encoder),
            Self::InboxMessages(output) => output.encode(encoder),
            Self::Registered(output) => output.encode(encoder),
            Self::KnownActors(output) => output.encode(encoder),
            Self::Flushed(output) => output.encode(encoder),
            Self::SubmissionAccepted(output) => output.encode(encoder),
            Self::SubmissionRejected(output) => output.encode(encoder),
            Self::RouterInboxListing(output) => output.encode(encoder),
        }
    }
}

impl SubmissionRejectionReason {
    fn from_signal(reason: SignalSubmissionRejectionReason) -> Self {
        match reason {
            SignalSubmissionRejectionReason::StoreRejected => Self::StoreRejected,
            SignalSubmissionRejectionReason::RecipientNotFound => Self::RecipientNotFound,
        }
    }
}

impl NotaEncode for SubmissionRejectionReason {
    fn encode(&self, encoder: &mut Encoder) -> nota_codec::Result<()> {
        match self {
            Self::StoreRejected => "StoreRejected".to_string().encode(encoder),
            Self::RecipientNotFound => "RecipientNotFound".to_string().encode(encoder),
        }
    }
}

impl NotaDecode for SubmissionRejectionReason {
    fn decode(decoder: &mut Decoder<'_>) -> nota_codec::Result<Self> {
        match String::decode(decoder)?.as_str() {
            "StoreRejected" => Ok(Self::StoreRejected),
            "RecipientNotFound" => Ok(Self::RecipientNotFound),
            other => Err(nota_codec::Error::UnknownKindForVerb {
                verb: "SubmissionRejectionReason",
                got: other.to_string(),
            }),
        }
    }
}

impl RouterInboxEntry {
    fn from_signal(entry: signal_persona_message::InboxEntry) -> Self {
        Self {
            message_slot: entry.message_slot.into_u64(),
            sender: ActorId::new(entry.sender.as_str()),
            body: entry.body.as_str().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLine {
    arguments: Vec<OsString>,
}

impl CommandLine {
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

    pub fn decode_input(&self) -> Result<Input> {
        let Some(first) = self.arguments.first() else {
            return Err(Error::MissingInput);
        };
        self.require_single_argument()?;

        if CommandLineArgument::new(first).starts_inline_record() {
            let Some(text) = first.to_str() else {
                return Err(Error::InvalidInlineNotaArgument {
                    got: format!("{first:?}"),
                });
            };
            Input::from_nota(text)
        } else {
            InputFile::from_path(PathBuf::from(first)).decode()
        }
    }

    pub fn run(&self, store: &MessageStore, output: impl Write) -> Result<()> {
        self.decode_input()?.run(store, output)
    }

    fn require_single_argument(&self) -> Result<()> {
        if let Some(argument) = self.arguments.get(1) {
            return Err(Error::UnexpectedArgument {
                got: argument.to_string_lossy().to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputFile {
    path: PathBuf,
}

impl InputFile {
    pub fn from_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn decode(&self) -> Result<Input> {
        let text = std::fs::read_to_string(&self.path)?;
        Input::from_nota(&text)
    }
}

struct CommandLineArgument<'argument> {
    argument: &'argument OsString,
}

impl<'argument> CommandLineArgument<'argument> {
    fn new(argument: &'argument OsString) -> Self {
        Self { argument }
    }

    fn starts_inline_record(&self) -> bool {
        self.argument.to_string_lossy().starts_with('(')
    }
}

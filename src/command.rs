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

use crate::error::{Error, Result};
use crate::resolver::ActorIndexPath;
use crate::router::SignalRouterSocket;
use crate::schema::{ActorId, expect_end};

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Send {
    pub recipient: ActorId,
    pub body: String,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Inbox {
    pub recipient: ActorId,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Send(Send),
    Inbox(Inbox),
}

impl Input {
    pub fn from_nota(text: &str) -> Result<Self> {
        let mut decoder = Decoder::new(text);
        let input = Self::decode(&mut decoder)?;
        expect_end(&mut decoder)?;
        Ok(input)
    }

    pub fn run(self, actor_index_path: &ActorIndexPath, mut output: impl Write) -> Result<()> {
        let socket =
            SignalRouterSocket::from_environment().ok_or(Error::SignalRouterSocketMissing)?;
        let sender = actor_index_path.resolve_current_process()?;
        let request = self.into_message_request();
        let reply = socket.client().submit(&sender, request)?;
        writeln!(output, "{}", Output::from_router_reply(reply)?.to_nota()?)?;
        Ok(())
    }

    fn into_message_request(self) -> MessageRequest {
        match self {
            Self::Send(send) => send.into_message_request(),
            Self::Inbox(inbox) => inbox.into_message_request(),
        }
    }
}

impl Send {
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
        }
    }
}

impl NotaDecode for Input {
    fn decode(decoder: &mut Decoder<'_>) -> nota_codec::Result<Self> {
        let head = decoder.peek_record_head()?;
        match head.as_str() {
            "Send" => Ok(Self::Send(Send::decode(decoder)?)),
            "Inbox" => Ok(Self::Inbox(Inbox::decode(decoder)?)),
            other => Err(nota_codec::Error::UnknownKindForVerb {
                verb: "Input",
                got: other.to_string(),
            }),
        }
    }
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

    pub fn run(&self, actor_index_path: &ActorIndexPath, output: impl Write) -> Result<()> {
        self.decode_input()?.run(actor_index_path, output)
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

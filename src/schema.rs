use nota_codec::{Decoder, Encoder, NotaDecode, NotaEncode, NotaRecord, NotaTransparent};
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

#[derive(
    Archive, RkyvSerialize, RkyvDeserialize, NotaTransparent, Debug, Clone, PartialEq, Eq, Hash,
)]
pub struct MessageId(String);

impl MessageId {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(
    Archive, RkyvSerialize, RkyvDeserialize, NotaTransparent, Debug, Clone, PartialEq, Eq, Hash,
)]
pub struct ThreadId(String);

impl ThreadId {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(
    Archive, RkyvSerialize, RkyvDeserialize, NotaTransparent, Debug, Clone, PartialEq, Eq, Hash,
)]
pub struct ActorId(String);

impl ActorId {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub name: ActorId,
    pub pid: u32,
    pub endpoint: Option<EndpointTransport>,
}

impl Actor {
    pub fn from_nota(text: &str) -> nota_codec::Result<Self> {
        let mut decoder = Decoder::nota(text);
        let actor = Self::decode(&mut decoder)?;
        expect_end(&mut decoder)?;
        Ok(actor)
    }

    pub fn to_nota(&self) -> nota_codec::Result<String> {
        let mut encoder = Encoder::nota();
        self.encode(&mut encoder)?;
        Ok(encoder.into_string())
    }
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct EndpointTransport {
    pub kind: EndpointKind,
    pub target: String,
    pub aux: Option<String>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaTransparent, Debug, Clone, PartialEq, Eq)]
pub struct EndpointKind(String);

impl EndpointKind {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub path: String,
    pub media_type: Option<String>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: MessageId,
    pub thread: ThreadId,
    pub from: ActorId,
    pub to: ActorId,
    pub body: String,
    pub attachments: Vec<Attachment>,
}

impl Message {
    pub fn from_nota(text: &str) -> nota_codec::Result<Self> {
        let mut decoder = Decoder::nota(text);
        let message = Self::decode(&mut decoder)?;
        expect_end(&mut decoder)?;
        Ok(message)
    }

    pub fn to_nota(&self) -> nota_codec::Result<String> {
        let mut encoder = Encoder::nota();
        self.encode(&mut encoder)?;
        Ok(encoder.into_string())
    }
}

pub fn expect_end(decoder: &mut Decoder<'_>) -> nota_codec::Result<()> {
    if let Some(token) = decoder.peek_token()? {
        return Err(nota_codec::Error::UnexpectedToken {
            expected: "end of input",
            got: token,
        });
    }
    Ok(())
}

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

    pub fn from_parts(
        sequence: u64,
        thread: &ThreadId,
        from: &ActorId,
        to: &ActorId,
        body: &str,
    ) -> Self {
        let mut hash = ShortMessageHash::new();
        hash.feed_u64(sequence);
        hash.feed_str(thread.as_str());
        hash.feed_str(from.as_str());
        hash.feed_str(to.as_str());
        hash.feed_str(body);
        Self(format!("m-{}", hash.finish_base32()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShortMessageHash {
    value: u64,
}

impl ShortMessageHash {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    const ALPHABET: &'static [u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

    fn new() -> Self {
        Self {
            value: Self::OFFSET,
        }
    }

    fn feed_u64(&mut self, value: u64) {
        self.feed_bytes(value.to_le_bytes().as_slice());
    }

    fn feed_str(&mut self, text: &str) {
        self.feed_u64(text.len() as u64);
        self.feed_bytes(text.as_bytes());
    }

    fn feed_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.value ^= u64::from(*byte);
            self.value = self.value.wrapping_mul(Self::PRIME);
        }
    }

    fn finish_base32(self) -> String {
        let mut value = self.value;
        let mut text = String::with_capacity(7);
        for _ in 0..7 {
            text.push(Self::ALPHABET[(value & 31) as usize] as char);
            value >>= 5;
        }
        text
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

use nota_codec::{Decoder, Encoder, NotaDecode, NotaEncode, NotaRecord, NotaTransparent};
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

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
}

impl Actor {
    pub fn from_nota(text: &str) -> nota_codec::Result<Self> {
        let mut decoder = Decoder::new(text);
        let actor = Self::decode(&mut decoder)?;
        expect_end(&mut decoder)?;
        Ok(actor)
    }

    pub fn to_nota(&self) -> nota_codec::Result<String> {
        let mut encoder = Encoder::new();
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

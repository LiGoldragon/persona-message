use nota_codec::{Decoder, NotaTransparent};
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

/// Name of the message recipient as written on the CLI surface.
#[derive(
    Archive, RkyvSerialize, RkyvDeserialize, NotaTransparent, Debug, Clone, PartialEq, Eq, Hash,
)]
pub struct RecipientName(String);

impl RecipientName {
    /// Creates a recipient name from the CLI text projection.
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// Returns the recipient name text.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Requires the NOTA input decoder to be at end of input.
pub fn expect_end(decoder: &mut Decoder<'_>) -> nota_codec::Result<()> {
    if let Some(token) = decoder.peek_token()? {
        return Err(nota_codec::Error::UnexpectedToken {
            expected: "end of input",
            got: token,
        });
    }
    Ok(())
}

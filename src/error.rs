use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("nota: {0}")]
    Nota(#[from] nota_codec::Error),

    #[error("signal frame: {0}")]
    SignalFrame(#[from] signal_core::FrameError),

    #[error("inline Nota argument must be UTF-8: {got:?}")]
    InvalidInlineNotaArgument { got: String },

    #[error("missing NOTA input; pass one record such as '(Send designer \"hello\")'")]
    MissingInput,

    #[error("unexpected command-line argument: {got:?}")]
    UnexpectedArgument { got: String },

    #[error("message store line {line} is invalid in {path:?}")]
    InvalidStoreLine {
        path: PathBuf,
        line: usize,
        source: nota_codec::Error,
    },

    #[error("actor index line {line} is invalid in {path:?}")]
    InvalidActorLine {
        path: PathBuf,
        line: usize,
        source: nota_codec::Error,
    },

    #[error("no actor in {path:?} matches this process ancestry")]
    NoMatchingAgent { path: PathBuf },

    #[error("process {pid} has no PPid field in /proc/{pid}/status")]
    MissingParentProcess { pid: u32 },

    #[error("process ancestry is empty")]
    EmptyProcessAncestry,

    #[error("process id {got:?} is invalid")]
    InvalidProcessId { got: String },

    #[error("daemon socket is not configured")]
    MissingDaemonSocket,

    #[error("daemon response was invalid: {got}")]
    InvalidDaemonResponse { got: String },

    #[error("daemon binary frame is too large: {bytes} bytes")]
    DaemonFrameTooLarge { bytes: usize },

    #[error("daemon binary codec: {detail}")]
    DaemonCodec { detail: String },

    #[error("actor call failed: {detail}")]
    ActorCall { detail: String },

    #[error("router reply was not valid for this command: {got}")]
    UnexpectedRouterReply { got: String },
}

pub type Result<T> = std::result::Result<T, Error>;

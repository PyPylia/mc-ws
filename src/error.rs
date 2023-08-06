use std::{fmt, result};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, watch, AcquireError};
use tokio_tungstenite::tungstenite;
use uuid::Uuid;

use crate::packet::{CommandResponsePacket, EventPacket, Packet};

pub type Result<T> = result::Result<T, Error>;
pub type MultiResult<T> = result::Result<T, MultiError>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("websocket error")]
    WebsocketError(#[from] tungstenite::Error),
    #[error("json parsing error")]
    JsonParseError(#[from] serde_json::Error),
    #[error("{0} stream exhausted")]
    StreamExhausted(&'static str),
    #[error("minecraft error with status: {status_code} {status_message:?}")]
    MinecraftError {
        status_message: Option<String>,
        status_code: i32,
    },
    #[error("event loop not running")]
    LoopNotRunning,
    #[error("failed to broadcast event")]
    EventBroadcastFailed(#[from] watch::error::SendError<EventPacket>),
    #[error("failed to receive event")]
    EventReceiveFailed(#[from] watch::error::RecvError),
    #[error("failed to send packet")]
    PacketSendFailed(#[from] mpsc::error::SendError<Packet>),
    #[error("failed to send command")]
    CommandSendFailed(
        #[from]
        mpsc::error::SendError<(
            Uuid,
            oneshot::Sender<CommandResponsePacket>,
        )>,
    ),
    #[error("unexpected packet: {0:?}")]
    UnexpectedPacket(Packet),
    #[error("failed to handle command")]
    CommandHandlingError,
    #[error("failed to obtain semaphore")]
    AcquireError(#[from] AcquireError),
    #[error("command response never broadcasted")]
    CommandResponseNeverBroadcasted(#[from] oneshot::error::RecvError),
    #[error("missing field: {0}")]
    MissingField(&'static str),
    #[error("invalid type")]
    InvalidType,
}

#[derive(Debug)]
pub enum MultiError {
    LoopErrored(Error),
    HandlerErrored(Error),
    BothErrored {
        loop_error: Error,
        handler_error: Error,
    },
}

impl std::error::Error for MultiError {}
impl fmt::Display for MultiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::LoopErrored(loop_error) => write!(f, "loop error: {}", loop_error),
            Self::HandlerErrored(handler_error) => write!(f, "handler error: {}", handler_error),
            Self::BothErrored {
                loop_error,
                handler_error,
            } => {
                writeln!(f, "loop error: {}", loop_error)?;
                write!(f, "handler error: {}", handler_error)
            }
        }
    }
}

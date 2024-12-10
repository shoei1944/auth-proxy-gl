use crate::launcher::types::response;
use std::fmt::{Display, Formatter};
use tokio::{
    sync::{mpsc, oneshot},
    time,
};
use tokio_tungstenite::tungstenite;

#[derive(Debug)]
pub enum Error {
    UnexpectedResponse(response::any::Kind),
    Actor(ActorError),
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UnexpectedResponse(err) => write!(fmt, "Unexpected response: {:?}", err),
            Error::Actor(err) => write!(fmt, "Error from actor: {}", err),
        }
    }
}

impl From<ActorError> for Error {
    fn from(value: ActorError) -> Self {
        Error::Actor(value)
    }
}

#[derive(Debug)]
pub enum ActorError {
    Send,
    Recv(oneshot::error::RecvError),
    Socket(tungstenite::Error),
    TimeoutExceeded(time::error::Elapsed),
    Serialize(serde_json::error::Error),
}

impl std::error::Error for ActorError {}

impl Display for ActorError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ActorError::Socket(err) => write!(fmt, "Socket: {}", err),
            ActorError::Send => write!(fmt, "Send to channel"),
            ActorError::Recv(err) => write!(fmt, "Recv from channel: {}", err),
            ActorError::TimeoutExceeded(err) => {
                write!(fmt, "Response wait timeout exceeded: {}", err)
            }
            ActorError::Serialize(err) => write!(fmt, "Failed to serialize request: {}", err),
        }
    }
}

impl From<tungstenite::Error> for ActorError {
    fn from(value: tungstenite::Error) -> Self {
        ActorError::Socket(value)
    }
}

impl<T> From<mpsc::error::SendError<T>> for ActorError {
    fn from(_value: mpsc::error::SendError<T>) -> Self {
        ActorError::Send
    }
}

impl From<oneshot::error::RecvError> for ActorError {
    fn from(value: oneshot::error::RecvError) -> Self {
        ActorError::Recv(value)
    }
}

impl From<time::error::Elapsed> for ActorError {
    fn from(value: time::error::Elapsed) -> Self {
        ActorError::TimeoutExceeded(value)
    }
}

impl From<serde_json::error::Error> for ActorError {
    fn from(value: serde_json::error::Error) -> Self {
        ActorError::Serialize(value)
    }
}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("transport error: {0}")]
    Transport(String),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("peer error: {0}")]
    Peer(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("connection error: {0}")]
    Connection(String),

    #[error("sync error: {0}")]
    Sync(String),

    #[error("channel error: {0}")]
    Channel(String),

    #[error("configuration error: {0}")]
    Config(String),
}

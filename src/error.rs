use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("model not found: {0}")]
    ModelNotFound(PathBuf),
    #[error("model load failed: {0}")]
    ModelLoadFailed(String),
    #[error("storage initialization failed: {0}")]
    StorageInit(#[from] io::Error),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("invalid config: {0}")]
    InvalidConfig(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ConversationError {
    #[error("conversation not found: {0}")]
    NotFound(String),
    #[error("storage read error: {0}")]
    StorageRead(io::Error),
    #[error("storage write error: {0}")]
    StorageWrite(io::Error),
    #[error("conversation in use: {0}")]
    InUse(String),
}

#[derive(Debug, thiserror::Error)]
pub enum PutError {
    #[error("data is empty or whitespace-only")]
    EmptyData,
    #[error("metadata required: content has {token_count} tokens, max indexable is {max}")]
    MetadataRequired { token_count: usize, max: usize },
    #[error("metadata too long: {token_count} tokens, max indexable is {max}")]
    MetadataTooLong { token_count: usize, max: usize },
    #[error("embedding failed: {0}")]
    EmbeddingFailed(String),
    #[error("storage write error: {0}")]
    StorageWrite(io::Error),
    #[error("persistence segment capacity exceeded: {current}/{max}")]
    PersistCapacityExceeded { current: usize, max: usize },
}

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("query is empty or whitespace-only")]
    EmptyQuery,
    #[error("query too long: {token_count} tokens, max is {max}")]
    QueryTooLong { token_count: usize, max: usize },
    #[error("embedding failed: {0}")]
    EmbeddingFailed(String),
    #[error("index corrupted: {0}")]
    IndexCorrupted(String),
}

#[derive(Debug, thiserror::Error)]
pub enum GetError {
    #[error("node not found: {0}")]
    NotFound(i64),
    #[error("storage read error: {0}")]
    StorageRead(io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum RebuildError {
    #[error("embedding failed: {0}")]
    EmbeddingFailed(String),
    #[error("storage write error: {0}")]
    StorageWrite(io::Error),
    #[error("no mismatched segments found")]
    NoMismatchedSegments,
}

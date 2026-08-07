use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("model not found: {0}")]
    ModelNotFound(PathBuf),
    #[error("model download failed: {0}")]
    ModelDownloadFailed(String),
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
pub enum ForgetError {
    #[error("node not found: {0}")]
    NodeNotFound(i64),
    #[error("conversation not found: {0}")]
    ConversationNotFound(String),
    #[error(
        "segment {segment_id} is the open segment of active conversation {conversation_id}; \
         drop the conversation handle first"
    )]
    SegmentInUse {
        segment_id: i64,
        conversation_id: String,
    },
    #[error("no nodes matched")]
    NoMatch,
    #[error("storage read error: {0}")]
    StorageRead(io::Error),
    #[error("storage write error: {0}")]
    StorageWrite(io::Error),
    #[error("index rebuild failed: {0}")]
    RebuildFailed(String),
    #[error("search failed: {0}")]
    Search(#[from] SearchError),
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

---
type: bmad-distillate
sources:
  - "brainstorming-session-2026-04-07-0800.md"
  - "brainstorming-session-2026-04-08-0800.md"
downstream_consumer: "Rust implementation of Moerae system"
created: "2026-04-08"
token_estimate: 4677
parts: 1
supersedes:
  - "brainstorming-session-2026-04-07-0800-distillate/"
  - "brainstorming-session-2026-04-08-0800-distillate.md"
---

## Core Concept

- Moerae: queryable hierarchical AI memory system replacing linear file indirection. Single search call returns relevant content at right detail level. Language: Rust.
- Two data verbs: put (store content), search (retrieve content). No delete verb; lifecycle handles forgetting.
- Search quality never degrades from delivery constraints. Ranking resolves conflicts; no conflict detection logic.
- Data path (put/search) is lifecycle-transparent; maintenance path (stats, rebuild) exposes internals when needed.
- Single-process-per-project constraint for v1. Multi-process access to same project is undefined behavior.
- No encryption at rest for v1; SQLite stores plaintext. SQLCipher or filesystem-level encryption are v2 options.

## API Design

- Three-level API: init() returns project handle; create_conversation() returns conversation handle with UUID; put/search called on conversation handle.
- `moerae::init(project_id: &str)` -> Result<Moerae, InitError>; project_id hashed to filesystem-safe directory name (hex(sha256)[..16]); original stored in SQLite config. Loads config from SQLite if exists, otherwise uses defaults. Runs eviction check via single grouped SQL query. Scans indexes/ directory and deletes orphan files (.usearch and .usearch.tmp) not referenced by any segment row. Default model path: ~/.moerae/models/embeddinggemma-300m-qat-q8_0.gguf; ModelNotFound if absent.
- `moerae::builder(project_id: &str).config(config).model_path(path).build()` -> Result<Moerae, InitError>; builder for custom config and/or model. Validates all config parameters (see Config Parameters validation list). Returns InitError::InvalidConfig on failure.
- `moerae.create_conversation()` -> Result<Conversation, ConversationError>; generates UUID internally; stored in SQLite. Caches segment list in memory. Open segments created lazily on first put, not on conversation creation.
- `moerae.conversation(uuid: &str)` -> Result<Conversation, ConversationError>; resumes existing conversation. Closes orphan open segments from prior sessions (state='closed'; usearch index validated by comparing node_count vs vector count, rebuilt from embeddings if missing or incomplete). Loads and caches segment list. Fresh open segments created lazily on next put.
- `moerae.delete_conversation(uuid: &str)` -> Result<(), ConversationError>; fails with ConversationError::InUse if active Conversation handles exist. Otherwise: invalidates LRU cache entries, deletes usearch index files, then deletes conversation row (segments and nodes cascade via SQL).
- `moerae.list_conversations()` -> Result<Vec<ConversationInfo>, ConversationError>; ConversationInfo: uuid, created_at, segment_count, node_count.
- `moerae.segment_stats()` -> Result<ProjectStats, ConversationError>; ProjectStats: total_segments, mismatched_segments, total_nodes.
- `moerae.rebuild_mismatched_segments()` -> Result<(), RebuildError>; user-triggered, blocks until complete. Creates temp index ({segment_id}.usearch.tmp), inserts all vectors, flushes, atomically renames to {segment_id}.usearch. Interrupted rebuilds leave .tmp files cleaned on next init.
- `moerae.get(node_id: i64)` -> Result<NodeContent, GetError>; fetches full original content for any node in the project. On project handle because project-scope search returns node_ids across conversations.
- `conv.put(data: &str, metadata: Option<&str>, persist: bool)` -> Result<(), PutError>; rejects empty or whitespace-only data (PutError::EmptyData). Deduplicates via content hash against current open segment; exact duplicate returns Ok(()) silently. Synchronous, ~5-15ms for ≤128 tokens. SQLite transaction wraps SQL insert + usearch insert; rollback on usearch failure. Creates open segment lazily if none exists. Refreshes cached segment list (may trigger close/open).
- `conv.search(query: &str, scope: Option<Scope>, limit: Option<usize>)` -> Result<SearchResults, SearchError>; rejects empty or whitespace-only queries (SearchError::EmptyQuery). Limit defaults to default_search_limit. Search hit relevancy updates: +1 hit_boost per distinct segment touched (not per node), wrapped in single SQLite transaction.
- `conv.search_debug(query: &str, scope: Option<Scope>, limit: Option<usize>)` -> Result<SearchResultsDebug, SearchError>; same validations as search (EmptyQuery, QueryTooLong). Separate method for type safety.

## Scope Model

- `pub enum Scope { Conversation, Project }`
- Conversation scope (default): query all segments belonging to current conversation_id.
- Project scope: query only promoted segments across all conversations. Promotion = curated subset, not full dump.
- Promotion trigger: segment relevancy ≥ promotion_threshold OR persist=true. Stored as `promoted` boolean in SQLite. Segments promoted only on close, not while open (prevents concurrency hazard on writable segments; staleness close mitigates delay for idle segments).
- No automatic scope widening; scope parameter is authoritative. Agent retries at wider scope manually if needed. Empty result vec is valid.
- Global scope (cross-project) deferred to v2.

## Search Return Types

- SearchResults: items (Vec<SearchResult>), has_more (bool), skipped_mismatched (usize). has_more = true when total candidates exceeded limit. skipped_mismatched = count of segments skipped due to model mismatch (0 in normal operation; nonzero signals agent to call rebuild).
- SearchResult: node_id (i64), data (String), metadata (Option<String>). For small nodes (≤ max_indexable_tokens): data = original content, metadata = optional summary if provided. For large nodes (> max_indexable_tokens): data = metadata summary (what was embedded), metadata = None. Predictable size. Full original content via moerae.get(node_id).
- SearchResultsDebug: items (Vec<SearchResultDebug>), has_more (bool), skipped_mismatched (usize).
- SearchResultDebug: node_id (i64), data (String), metadata (Option<String>), score (f32), segment_id (i64), conversation_id (String). Score is cosine similarity (0.0-1.0), converted from usearch cosine distance via `1.0 - (distance / 2.0)`.
- NodeContent (from moerae.get): data (String), metadata (Option<String>); always returns full original content regardless of size.

## Search Pipeline

- put = validate non-empty -> check duplicate hash in open segment -> begin SQLite transaction -> embed content -> insert node to SQLite -> insert vector to usearch -> commit. Rollback SQLite on usearch failure. No ghost entries.
- search = validate non-empty, validate query token count ≤ model max context (detected from loaded model at init; SearchError::QueryTooLong if exceeded) -> embed query -> usearch top-k per relevant segment (skip mismatched segments, count in skipped_mismatched) -> merge all candidates by cosine distance ascending (lower = better match) -> take top limit (set has_more if candidates exceeded limit) -> convert distances to similarity for return -> update relevancy: +1 hit_boost per distinct segment touched, single SQLite transaction.
- Conversation-scope search uses cached segment list. Project-scope search queries SQLite directly (SELECT segment_id FROM segments WHERE promoted = 1), bypasses cache.
- Ranking: usearch cosine distance only. Relevancy scoring is for lifecycle (eviction, promotion), not search-time ranking.
- One usearch index per segment; capacity set to segment_capacity at creation. One embedding per node; MetricKind::Cos set on every index creation (returns cosine distance: 0 = identical, 2 = opposite). usearch default M and ef_construction accepted; sufficient for ≤segment_capacity nodes. Explicit save/flush on segment close for durability.
- node_id (SQLite INTEGER PRIMARY KEY AUTOINCREMENT, i64 in Rust) as key in both usearch (cast to u64; safe since AUTOINCREMENT guarantees positive) and SQLite; globally unique within project.
- Content ≤ max_indexable_tokens: data embedded directly, metadata optional (stored in SQLite if provided).
- Content > max_indexable_tokens: metadata required and embedded, data stored in SQLite but not embedded; PutError::MetadataRequired if missing; PutError::MetadataTooLong if metadata itself exceeds max_indexable_tokens.
- LRU cache of open usearch index handles at project level; max_cached_indexes configurable. Cache entries invalidated on segment eviction or conversation deletion. Search never exceeds max_cached_indexes simultaneous file descriptors.

## Mandatory Metadata Rule

- Config: max_indexable_tokens (e.g. 128).
- Below threshold: data embedded directly, metadata optional.
- Above threshold: metadata required, only metadata embedded, data stored but NOT embedded; error if metadata missing (PutError::MetadataRequired) or if metadata itself exceeds threshold (PutError::MetadataTooLong).
- Reason: long raw data creates diffuse vectors = noise in index.
- Token counting uses embedding model's own tokenizer; already loaded with model; ~microseconds; exact count.

## Error Handling

- Per-call error enums using thiserror; Rust compiler enforces exhaustive matching.
- InitError: ModelNotFound(PathBuf), ModelLoadFailed(String), StorageInit(io::Error), InvalidConfig(String)
- ConversationError: NotFound(String), StorageRead(io::Error), StorageWrite(io::Error), InUse(String)
- PutError: EmptyData, MetadataRequired { token_count: usize }, MetadataTooLong { token_count: usize, max: usize }, EmbeddingFailed(String), StorageWrite(io::Error), PersistCapacityExceeded { current: usize, max: usize }
- SearchError: EmptyQuery, QueryTooLong { token_count: usize, max: usize }, EmbeddingFailed(String), IndexCorrupted(String)
- GetError: NotFound(i64), StorageRead(io::Error)
- RebuildError: EmbeddingFailed(String), StorageWrite(io::Error), NoMismatchedSegments

## Segment Lifecycle

- Two open segments per conversation: one regular, one persistence. persist=true puts route to persistence segment; regular puts route to regular segment. Prevents persist contaminating regular nodes. Open segments created lazily on first put of each type.
- On conversation resume: orphan open segments from prior sessions closed automatically (state='closed' in SQLite). usearch index validated by comparing SQLite node_count vs usearch vector count; rebuilt from embeddings if missing or incomplete.
- OPEN: accepting writes, fully searchable within own conversation. CLOSED: read-only, fully searchable; triggered at segment_capacity. Explicit usearch save/flush on close for durability. State transitions enforced by application code (Rust enum state machine); SQL CHECK validates values only.
- Staleness close: on put, if open segment's last_write is older than segment_staleness AND node_count > 0, close it first, then open fresh segment. If stale but empty (node_count == 0), update last_write_timestamp, keep open. Applies to regular and persistence segments independently.
- Eviction: full deletion. Sequence: invalidate LRU cache entries -> delete usearch index files from disk -> delete segment row (nodes cascade via SQL). Data permanently gone. Eviction candidate query explicitly filters persist=0. Composite priority = `time_since_last_hit_secs / ((relevancy_score + 0.01) * (hit_count + 1))`. When last_hit_timestamp is NULL (never searched), use created_at as reference time instead. Higher priority = evict first. Triggered on segment close when closed regular segment count > max_segments. Also runs on init via single grouped SQL query if segment counts exceed config.
- Persistence segments: never evicted; capped at max_persist_segments; PutError::PersistCapacityExceeded when exceeded.
- Relevancy tracked at segment level. On creation: relevancy = base_score. On persist=true segment: relevancy = 1.0. On search hit: +1 hit_boost per distinct segment touched per search (not per node); relevancy = min(relevancy + hit_boost, 1.0); hit_count += 1; last_hit = now(). Updates wrapped in single SQLite transaction.
- Promotion: segment relevancy ≥ promotion_threshold -> promoted flag set on close. Persist segments promoted on close. Promoted segments visible at project scope. No explicit demotion; eviction removes stale promoted non-persist segments.
- Embedding stored in both usearch index (primary) and SQLite nodes.embedding blob (backup for rebuild). ~3KB per node overhead; intentional tradeoff for fast index rebuild without re-embedding.
- Duplicate detection: content hash of (data, metadata) checked against current open segment before insert. Exact duplicate returns Ok(()) silently (idempotent put).

## Conflict Resolution

- No conflict detection, no supersession, no penalties.
- Ranking decides: cosine distance to query determines result order.
- Consistent search hits boost segment relevancy score.
- Outdated data eventually evicted through disuse.

## Persistence Format

- SQLite via rusqlite; one DB per project. Bootstrap order: create DB -> set WAL mode -> create tables -> load/store config.
- Layout: ~/.moerae/projects/{hex(sha256(project_id))[..16]}/moerae.db + indexes/{segment_id}.usearch
- Default model path: ~/.moerae/models/embeddinggemma-300m-qat-q8_0.gguf; no auto-download; ModelNotFound if absent.
- One usearch memory-mapped index file per segment; MetricKind::Cos.
- Orphan file cleanup: on init, scan indexes/ directory, delete .usearch and .usearch.tmp files not referenced by any segment row.
- Rebuild uses temp-then-rename: {segment_id}.usearch.tmp -> {segment_id}.usearch. Atomic replacement; interrupted rebuilds leave .tmp files cleaned on init.

## SQLite Schema

```sql
CREATE TABLE schema_version (id INTEGER PRIMARY KEY CHECK(id = 1), version INTEGER NOT NULL);
CREATE TABLE config (id INTEGER PRIMARY KEY CHECK(id = 1), data TEXT NOT NULL);  -- JSON blob; serde(default) for new fields
CREATE TABLE conversations (
    uuid TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL
);
CREATE TABLE segments (
    segment_id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT NOT NULL REFERENCES conversations(uuid) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK(state IN ('open', 'closed')),
    persist INTEGER NOT NULL DEFAULT 0,
    promoted INTEGER NOT NULL DEFAULT 0,
    relevancy_score REAL NOT NULL,
    hit_count INTEGER NOT NULL DEFAULT 0,
    node_count INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    last_hit_timestamp INTEGER,
    last_write_timestamp INTEGER NOT NULL,
    model_id TEXT NOT NULL
);
CREATE TABLE nodes (
    node_id INTEGER PRIMARY KEY AUTOINCREMENT,
    segment_id INTEGER NOT NULL REFERENCES segments(segment_id) ON DELETE CASCADE,
    data TEXT NOT NULL,
    metadata TEXT,
    embedding BLOB NOT NULL,
    content_hash BLOB NOT NULL  -- sha256 of (data, metadata) for duplicate detection
);
CREATE INDEX idx_segments_conversation ON segments(conversation_id);
CREATE INDEX idx_segments_promoted ON segments(promoted) WHERE promoted = 1;
CREATE INDEX idx_segments_model ON segments(model_id);
CREATE INDEX idx_nodes_segment ON nodes(segment_id);
CREATE INDEX idx_nodes_content_hash ON nodes(content_hash, segment_id);
```

- Crash recovery: SQLite ACID. usearch indexes rebuilt from stored embeddings on query failure; if embeddings also corrupt, segment marked unrecoverable and skipped. Orphan usearch files cleaned on init.
- Concurrency: SQLite WAL mode; read-write lock per usearch segment index. Single-process-per-project assumption; open segments writable only by owning conversation. Project-scope search only queries closed promoted segments (no concurrent read-write hazard).

## Default Embedding Model

- ggml-org/embeddinggemma-300m-qat-q8_0-GGUF. 329MB, Q8_0, 308M params, 768 dims. Confirmed working; broken conversion (llama.cpp #19040) was Unsloth-specific.
- Default path: ~/.moerae/models/embeddinggemma-300m-qat-q8_0.gguf. Custom override via builder. No auto-download; user downloads once manually.
- Integration test: embed known string, assert cosine similarity against hardcoded reference vector.
- Model loading blocks init(); one-time cost per process lifetime. Pre-loaded model handle sharing deferred to v2.

## Versioning and Migration

- SQLite schema_version table; sequential migrations baked into binary (v1->v2, v2->v3).
- Config: serde(default) on new fields; stored as JSON blob in SQLite config table; loaded on init, overridable via builder.
- Embedding model changes: model_id (name + dimensions + file_size + hex(sha256(first_4096_bytes))) tracked per segment. Fast check (stat + 4KB read). Index on segments.model_id for efficient mismatch detection. Mismatched segments detected on init (count via segment_stats). Search skips mismatched segments (counted in skipped_mismatched). Put only writes to current-model open segments. User calls rebuild_mismatched_segments() to re-embed mismatched segments; blocks until complete.

## Config Parameters

- max_indexable_tokens: threshold for mandatory metadata (e.g. 128)
- segment_capacity: nodes per segment before close (e.g. 100)
- segment_staleness: seconds since last write before auto-close on next put (e.g. 3600)
- max_segments: closed regular segments per conversation before eviction (e.g. 50)
- max_persist_segments: closed persistence segments per conversation; hard cap (e.g. 100)
- max_cached_indexes: LRU cache size for usearch index handles (e.g. 20)
- base_score: initial segment relevancy on creation (e.g. 0.3)
- hit_boost: relevancy increment on search hit (e.g. 0.1)
- promotion_threshold: relevancy score for auto-promotion to project scope (e.g. 0.7)
- default_search_limit: top-k results returned by search (e.g. 10)
- All validated on init: max_indexable_tokens > 0, segment_capacity > 0, segment_staleness > 0, max_segments > 0, max_persist_segments > 0, max_cached_indexes > 0, base_score in 0.0..=1.0, hit_boost > 0.0, promotion_threshold in 0.0..=1.0, default_search_limit > 0. InitError::InvalidConfig on failure.

## Crate Dependencies

- usearch: HNSW vector search; memory-mapped indexes; MetricKind::Cos; default M and ef_construction
- llama.cpp Rust bindings: GGUF model inference + tokenizer (specific crate chosen at implementation time)
- rusqlite: SQLite with WAL mode
- serde + serde_json: config serialization as JSON blob
- thiserror: error enum derives
- uuid: conversation UUID generation
- sha2: project_id hashing, content dedup hashing, model_id partial hash
- lru: LRU cache for usearch index handles

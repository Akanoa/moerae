---
type: bmad-distillate
sources:
  - "brainstorming-session-2026-04-08-0800.md"
downstream_consumer: "Rust implementation of Moerae system"
created: "2026-04-08"
token_estimate: 2854
parts: 1
---

## Architectural Simplifications

- Conversation boundaries eliminated; conversations never end, just go quiet. All lifecycle triggers replaced with activity-based timestamps and hit tracking.
- Segment states reduced to OPEN/CLOSED + LRU eviction. Closed segments on disk (memory-mapped via usearch). Eviction by composite score when segment count exceeds max_segments.
- Internal graph eliminated. HNSW in usearch provides neighbor connections on insert, top-k with guaranteed termination, search-time routing. Co-access shortcuts and cross-conversation edges deferred to future work.
- One usearch index per segment; one embedding per node. Content ≤ max_indexable_tokens: embed data directly. Content > max_indexable_tokens: embed metadata only, data stored in SQLite but not embedded.
- Tokio dropped; put is synchronous. Crate dependencies: usearch, llama.cpp Rust bindings (GGUF loading + embedding mode + tokenizer required; specific crate chosen at implementation time), rusqlite, serde, thiserror.
- Global scope (cross-project search) deferred to v2. V1 scopes: Conversation, Project.
- No automatic scope widening; scope parameter is authoritative. Agent retries at wider scope manually if needed. Empty result vec is valid.
- Eliminated concepts: ARCHIVED/DEAD states, k-means centroids, i8 quantized vectors, hints mechanism, blast radius walk, edge weaving, graph persistence, async write pipeline, dual-layer indexing, iterative deepening, weighted ranking formula. Eliminated configs: connection_threshold, max_neighbors, archive_after, dead_after, centroid_clusters, archive_threshold.

## API Design

- Three-level API: init() returns project handle; create_conversation() returns conversation handle with UUID; put/search called on conversation handle.
- `moerae::init(project_id: &str)` -> Result<Moerae, InitError>; loads config from SQLite if exists, otherwise uses defaults.
- `moerae::init_with_config(project_id: &str, config: Config)` -> Result<Moerae, InitError>; overrides stored config.
- `moerae::init_with_model(project_id: &str, model_path: &Path)` -> Result<Moerae, InitError>; custom embedding model.
- `moerae.create_conversation()` -> Result<Conversation, ConversationError>; generates UUID internally; stored in SQLite. Caches segment list in memory.
- `moerae.conversation(uuid: &str)` -> Result<Conversation, ConversationError>; resumes existing conversation. Loads and caches segment list.
- `moerae.list_conversations()` -> Result<Vec<ConversationInfo>, ConversationError>; ConversationInfo: uuid, created_at, segment_count, node_count.
- `moerae.segment_stats()` -> Result<ProjectStats, ConversationError>; ProjectStats: total_segments, mismatched_segments, total_nodes.
- `moerae.rebuild_mismatched_segments()` -> Result<(), RebuildError>; user-triggered, blocks until complete.
- `conv.put(data: &str, metadata: Option<&str>, persist: bool)` -> Result<(), PutError>; refreshes cached segment list after put (may trigger close/open).
- `conv.search(query: &str, scope: Option<Scope>, limit: Option<usize>)` -> Result<Vec<SearchResult>, SearchError>; limit defaults to default_search_limit. Uses cached segment list.
- `conv.search_debug(query: &str, scope: Option<Scope>, limit: Option<usize>)` -> Result<Vec<SearchResultDebug>, SearchError>; separate method for type safety.
- `conv.get(node_id: u64)` -> Result<NodeContent, SearchError>; fetches full original content for metadata-only nodes. NodeContent: data (String), metadata (Option<String>).
- Two data verbs (put, search) preserved at conversation level; create_conversation/list/stats/rebuild/get are setup, maintenance, and retrieval.

## Scope Model

- `pub enum Scope { Conversation, Project }`
- Conversation scope (default): query segments belonging to current conversation_id (both regular and persistence).
- Project scope: query only promoted segments across all conversations. Promotion = curated subset, not full dump.
- Promotion trigger: segment relevancy ≥ promotion_threshold OR persist=true. Stored as `promoted` boolean in SQLite. Project scope search = WHERE promoted = true.
- Demotion: no explicit demotion. Eviction serves as removal for non-persist promoted segments whose time_since_last_hit grows large enough. Persist segments are exempt from eviction by design.

## Search Return Types

- SearchResult: data (String), metadata (Option<String>); data contains the embedded content (metadata summary for large nodes, raw data for small nodes). Predictable size.
- SearchResultDebug: data (String), metadata (Option<String>), score (f32), segment_id (String), conversation_id (String), node_id (u64)
- NodeContent (from conv.get): data (String), metadata (Option<String>); always returns full original content regardless of size.

## Search Model

- put = embed content -> usearch insert -> assign to open segment -> return (synchronous, ~5-15ms for ≤128 tokens).
- search = embed query -> usearch top-k per relevant segment -> merge all candidates by cosine similarity descending -> take top limit. Cosine similarity scores are comparable across segments (same model, same metric, normalized vectors).
- Ranking: usearch cosine similarity only. Relevancy scoring is for lifecycle (eviction, promotion), not search-time ranking. No weighted ranking formula.
- One usearch index per segment; one embedding per node; node_id (SQLite INTEGER PRIMARY KEY AUTOINCREMENT) as key in both usearch and SQLite. Globally unique within project.
- Content ≤ max_indexable_tokens: data embedded directly, metadata optional (stored in SQLite if provided).
- Content > max_indexable_tokens: metadata required and embedded, data stored in SQLite but not embedded; PutError::MetadataRequired if missing. Search returns metadata in data field; full content via conv.get(node_id).

## Error Handling

- Per-call error enums using thiserror; Rust compiler enforces exhaustive matching.
- InitError: ModelNotFound(PathBuf), ModelLoadFailed(String), StorageInit(io::Error)
- ConversationError: NotFound(String), StorageRead(io::Error), StorageWrite(io::Error)
- PutError: MetadataRequired { token_count: usize }, EmbeddingFailed(String), StorageWrite(io::Error), ModelMismatch { segment_id: String }, PersistCapacityExceeded { current: usize, max: usize }
- SearchError: EmbeddingFailed(String), IndexCorrupted(String), ModelMismatch { segment_id: String }
- RebuildError: EmbeddingFailed(String), StorageWrite(io::Error), NoMismatchedSegments

## Default Embedding Model

- ggml-org/embeddinggemma-300m-qat-q8_0-GGUF confirmed working. Broken conversion (llama.cpp #19040) was Unsloth-specific; ggml-org QAT variant unaffected. 329MB, Q8_0, 308M params, 768 dims.
- Custom model override via init_with_model(). Integration test: embed known string, assert cosine similarity against hardcoded reference vector.

## Token Counting

- Use embedding model's own tokenizer for max_indexable_tokens threshold; already loaded with model; ~microseconds; exact count.

## Segment Lifecycle

- Two open segments per conversation: one regular, one persistence. persist=true puts route to persistence segment; regular puts route to regular segment. Prevents persist contaminating regular nodes.
- OPEN: accepting writes, fully searchable. CLOSED: read-only, fully searchable; triggered at segment_capacity.
- Staleness close: on put, if open segment's last_write is older than segment_staleness, close it first, then open fresh segment. Applies to both regular and persistence segments independently. May produce single-node segments; acceptable tradeoff (small usearch index, eviction caps count).
- Eviction: composite priority = `time_since_last_hit_secs / ((relevancy_score + 0.01) * (hit_count + 1))`. Higher = evict first. Triggered on segment close when closed segment count > max_segments. Persistence segments never evicted.
- Persistence segments capped at max_persist_segments; PutError::PersistCapacityExceeded when exceeded.
- Relevancy tracked at segment level. On creation: relevancy = base_score. On persist=true segment: relevancy = 1.0. On search hit touching any node in segment: relevancy = min(relevancy + hit_boost, 1.0); hit_count += 1; last_hit = now().
- Promotion: segment relevancy ≥ promotion_threshold -> promoted flag set. Persist segments promoted immediately. Promoted segments visible at project scope. No explicit demotion; eviction removes stale promoted segments.

## Persistence Format

- SQLite via rusqlite; one DB per project; WAL mode for concurrent reads during writes. One usearch memory-mapped index file per segment.
- Layout: ~/.moerae/projects/{project_id}/moerae.db + indexes/{segment_id}.usearch
- SQLite stores: node content, node metadata, node embeddings (blobs for rebuild), segment state (OPEN/CLOSED, hit_count, relevancy_score, last_hit_timestamp, last_write_timestamp, node_count, persist flag, promoted flag, model_id), scope assignment (conversation_id). Config stored in SQLite config table.
- node_id: SQLite INTEGER PRIMARY KEY AUTOINCREMENT; globally unique within project; used as usearch label.
- Crash recovery: SQLite ACID. usearch indexes rebuilt from stored embeddings on query failure; if embeddings also corrupt, segment marked unrecoverable and skipped.
- Concurrency: SQLite WAL mode; read-write lock per usearch segment index. Single-process-per-conversation assumption; conversation handle caches segment list, refreshed on put.

## Versioning and Migration

- SQLite schema_version table; sequential migrations baked into binary.
- Config: serde(default) on new fields; stored in SQLite; loaded on init, overridable via init_with_config.
- Embedding model changes: model_id (name + dimensions + file_size) tracked per segment. Fast to check (stat call). Mismatched segments detected on init (count via segment_stats). put/search on mismatched segments return ModelMismatch error. User calls rebuild_mismatched_segments() explicitly; blocks until complete.

## Consolidated Config Parameters

- max_indexable_tokens: threshold for mandatory metadata (e.g. 128)
- segment_capacity: nodes per segment before close (e.g. 100)
- segment_staleness: seconds since last write before auto-close on next put (e.g. 3600)
- max_segments: closed regular segments per conversation before eviction (e.g. 50)
- max_persist_segments: closed persistence segments per conversation; hard cap (e.g. 100)
- base_score: initial segment relevancy on creation (e.g. 0.3)
- hit_boost: relevancy increment on search hit (e.g. 0.1)
- promotion_threshold: relevancy score for auto-promotion to project scope (e.g. 0.7)
- default_search_limit: top-k results returned by search (e.g. 10)

## Resolution Summary

- Four adversarial review passes resolved all findings through targeted decisions and architectural simplification.
- Key simplifications: no conversation boundaries (activity-based lifecycle), no internal graph (usearch HNSW sufficient), one embedding per node (no dual-layer), synchronous put, OPEN/CLOSED + eviction, vector similarity as sole ranking, manual scope control.
- Preserved from original: two segment types (regular + persistence), scope hierarchy (conversation + project), mandatory metadata rule for large content, relevancy-based promotion.

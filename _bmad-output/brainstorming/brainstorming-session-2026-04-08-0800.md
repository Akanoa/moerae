---
stepsCompleted: [1, 2, 3]
inputDocuments:
  - "_bmad-output/brainstorming/brainstorming-session-2026-04-07-0800-distillate/"
  - "_bmad-output/brainstorming/brainstorming-session-2026-04-07-0800-distillate/adversarial-review.md"
session_topic: 'Resolve all 12 adversarial review findings against Moerae distillate for direct Rust implementation'
session_goals: 'Produce concrete, implementable design decisions for each finding — types, formats, algorithms, thresholds, boundaries — so no design questions remain during implementation'
selected_approach: 'ai-recommended'
techniques_used: ['morphological-analysis']
ideas_generated: [12]
technique_execution_complete: true
context_file: ''
---

# Brainstorming Session Results

**Facilitator:** Noa
**Date:** 2026-04-08

## Session Overview

**Topic:** Resolve all 12 adversarial review findings against Moerae architecture distillate
**Goals:** Each finding gets a firm, implementable design decision — precise enough for direct Rust implementation with zero ambiguity

### Context Guidance

_Source: Moerae distillate (4 parts: core architecture/API, segment lifecycle/memory, search/write pipelines, tech stack/implementation) + adversarial review (12 findings). Session resolves every gap so the Rust codebase can be built without further design stops._

### Session Setup

_Noa wants to close all 12 adversarial findings with concrete decisions. Output will serve as an amendment to the original distillate, enabling direct implementation._

## Technique Selection

**Approach:** AI-Recommended Techniques
**Analysis Context:** Adversarial finding resolution with focus on implementation-ready design decisions

**Technique Used:** Morphological Analysis — systematically enumerate options per finding, evaluate tradeoffs, select best combination. First principles thinking and stress-testing emerged naturally within the morphological analysis rather than requiring separate phases.

## Technique Execution Results

### Morphological Analysis — Adversarial Finding Resolution

All 12 findings resolved through systematic option enumeration and collaborative decision-making. Three major architectural simplifications emerged during the session that eliminated multiple findings simultaneously.

---

### Architectural Simplification 1: Conversation Boundaries Don't Exist

**Insight:** A conversation is never finished — it just goes quiet and may resume. There is no natural "end" event. The API must stay at two verbs: `put` and `search`. No `end_conversation()`.

**Impact:** All lifecycle triggers that depended on "conversation boundary" (score freezing, archival countdown, death countdown, batch promotion) were replaced with activity-based mechanisms using timestamps and hit tracking that the system already observes through its own two verbs.

**Findings resolved:** #2 (conversation boundary undefined)

### Architectural Simplification 2: Segment States Reduced to OPEN/CLOSED + LRU Eviction

**Insight:** The ARCHIVED and DEAD states existed to save memory through centroid compression, then eventual removal. But closed segments live on disk (memory-mapped via usearch) — memory pressure is not the real constraint. Disk is cheap. The real cost of keeping segments is search time (more indexes to query). Eviction should be driven by relevancy and access patterns, not arbitrary state progression.

**New lifecycle:**
- **OPEN** — accepting writes, fully searchable
- **CLOSED** — read-only, fully searchable (triggered at `segment_capacity`)
- **Eviction** — not a state, just removal. Composite score: segments with low relevancy, low hit count, and old last-hit timestamp are evicted first when segment count exceeds `max_segments`

**Eliminated concepts:** ARCHIVED state, DEAD state, k-means centroids, i8 quantized vectors, centroid ghost search path, hints mechanism, `archive_after`, `dead_after`, `centroid_clusters` configs, conversation boundary counting, score freezing.

**Findings resolved:** #4 (k-means hand-waved), #11 (hints leak info)

### Architectural Simplification 3: Internal Graph Eliminated

**Insight:** The internal Moerae graph (edge weaving, blast radius expansion, metadata↔data edges, topology-as-richness) duplicates what usearch's HNSW algorithm already provides. HNSW connects vectors to nearest neighbors on insert, returns top-k with guaranteed termination, and handles all search-time routing internally. 5 of 6 graph features were redundant with usearch. The remaining two (co-access shortcuts, cross-conversation edges) were the least specified and most complex — both flagged in the adversarial review as hand-waved.

**New search model:**
- **Put** = embed content → usearch insert → assign to open segment
- **Search** = embed query → usearch top-k → return content
- **Dual-layer indexing** = two usearch indexes (metadata, data) instead of graph edges

**Eliminated concepts:** Custom graph data structure, edge weaving at write time, blast radius walk, cross-conversation edge creation, graph persistence, `connection_threshold`, `max_neighbors` configs.

**Findings resolved:** #5 (blast radius no termination), #8 (cross-conversation edges underspecified)

---

### Finding #1: Error Handling — Per-Call Error Enums

**Decision:** Per-call error enums using `thiserror` crate. Each API call returns only the error variants that can actually occur for that call. Rust compiler enforces exhaustive matching.

```rust
enum InitError {
    ModelNotFound(PathBuf),
    ModelLoadFailed(String),
    StorageInit(io::Error),
}

enum PutError {
    MetadataRequired { token_count: usize },
    EmbeddingFailed(String),
    StorageWrite(io::Error),
}

enum SearchError {
    EmbeddingFailed(String),
    IndexCorrupted(String),
}
```

### Finding #2: Conversation Boundary — Eliminated

**Decision:** No conversation boundary concept. Lifecycle driven by activity-based timestamps. Scores never freeze — always live and boostable. Promotion happens immediately when threshold crossed. See Architectural Simplification 1.

### Finding #3: Default Embedding Model — Verified Working

**Decision:** Default model is `ggml-org/embeddinggemma-300m-qat-q8_0-GGUF`. The broken conversion issue (llama.cpp #19040) was specific to Unsloth GGUF conversions missing sentence-transformer dense layers — the ggml-org QAT variant is unaffected, actively maintained, 74k downloads/month, no known issues. 329MB, Q8_0 quantization, 308M params, 768 dimensions.

No fallback model needed. `InitError::ModelNotFound` and `InitError::ModelLoadFailed` cover the edge case. Custom model override via `init_with_model()` for power users. One integration test at build time: embed a known string, assert cosine similarity against hardcoded reference vector.

### Finding #4: K-means at Archive Time — Eliminated

**Decision:** No archival, no k-means, no centroids. See Architectural Simplification 2.

### Finding #5: Blast Radius Termination — Eliminated

**Decision:** No blast radius expansion. usearch top-k with guaranteed termination replaces custom graph walk. See Architectural Simplification 3.

### Finding #6: Async Write Consistency Gap — Eliminated

**Decision:** `put` is synchronous. Embed → insert → assign to segment → return. Single embed call for ≤128 tokens on embeddinggemma-300m is ~5-15ms — invisible to an agent. Batch optimization can be added later as an internal detail. Two-verb API preserved. No consistency gap. No flush verb.

### Finding #7: Relevancy Scoring Unbounded — Capped 0.0–1.0

**Decision:** Relevancy score normalized to 0.0–1.0 range.
- On put: `relevancy = base_score` (e.g., 0.3)
- On put+persist: `relevancy = 1.0`
- On search hit: `relevancy = min(relevancy + hit_boost, 1.0)`
- Stored as f32. No quantization needed.
- Persist=true starts at cap — maximum importance, can't be outranked by accumulated hits.

### Finding #8: Cross-Conversation Edge Weaving — Eliminated

**Decision:** No custom graph, no edge weaving. See Architectural Simplification 3. Co-access tracking and cross-conversation bridges deferred to future work once core proves itself.

### Finding #9: Persistence Format — SQLite + usearch Files

**Decision:** SQLite via `rusqlite` crate. One database per project. usearch memory-mapped index files alongside.

```
~/.moerae/
  projects/
    {project_id}/
      moerae.db                    # SQLite: nodes, segments, config
      indexes/
        {segment_id}.usearch       # usearch memory-mapped index files
```

SQLite stores: node content, node metadata, node embeddings (blobs, for rebuild), segment state (OPEN/CLOSED, hit_count, relevancy_score, last_hit_timestamp, node_count), scope assignment (conversation, project). Crash recovery inherited from SQLite ACID guarantees. usearch indexes rebuilt from stored embeddings if corrupted.

### Finding #10: Token Counting — Model's Own Tokenizer

**Decision:** Use the embedding model's own tokenizer for `max_indexable_tokens` threshold. Tokenizer is already loaded as part of the model. Tokenization without inference is ~microseconds. Exact token count, no heuristic drift. If model didn't load, `InitError::ModelLoadFailed` already covers it.

### Finding #11: Hints Mechanism Leaks Info — Eliminated

**Decision:** No hints mechanism. No archived segments to hint about. See Architectural Simplification 2.

### Finding #12: Versioning and Migration — Schema Versioning + Segment-Level Model Tagging

**Decision:** Three-layer versioning strategy:

**SQLite schema:** `schema_version` table in every `moerae.db`. On startup, read version, run sequential migrations if behind (`v1→v2`, `v2→v3`, ...). Migrations are SQL statements baked into the binary.

**Config:** `serde(default)` on new fields. Old configs load fine, new fields get defaults. No migration needed.

**Embedding model changes:** Model coherence tracked at segment level. Each segment tagged with `model_id` (name + dimensions). New puts always go to segments with current model. Mismatched segments re-embedded on-demand when search touches them — content is in SQLite, re-embed and rebuild usearch index for that segment. Power-user feature; opens the door for future evolution without forcing upfront migration cost.

---

### Creative Facilitation Narrative

_Session began as systematic morphological analysis but quickly evolved into first-principles questioning that challenged core architectural assumptions. Three major simplifications emerged organically: eliminating conversation boundaries (replacing with activity-based lifecycle), collapsing segment states to OPEN/CLOSED + eviction (eliminating k-means, hints, and multi-state progression), and dropping the internal graph entirely (usearch HNSW handles search-time routing). These simplifications cascaded — each one eliminated multiple adversarial findings simultaneously. What started as 12 discrete gaps became 3 architectural insights + 5 targeted decisions._

### Session Highlights

**Key Breakthrough:** The internal graph was conceived early in the original brainstorming before usearch was validated. Once HNSW handles search-time routing, 5 of 6 graph features are redundant. Eliminating it removed the most complex and underspecified parts of the architecture.

**Simplification Score:** 12 findings resolved. 3 resolved by targeted decisions. 9 resolved by eliminating the features that caused them. Net effect: fewer concepts, fewer configs, fewer code paths, same functionality.

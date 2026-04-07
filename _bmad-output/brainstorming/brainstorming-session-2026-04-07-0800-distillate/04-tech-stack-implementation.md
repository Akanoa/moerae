This section covers tech stack and implementation decisions. Part 4 of 4 from brainstorming-session-2026-04-07-0800.md.

## Language: Rust (Direct Implementation, No Prototype)

- Decision: Rust, direct implementation; no Python prototype phase
- Reason: compiler as LLM quality gate; LLM-generated code benefits from exhaustive compile errors creating tight feedback loop; if it compiles, it's memory-safe and likely correct
- Rejected: Python first. Reason: LLM-generated Python can produce subtly buggy code (wrong state transitions, race conditions, silent segment corruption) found only at runtime
- Rejected: C. Reason: maximum footprint for bugs in segment/lifecycle logic; no built-in async, no memory safety
- First-class async via Tokio maps naturally to async write pipeline
- Memory safety without GC critical for segment lifecycle, scope transitions, complex state management
- Single binary deployment, no runtime dependency
- Architecture spec is already complete; prototype unnecessary for validation

## Crate Dependencies

- usearch: HNSW vector search
- llama-cpp-rs: GGUF model inference
- tokio: async runtime (write pipeline)
- serde: config serialization

## Embedding Model

- Default: `ggml-org/embeddinggemma-300m-qat-q8_0-GGUF`
- embeddinggemma-300m: 308M params, 768 dims, MTEB v2 rank #23, retrieval score 62.49 (nearly 2x alternatives)
- Known issue: unsloth GGUF conversion broken (missing 2 dense layers); QAT variant from ggml-org may fix — needs verification
- Configurable override: any GGUF model via init parameter; system auto-detects embedding dimensions from model output
- Moerae only indexes short text (<=128 tokens by design) but model earns its 300M size via benchmark dominance

## MTEB v2 Benchmark Context

- embeddinggemma-300m: 308M/768d, retrieval=62.49
- mxbai-embed-large-v1: 335M/1024d, retrieval=40.30
- nomic-embed-text-v1.5: 137M/768d, retrieval=34.09
- bge-small-en-v1.5: 33M/512d, retrieval=36.26
- all-MiniLM-L6-v2: 23M/384d, retrieval=32.51

## usearch Features to Leverage

- Quantized vectors: f32 (active), f16, i8 (archived centroids = 4x smaller)
- Memory-mapped indexes for closed segments on disk
- Index merging for project-scope operations
- HNSW scales O(log n); 5000+ nodes still fast

## Vector Search: usearch Validated

- Survives all constraints: no external DB, lightweight, embeddable
- Alternative considered: lancedb (stores vectors + payloads together) but adds dependency weight

## Configuration Parameters

- `max_indexable_tokens`: threshold for mandatory metadata (e.g., 128)
- `segment_capacity`: nodes per segment before close
- `archive_after`: conversation boundaries before archiving
- `dead_after`: boundaries after archive before death
- `centroid_clusters`: k for k-means on archive (e.g., 3-5)
- `connection_threshold`: minimum similarity for edge creation
- `max_neighbors`: safety cap on edges per node
- `base_score`: initial relevancy on put (e.g., 0.3)
- `hit_boost`: relevancy increment on search hit (e.g., 0.1)
- `promotion_threshold`: relevancy score for auto-promotion
- `archive_threshold`: relevancy score below which node contributes to segment archival

## Init API

- `moerae::init()` — default model, zero config
- `moerae::init_with_model("path/to/custom.gguf")` — power user override

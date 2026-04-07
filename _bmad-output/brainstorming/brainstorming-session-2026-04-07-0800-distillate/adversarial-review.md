---
type: adversarial-review
target: brainstorming-session-2026-04-07-0800-distillate
created: 2026-04-08
---

## Adversarial Review — Moerae Architecture Distillate

1. No error handling specification. API defines put returning "success/error" but no error types, structure, or guidance. Embedding failure, missing model file, full disk during segment close — all undefined. Rust implementation needs exhaustive error types.

2. "Conversation boundary" never defined. Entire lifecycle hinges on it (archive_after, dead_after, score freezing, promotion) but no definition of what constitutes one, who signals it, or how detected. Explicit API call? Inactivity timeout? Process termination? Segment lifecycle unimplementable without this.

3. Default embedding model has known broken GGUF conversion. QAT variant "may fix" and "needs verification." Implementer's first task is debugging third-party model bug. No fallback model specified if verification fails.

4. K-means at archive time is hand-waved. Iterative computation over potentially hundreds of vectors. No crate specified. Computational cost undefined. Behavior when segment has fewer nodes than k is unspecified.

5. Blast radius expansion has no termination guarantee. Content budget referenced but never defined or configured. Dense graphs could expand to consume entire graph. No hard cap.

6. Async write pipeline creates observable consistency gap with no mitigation. Put-then-immediately-search misses are acknowledged but dismissed as "agents don't typically do this." No mechanism for synchronous wait when needed.

7. Relevancy scoring arithmetic has no bounds. hit_boost of 0.1 with no ceiling means unbounded scores. Relationship between accumulated score and promotion_threshold undefined. Configurable weights need coordination to avoid nonsensical ranking.

8. Cross-conversation edge weaving has no concrete trigger or algorithm. "Detect semantic proximity" — how? What threshold? Creates edges during search, contradicting search-as-read-only principle. Latency impact unaddressed.

9. No persistence format or storage layout. Segments, nodes, edges, metadata — none have defined disk format, directory structure, or durability guarantees. Crash recovery undefined.

10. Token counting for max_indexable_tokens assumes a tokenizer but none specified. Model tokenizer (expensive, requires loaded model) vs heuristic (cheap, inaccurate). For a threshold that rejects data, precision matters.

11. Hints mechanism leaks more than claimed. Integer count of wider-scope matches is information — knowing 47 matches exist for "database credentials" in project scope reveals something about other conversations. Privacy model weaker than stated.

12. No versioning or migration strategy. Config parameters, segment format, graph structure will evolve. No schema versioning concept. Upgraded Moerae instances face incompatible segments with no migration path.

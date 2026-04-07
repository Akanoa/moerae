---
type: bmad-distillate
sources:
  - "../brainstorming-session-2026-04-07-0800.md"
downstream_consumer: "Rust implementation of Moerae system"
created: "2026-04-07"
token_estimate: 2100
parts: 5
---

## Moerae Distillate — Root

Moerae: queryable hierarchical AI memory system replacing linear file indirection. Single search call returns relevant content at right detail level. Language: Rust. Two-verb API: put/search.

## Cross-Cutting Principles

- Search quality never degrades from delivery constraints; search and delivery are independent budgets
- No time-based decay; relevancy is access-score + freshness rank
- Ranking resolves conflicts; no conflict detection logic
- Write contention is a non-problem: single-threaded per conversation, append-only project graph, batch promotion
- Ghost/forgetting operates at segment level, not per-node
- Agent never needs to know about internal lifecycle (ghosts, segments); API is just put/search

## Section Manifest

- [01-core-architecture-api.md](01-core-architecture-api.md) — API contract, scope hierarchy, dual-layer indexing, blast radius, graph topology
- [02-segment-lifecycle-memory.md](02-segment-lifecycle-memory.md) — Segment states, archival, persistence, relevancy scoring, centroid ghosts
- [03-search-write-pipelines.md](03-search-write-pipelines.md) — Search pipeline, write pipeline, effort model, routing strategies, co-access tracking
- [04-tech-stack-implementation.md](04-tech-stack-implementation.md) — Rust rationale, crate dependencies, embedding model, usearch, llama.cpp, config parameters

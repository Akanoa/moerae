This section covers search and write pipelines. Part 3 of 4 from brainstorming-session-2026-04-07-0800.md.

## Search Pipeline

- Embed query (one call, ~5ms)
- usearch nearest-neighbor across conversation segments (microseconds)
- Blast radius expansion: edge lookups, no computation
- If insufficient: scope widen (repeat search on larger index)
- Return clean data response (default) or debug internals

## Effort Model (Hybrid: Specificity + Iterative Deepening)

- Query specificity sets initial effort budget: embedding entropy/variance against cluster centroids; high spread=vague=stay broad; tight clustering=precise=go deep
- Iterative deepening adjusts: start at minimum effort; if result below relevance threshold, escalate (add hop, engage heavier routing)
- Two axes: depth within graph AND breadth across scopes
- Rejected: token-budget-as-effort; creates perverse incentive where agent needing memory most gets worst results

## Effort Stack

- Low: embed + search current conv index (~5ms)
- Medium: same + blast radius expansion (~5ms + lookups)
- High: same + search project scope index (~10ms)
- Max: same + search global scope (~15ms)
- Expensive part is always embedding call, done exactly once

## Routing Strategies

- Heaviest-edge-first: O(1) per hop; context-blind; best for known/frequent paths
- Full re-rank: O(k * embed_cost) per hop; context-aware; best for vague/novel queries
- Query-projected weight modulation: O(k * dot_product) per hop; partially context-aware; best for most queries; similar to HNSW internal routing
- Leaf-level scan with shortcut entry: O(1) + O(n) scan; context-aware at entry only; best for precise retrieval
- Effort level can select routing strategy
- Key insight: usearch HNSW handles all search-time routing internally; Moerae graph adds value at write time (edge weaving, lifecycle) and post-search (blast radius), not during search

## Write Pipeline (Async)

- `put` returns immediately to agent (~0ms perceived)
- Background: embed -> threshold check (metadata required?) -> usearch insert -> weave edges to neighbors -> searchable ~10-50ms after put
- Batch embedding for write bursts: 20 texts batched = ~150ms vs 20 * 50ms = 1s sequential

## Edge Weaving at Write Time

- On put: system embeds content, finds similar existing nodes, creates weighted edges automatically
- Agent never describes relationships; graph builds itself
- Adaptive neighbor count: connect to neighbors above `connection_threshold`; stop when similarity drops
- Concrete facts get few edges; conceptual topics get many; bushiness emerges automatically
- Safety cap: `max_neighbors`

## Co-Access Tracking

- Track by node IDs hit, not query strings (robust to query variation)
- Rolling buffer of ~20 recent search hits with timestamps
- Scan on each search for temporal proximity patterns; O(buffer_size) = negligible
- When agent searches A then B repeatedly in temporal proximity, shortcut edge forms
- Co-access shortcuts computed asynchronously after search, stored as regular edges
- Zero extra search-time cost; system gets smarter without getting slower

## Clean Response Design

- Default response: relevant data payloads only; no scores, metadata, source tags
- Reason: metadata is deceptive noise consuming agent context
- Debug flag exposes full internals when needed
- Hints as separate channel: single integer count of wider-scope matches, minimal footprint

This section covers core architecture and API contract. Part 1 of 4 from brainstorming-session-2026-04-07-0800.md.

## API Contract

- `put(data, metadata?, persist?)` — store content; returns success/error
  - persist=false (default): routes to regular segment
  - persist=true: routes to persistence segment, relevancy=1.0
- `search(query, scope?, debug?)` — retrieve content
  - Default: returns clean data array only (no scores, no metadata, no source tags)
  - debug=true: returns full internals (scores, sources, segment info)
  - hints: integer count of wider-scope matches when current scope empty/below threshold
- No delete verb; lifecycle handles forgetting

## Scope Hierarchy

- Three levels: conversation (default) < project < global
- Memory partitioned as (project, conversation) tuple
- Search defaults to current conversation scope
- Agent explicitly controls boundary crossing via scope parameter:
  - `search(query)` — current conversation
  - `search(query, scope="project")` — all conversations in project
  - `search(query, scope="global")` — cross-project; agent must explicitly request
- System never silently leaks data across projects
- Scope widening plugs into iterative deepening: conversation miss -> project -> global
- Results tagged with origin scope
- Hints surface match existence in wider scopes without revealing content

## Dual-Layer Indexing

- `put(data, metadata?)` — metadata is optional agent-provided summary
- System embeds metadata and data independently
- Metadata embeddings form coarse navigation layer; data embeddings form fine-grained layer
- Search hits metadata first (broad), then data (precise)
- Agent creates hierarchy by choosing what to summarize
- Graph edge types that emerge:
  - Metadata<->Metadata: hub-to-hub conceptual map
  - Data<->Data: leaf-to-leaf lateral links (B+Tree-like)
  - Metadata->Data: vertical parent-child

## Mandatory Metadata Rule

- Config: `max_indexable_tokens` (e.g., 128)
- Below threshold: data embedded directly, metadata optional
- Above threshold: metadata required, only metadata embedded, data stored but NOT embedded; error if metadata missing
- Reason: long raw data creates diffuse vectors = noise in index

## Graph Topology

- Weighted graph, not strict tree; nodes have varying connectivity
- Node connectivity degree encodes information richness (conceptual=bushy, concrete=sparse)
- Leaf-level connectivity allows traversal without climbing back up (B+Tree analogy)
- Edge weights encode relationship strength, co-access frequency, semantic proximity
- Hierarchy emerges from topology; no explicit layer definitions
- Entry via embedding similarity; navigation via topology

## Blast Radius

- Every search hit returns a neighborhood: direct hit + connected nodes above weight threshold
- Expansion: walk outward collecting nodes weighted by edge_weight x query_similarity
- Stops when: edge weights drop, content budget reached, or no more relevant neighbors

## Conflict Resolution

- No conflict detection, no supersession, no penalties
- Ranking decides: freshness tiebreaker within conversation; newer conversation ranks higher cross-conversation
- Consistent search hits boost correct version's access score
- Outdated data eventually archives through disuse
- Reason: start simple; add agent control later if needed

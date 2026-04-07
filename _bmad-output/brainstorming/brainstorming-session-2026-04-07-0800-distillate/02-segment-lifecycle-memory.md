This section covers segment lifecycle and memory management. Part 2 of 4 from brainstorming-session-2026-04-07-0800.md.

## Segment Architecture (Replaces Per-Node Ghosts)

- Segment is the atom of memory management, not the node
- Segments are thematically cohesive (built during same working context)
- Two segment types:
  - REGULAR: OPEN -> CLOSED -> ARCHIVED -> DEAD
  - PERSISTENCE: OPEN -> CLOSED (permanent, never archive, never die)
- `persist=true` routes node to persistence segment; regular `put` routes to regular segment; no duplication

## Segment States

- OPEN: read/write, one per type per conversation; accepts new puts
- CLOSED: read-only, fully searchable, all node embeddings present; triggered when node_count reaches `segment_capacity`
- ARCHIVED: 3-5 cluster centroids replace full embeddings; returns hints only; ~1% of full size; centroids computed via k-means (k=`centroid_clusters`, e.g. 3-5)
- DEAD: nothing in index, removed entirely

## Segment Lifecycle Transitions

- OPEN -> CLOSED: node count reaches `segment_capacity`
- CLOSED -> ARCHIVED: no search hits across `archive_after` conversation boundaries
- ARCHIVED -> DEAD: no centroid hits across `dead_after` more boundaries
- Any state stays alive: search hits reset countdown
- Segment tracked with: creation_conversation, last_search_hit, hit_count, node_count

## Archived Segment Centroids

- K-means clustering on segment embeddings produces 3-5 centroids
- Each centroid = mean of cluster; preserves topical resolution at ~1% size
- Active embeddings: f32 full precision; archived centroids: i8 quantized (4x smaller)
- Single centroid rejected; too diffuse for diverse segments

## Relevancy Scoring (Revised — No Time Decay)

- On put: relevancy = `base_score` (e.g., 0.3)
- On put+persist: relevancy = 1.0 (immediate max)
- On search hit: relevancy += `hit_boost` (e.g., 0.1)
- On conversation end: score frozen, no further changes
- NO time-based decay; data stored 6 hours ago with no hits retains base score
- Reason: time decay punishes perfectly valid data; data becomes less true only when contradicted

## Promotion

- Two paths to project-scope promotion:
  1. Organic: relevancy score climbs from search hits; crosses `promotion_threshold` -> auto-promoted
  2. Explicit: `put(data, metadata, persist=true)` -> immediate promotion (relevancy=1.0)
- Mirrors human memory: repetition-based + deliberate memorization

## Temporal Precedence

- Freshness is a ranking tiebreaker, not a destruction mechanism
- Search ranking = `access_score * weight + freshness_rank * weight` (configurable)

## Cross-Conversation Edges

- Project-scope searches that hit nodes across conversations detect semantic proximity
- Create weak cross-conversation edges (bridges, not merges)

## Write Contention — Non-Problem

- Within conversation: single-threaded by definition
- Promotion: infrequent, batchable as single atomic batch at conversation end
- Append-only project graph: no modify conflicts
- Ghost GC: lazy, off critical path

## What Segment Architecture Replaced

- Removed: per-node ghost state, per-node lifetime, ghost death cascades, edge-count ghost lifetime

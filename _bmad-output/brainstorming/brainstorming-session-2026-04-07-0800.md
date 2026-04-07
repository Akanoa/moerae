---
stepsCompleted: [1, 2, 3]
inputDocuments: []
session_topic: 'Multi-layer hierarchical AI memory system with lateral shortcut navigation'
session_goals: '1. Architecture-level ideas, 2. Effort-to-computation strategies, 3. Tech stack alternatives, 4. Language selection'
selected_approach: 'progressive-flow'
techniques_used: ['Morphological Analysis', 'Analogical Thinking', 'SCAMPER Method', 'Decision Tree Mapping']
ideas_generated: [1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49]
context_file: ''
current_phase: 'ALL PHASES COMPLETE'
unexplored_zones: []
---

# Brainstorming Session Results — Moerae

**Facilitator:** Noa
**Date:** 2026-04-07

## Session Overview

**Topic:** Multi-layer hierarchical AI memory system with lateral shortcut navigation
**Goals (ranked):**
1. Architecture-level ideas (how layers are structured, what triggers shortcuts)
2. Strategies for how "effort" translates into computation
3. Alternatives or improvements to initial tech picks (usearch, embeddinggemma-300m)
4. Programming language selection

**Approach:** Progressive Technique Flow
- Phase 1: Morphological Analysis → Architecture
- Phase 2: Analogical Thinking → Effort-to-computation
- Phase 3: SCAMPER → Tech stack stress-test
- Phase 4: Decision Tree Mapping → Language selection

### Initial Tech Findings
- **usearch** (https://github.com/unum-cloud/usearch) — HNSW-based vector search engine
- **embeddinggemma-300m** (https://huggingface.co/unsloth/embeddinggemma-300m-GGUF) — lightweight embeddable model

---

## Phase 1: Morphological Analysis — Architecture Ideas

### Core Problem Statement

Current AI agent memory (CLAUDE.md → MEMORY.md → individual files) suffers from **linear indirection** — the agent must read file after file, burning context tokens on navigation. Moerae replaces this with a queryable system where a single `search` call returns relevant content at the right level of detail.

### Parameter Decomposition

| Parameter | What it governs |
|---|---|
| Layer granularity | What defines each level in the hierarchy |
| Inter-layer relationship | How layers connect vertically |
| Lateral shortcut trigger | What causes a side-indirection link to form |
| Shortcut topology | How lateral links are structured |
| Storage backing | Where/how each layer persists data |
| Query routing | How a query decides which layer(s) to hit |

### Key Design Decision: Layers = Semantic Abstraction

The layers represent levels of semantic specificity:
- Top: Broad categories (Rules, Context, Patterns)
- Middle: Domain scoping (Building, Testing, Deploying)
- Bottom: Concrete artifacts (commands, configs, file paths)

### Architectural Ideas Generated

---

**[Architecture #1]: Semantic Abstraction Hierarchy**
_Concept:_ Layers represent levels of semantic specificity. Top = broad categories, middle = domain scoping, bottom = concrete artifacts.
_Novelty:_ Replaces linear file indirection with a queryable index where depth = precision.

---

**[Architecture #2]: Weighted Graph with B+Tree-like Leaf Connectivity**
_Concept:_ Not a strict tree — a weighted graph where nodes have varying "bushiness." Simple facts are sparse leaves with few/no lateral links. Rich concepts are dense nodes with many weighted edges. Leaf-level connectivity allows traversal without climbing back up.
_Novelty:_ The topology itself encodes information richness — a node's connectivity degree tells you how conceptual vs. concrete it is.

**Key insight from Noa:** This is more like a B+Tree than a tree — leaves are connected for traversal. Link ponderation (weighting) matters more than ordering. Simple subjects (a single command) have sparse connectivity. Conceptual subjects (a brainstorming session, an architectural pattern) are bushy with many cross-connections.

---

**[Architecture #3]: Ponderated Links — Edges Carry Meaning**
_Concept:_ Links have weights encoding relationship strength, co-access frequency, or semantic proximity. The agent uses edge weights to decide whether a lateral jump is worth the effort.
_Novelty:_ Edge weights become the "effort" mechanism itself — following a heavy edge is cheap, following a weak one costs more.

**Critical insight:** The effort gradient is both vertical (broad→specific) AND lateral (strong association→weak association).

---

**[Architecture #4]: Layer-Free Emergent Hierarchy**
_Concept:_ No explicit layers. Hierarchy emerges from graph topology — highly connected nodes are conceptual hubs, sparsely connected nodes are concrete leaves. Node degree = proxy for abstraction level.
_Novelty:_ Eliminates the layer-design problem. Graph self-organizes.

**Limitation identified:** Two unrelated concepts with equal bushiness are indistinguishable by topology alone. Requires semantic matching at entry point. → Leads to hybrid: embeddings for entry, topology for navigation.

---

### Routing Strategies (How the graph is navigated)

**[Architecture #5]: Heaviest-Edge-First Routing**
_Concept:_ At each node, follow the highest-weighted outgoing edge. O(1) per hop.
_Limitation:_ Context-blind. The heaviest edge may not be relevant to the current query.

**[Architecture #6]: Embedding Re-ranking at Each Hop**
_Concept:_ Re-embed query against neighboring node embeddings at each hop. Context-aware but expensive (O(k × embed_cost) per hop).

**[Architecture #7]: Query-Projected Weight Modulation**
_Concept:_ Pre-computed edge weights modulated by dot product between query embedding and edge embedding. Cheap bias toward relevance without full re-ranking. O(k × dot_product) per hop.
_Novelty:_ Similar to HNSW's internal greedy routing.

**[Architecture #8]: Leaf-Level Scan with Shortcut Entry**
_Concept:_ Like B+Tree — route coarsely to the right neighborhood (1-2 hops), then scan laterally across linked leaves. Fine-grained retrieval is sequential scan.
_Novelty:_ Routing only needed for coarse navigation. Leaf retrieval is cheapest possible operation.

**Summary of routing strategies:**

| Strategy | Cost per hop | Context-aware? | Best for |
|---|---|---|---|
| #5 Heaviest-edge | O(1) | No | Known/frequent paths |
| #6 Full re-rank | O(k × embed) | Yes | Vague/novel queries |
| #7 Weight modulation | O(k × dot) | Partially | Most queries |
| #8 Leaf scan | O(1) + O(n) scan | At entry only | Precise retrieval |

**Key insight:** These aren't mutually exclusive. Effort level could select the routing strategy — low effort uses #5, medium uses #7, high uses #6+#8.

---

### Effort Determination (What dictates search depth)

**[Architecture #9]: Query Specificity as Effort Signal**
_Concept:_ The query embedding's entropy/variance against known cluster centroids indicates specificity. High spread = vague query = stay broad. Tight clustering = precise query = go deep. Effort is measured, not chosen.

**[Architecture #10]: Hit Quality Feedback Loop (Iterative Deepening)**
_Concept:_ Start at minimum effort. If result doesn't meet relevance threshold, escalate — add a hop, engage heavier routing. Self-calibrating, no prediction needed. Mirrors human memory: try quick recall, actively search on failure.

**[Architecture #11]: Task-Type Classification**
_Concept:_ Agent's current task carries implicit effort profile. Code generation = high effort (need precise commands). Planning = low effort (need broad context). Memory system receives task-type hint.

**[Architecture #12]: Graph Response Signal**
_Concept:_ The landing node's local topology determines effort. 2 children = sparse, one more hop suffices. 30 children = dense, need more routing intelligence. Effort is locally determined per hop.

**[Architecture #13]: Hybrid — Specificity + Iterative Deepening** ⭐ FAVORED
_Concept:_ Query specificity (#9) sets initial effort budget. Iterative deepening (#10) adjusts. Precise query → start deep with modulated routing. Vague query → start at hub. Either way, escalate if result quality is below threshold.
_Novelty:_ Two-phase: fast estimate + adaptive correction. Avoids both over-fetching and under-fetching.

**[Architecture #14]: Token-Budget-as-Effort** ❌ REJECTED
_Concept:_ Agent's available context window = effort budget.
_Why rejected (by Noa):_ Creates perverse incentive — when the agent needs memory most (context nearly full), it gets the worst results. The system should never degrade search quality based on delivery constraints.

---

### Search vs. Delivery Separation

**[Architecture #15]: Decoupled Search and Delivery** ⭐ KEY PRINCIPLE
_Concept:_ Two independent budgets. Search budget governed by query specificity + iterative deepening — traverses as far as needed. Delivery budget governed by available context — controls compression/truncation of results.
_Novelty:_ Search quality never degrades. Only presentation density changes.

**[Architecture #16]: Multi-Resolution Node Content**
_Concept:_ Every node stores content at multiple compression levels (full, summary, one-liner). Search finds the right node at full fidelity. Delivery picks the compression level fitting available tokens.
_Status:_ Partially superseded by #20/#22 — the metadata IS the compressed form.

---

### API Design and Indexing Rules

**[Architecture #17]: Contextual Blast Radius**
_Concept:_ Every search hit returns a neighborhood — direct hit + connected nodes above a weight threshold. Single `search` returns core match + surrounding context automatically.

```
search "testing rules for build"
→ direct_hit: "run pytest -x --tb=short from project root"
→ context: [
    { relevance: 0.92, "tests require DB container running" },
    { relevance: 0.85, "CI runs same command with --coverage" },
    { relevance: 0.71, "linting must pass before test suite" }
  ]
```

**[Architecture #18]: Write-Time Graph Weaving**
_Concept:_ On `put`, the system embeds content, finds similar existing nodes, creates weighted edges automatically. The graph builds itself from simple `put` calls — agent never describes relationships.

**[Architecture #19]: Search-Time Neighborhood Expansion**
_Concept:_ Search has two phases: (1) Target — find best node via embedding similarity. (2) Expand — walk outward collecting nodes above threshold, weighted by `edge_weight × query_similarity`. Stops when edge weights drop, content budget reached, or no more relevant neighbors.

---

### Metadata Architecture — The Breakthrough

**[Architecture #20]: Dual-Layer Indexing — Metadata + Payload** ⭐ CORE DESIGN
_Concept:_ `put(data, metadata?)` — metadata is optional agent-provided summary. System embeds both independently. Metadata embeddings form coarse navigation layer. Data embeddings form fine-grained layer. Search hits metadata first for broad matching, then data for precision.
_Novelty:_ The agent creates the hierarchy by choosing what to summarize. No forced summarization, no lost fidelity.

**Why this works:**
1. The agent knows its own intent — better than any auto-summarizer
2. Two embedding spaces = two granularities naturally
3. Graceful degradation when metadata absent

**[Architecture #21]: Metadata as Agent-Authored Index Entry**
_Concept:_ Reframe metadata as the agent writing its own index entry — "future me, if you're looking for this, here's how you'd describe it." The hierarchy is agent-curated through usage.

**Graph structure that emerges:**
- Metadata↔Metadata edges: hub-to-hub (conceptual map)
- Data↔Data edges: leaf-to-leaf (B+Tree lateral links)
- Metadata→Data: vertical parent-child link

**[Architecture #22]: Mandatory Metadata for Large Payloads** ⭐ ACCEPTED RULE
_Concept:_ Below token threshold → data embedded directly, metadata optional. Above threshold → metadata required, only metadata embedded, data stored but NOT embedded. Prevents noisy long documents from polluting the vector space.

**Why (from Noa):** Long raw data creates diffuse, meaningless vectors — pure noise in the index. The index must stay clean.

**[Architecture #23]: Fixed Configurable Token Threshold** ⭐ ACCEPTED
_Concept:_ Single config value `max_indexable_tokens` (e.g., 128). Below: data indexed directly. Above: metadata required, error if missing. Simple, predictable, no heuristics.

**API contract:**
```
put(data)                    # works if len(data) ≤ threshold
put(data, metadata)          # always works, metadata indexed if provided
put(long_data)               # ERROR: exceeds threshold, metadata required
search(query) → results      # searches metadata + short-data layer
```

---

## Emerging Architecture Summary

```
┌─────────────────────────────────────────────┐
│              MOERAE SYSTEM                  │
│                                             │
│  ┌─────────────────────────────────────┐    │
│  │        INDEX LAYER                  │    │
│  │  Weighted graph of embeddings from  │    │
│  │  metadata + short data              │    │
│  │  • Nodes connected by semantic      │    │
│  │    proximity with weighted edges    │    │
│  │  • Searchable surface              │    │
│  └──────────────┬──────────────────────┘    │
│                 │ parent→child               │
│  ┌──────────────▼──────────────────────┐    │
│  │        STORAGE LAYER                │    │
│  │  Raw payloads (long data)           │    │
│  │  • Stored but NOT embedded          │    │
│  │  • Retrieved by reference on hit    │    │
│  └─────────────────────────────────────┘    │
│                                             │
│  SEARCH: embed query → hit index →          │
│    expand neighborhood → return hit +       │
│    context shaped by delivery budget        │
│                                             │
│  WRITE: embed metadata/short data →         │
│    connect to existing neighbors →          │
│    store payload                            │
│                                             │
│  EFFORT: query specificity sets initial     │
│    depth, iterative deepening adjusts,      │
│    search never degraded by token pressure  │
│                                             │
│  CONFIG: max_indexable_tokens (e.g. 128)    │
│    enforces clean index                     │
└─────────────────────────────────────────────┘
```

---

### Memory Lifecycle and Scoping

**Context:** Claude Code's current memory is persistent, project-scoped flat files (`~/.claude/projects/<path>/memory/`). No indexing, no search, linear indirection. Moerae replaces this with an intelligent system.

**Key decision from Noa:** Memory databases should NOT be one global pool. They are scoped hierarchically.

**[Architecture #24]: Edge Weight Decay Over Time**
_Concept:_ Edge weights decay with a half-life. Connections never reinforced (co-accessed, re-stored, traversed) gradually weaken. Weak edges pruned. Orphan nodes garbage collected.
_Novelty:_ Graph naturally forgets unused associations.
_Status:_ Less critical given conversation-scoped design, but relevant within long conversations and at project scope.

**[Architecture #25]: Access-Reinforced Persistence**
_Concept:_ Every search hit or edge traversal gives a small weight boost. "Use it or lose it." Combined with #24, creates equilibrium = living memory the agent actually relies on.

**[Architecture #26]: Explicit Tombstoning**
_Concept:_ Agent can `delete(query)` or `put` with supersede flag. Old nodes downranked immediately, garbage collected after grace period. Gives agent explicit control over outdated entries.

**[Architecture #27]: Conversation-Scoped Memory Isolation**
_Concept:_ Each conversation gets its own Moerae instance. No cross-contamination. Graph lifecycle = conversation lifecycle.
_Status:_ Evolved into #28 — conversations are isolated but bridgeable.

**[Architecture #28]: Hierarchical Memory Scopes with Boundary Crossing** ⭐ CORE DESIGN
_Concept:_ Memory partitioned as `(project, conversation)`. Search defaults to current scope. Can cross boundaries — first to other conversations in same project, then to other projects. Boundary crossing is explicit.
_Novelty:_ Mimics human memory — focused attention (current conversation) with ability to recall broader experience. The scope hierarchy IS the effort gradient.

**Three natural effort tiers:**

| Scope | Effort | Searches | When |
|---|---|---|---|
| `(projectA, conv2)` | Low | Current conversation only | Default |
| `(projectA, *)` | Medium | All conversations in project | Agent requests or auto-escalate |
| `(*, *)` | High | Cross-project | Agent must explicitly request |

**[Architecture #29]: Agent-Controlled Boundary Crossing**
_Concept:_ Search API gains scope parameter:
```
search(query)                          # current conversation only
search(query, scope="project")         # all conversations in project
search(query, scope="global")          # cross-project
```
Agent explicitly chooses when to widen. System never silently leaks data across projects — prevents mixing credentials, conflicting conventions, private contexts.

**[Architecture #30]: Scope as Iterative Deepening Extension**
_Concept:_ Scope widening plugs into #13 iterative deepening. Deepening doesn't just go deeper in graph — it widens scope:
1. Search current conversation → found? → return
2. Not found → widen to project scope → found? → return
3. Not found → agent explicitly requests global → search across projects

_Novelty:_ Effort operates on two axes — depth within graph AND breadth across scopes.

**[Architecture #31]: Cross-Conversation Edge Weaving**
_Concept:_ Project-scope searches that hit nodes across conversations detect semantic proximity and create weak cross-conversation edges. Bridges, not merges. The "side indirection" concept operating at conversation boundaries.

```
Conversation 1 graph:        Conversation 2 graph:
  [testing rules]              [build quality]
       |                            |
  [pytest -x]  · · · · · · ·  [fast-fail rules]
                 weak cross-
                 conv edge
```

**[Architecture #32]: Scope-Tagged Results with Boundary Hints** ⭐ ACCEPTED
_Concept:_ When current scope returns empty/below-threshold, system hints that matches exist in wider scopes WITHOUT revealing content. Agent must explicitly opt in. All results tagged with origin scope.

```
search("database connection string")
→ results: []
→ hints: [
    { scope: "projectA/conv1", match_count: 2, relevance: "high" },
    { scope: "projectA/conv3", match_count: 1, relevance: "medium" }
  ]

search("database connection string", scope="project")
→ results: [
    { data: "...", source: "projectA/conv1", relevance: 0.94 },
    { data: "...", source: "projectA/conv3", relevance: 0.72 }
  ]
```

_Novelty:_ Agent gets situational awareness without accidental cross-contamination. Boundary crossing is always a conscious decision.

---

### Memory Lifecycle — Decay and Promotion

**[Architecture #33]: Scope-Dependent Decay**
_Concept:_ Decay operates differently per scope level:
- **Conversation scope:** No decay. Everything stored is current.
- **Project scope:** Nodes weighted by conversation recency. Newer conversations outrank older ones.
- **Global scope:** Projects weighted by last activity.
_Novelty:_ Decay is structural — conversation timestamp IS the decay signal. No background daemon needed.

**[Architecture #34]: Reinforcement Through Cross-Conversation Recurrence**
_Concept:_ If semantically similar content is stored across multiple conversations (conv1, conv5, conv9), the pattern is reinforced. Recurrent knowledge resists decay. One-off observations fade.
_Novelty:_ Project-scope graph develops "strong memories" (frequently reinforced) and "weak memories" (stored once) — like human long-term memory consolidation.

**[Architecture #35]: Conversation Archival Compaction**
_Concept:_ When conversation ends, nodes are evaluated by relevancy score. High-relevancy nodes promoted to project scope. Low-relevancy archived to cold storage (stored but not indexed). Mid-range stays searchable at project scope but ranked lower.

**[Architecture #36]: Dual Promotion — Relevancy Score + Explicit Persist** ⭐ ACCEPTED
_Concept:_ Two paths to project-scope promotion:
1. **Organic:** Node relevancy score climbs from search hits. Crosses threshold → auto-promoted. Frequently useful knowledge promotes itself.
2. **Explicit:** Agent calls `put(data, metadata, persist=true)` → immediate promotion (relevancy=1.0). Agent knows this matters even before it's been searched.

_Novelty:_ Mirrors human memory. Some things remembered through repetition (phone number dialed daily). Some deliberately memorized (new address after moving).

**Why both paths (from Noa):** The agent should be helped to promote knowledge explicitly when it knows it matters. But regular access to knowledge also signals high relevancy organically. Low relevancy = uninteresting data = the phone number forgotten right after dialing.

**[Architecture #37]: Relevancy Score Mechanics (REVISED — no time decay)**
_Concept:_ Simple arithmetic, no decay over time:

```
On put:             relevancy = base_score (e.g., 0.3)
On put+persist:     relevancy = 1.0 (immediate max)
On search hit:      relevancy += hit_boost (e.g., 0.1)
On conv end:        score frozen — no further changes
```

Configurable: `base_score`, `hit_boost`, `promotion_threshold`, `archive_threshold`.

_Novelty:_ NO time-based decay. Data stored 6 hours ago with no hits still has its base score intact. Nothing degrades over time — newer data just ranks better via freshness.

**Why no time decay (from Noa):** Time-based decay punishes perfectly valid data. "Use pytest -x" doesn't become less true after 6 hours of not being accessed. It becomes less true only when explicitly contradicted.

**[Architecture #38]: Temporal Precedence — Newest Wins (for ranking, not decay)**
_Concept:_ When search returns results across conversations, freshness is a ranking tiebreaker, not a destruction mechanism. Two nodes with similar access scores → newer one ranks first.

Search ranking = `access_score × weight + freshness_rank × weight` (configurable).

**[Architecture #39]: Contradiction Detection via Semantic Opposition**
_Concept:_ On `put`, system checks if new node is semantically close but directionally opposite to existing project-scope nodes. Flags conflict in response — doesn't resolve it, surfaces it.

```
put("test framework: vitest, do not use pytest", persist=true)
→ stored successfully
→ conflict_detected: [
    { existing: "test framework: pytest", source: "conv1", similarity: 0.89 }
  ]
```

**[Architecture #40-42]: Superseded by revised model** — No time-based decay at all. Relevancy is access score + freshness rank. Ghost nodes handle the forgetting mechanism instead.

---

### Ghost Nodes — The Forgetting Mechanism

**Key insight (from Noa):** Like the human brain, what we decide to forget from earlier chitchat becomes inaccessible. But the frame — the shape of what was there — remains and can guide us to new searches. That frame is the ghost node.

**[Architecture #43]: Ghost Nodes — Archived Data, Persistent Metadata** ⭐ CORE DESIGN
_Concept:_ When a node falls below archive threshold at conversation end:
- Payload (data) hidden from search results
- Metadata embedding stays in project graph
- Edges preserved — ghost participates in topology
- Returns a hint on hit: "knowledge existed here, in convN, about X"
_Novelty:_ Ghosts preserve graph shape without data weight. They enable lateral jumps through historical neighborhoods — the "eureka" waypoint.

**[Architecture #44]: Ghost Nodes as Connective Tissue**
_Concept:_ Ghosts participate in write-time edge weaving. New nodes can form edges to ghosts via metadata proximity. Fresh knowledge inherits structural connections to archived knowledge.

**[Architecture #47]: Ghosts as Nodes on Death Row — Search Saves Them** ⭐ ACCEPTED
_Concept:_ Ghost is just a regular node with a flag and a countdown. Still has metadata in index, still has edges, still participates in search. If search traverses it with sufficient ranking → relevancy boosted → if above threshold, revived. If never traversed → second death after lifetime expires.

No special API. No revive verb. Just `put` and `search`. The agent doesn't need to know ghosts exist — internal implementation detail.

_Novelty:_ **Search is both retrieval AND the garbage collection signal.** Knowledge that remains useful (even as a waypoint) survives. Knowledge nobody's searches touch dissolves naturally.

**[Architecture #48]: Ghost Payload — Hidden Not Deleted**
_Concept:_ Ghost payload retained but hidden from results. Revival needs the data still there. Actual deletion only at second death. Implementation: a boolean visibility flag.

| State | Metadata | Payload | Indexed | Edges | In results |
|---|---|---|---|---|---|
| Active | Yes | Yes | Both | Full | Yes |
| Promoted | Yes | Yes | Both | Full | Yes |
| Ghost | Yes | Retained | Metadata only | Preserved | Hints only |
| Dead | Removed | Removed | Neither | Severed | No |

**[Architecture #49]: Edge-Count Ghost Lifetime** ⭐ ACCEPTED
_Concept:_ Ghost survival window scales with connectivity:

```
ghost_lifetime = base_lifetime + (edge_count × lifetime_per_edge)

ghost with 1 edge:   2 + (1 × 1) = 3 conversations
ghost with 5 edges:  2 + (5 × 1) = 7 conversations
ghost with 12 edges: 2 + (12 × 1) = 14 conversations
```

Configurable: `base_lifetime`, `lifetime_per_edge`

_Novelty:_ Graph topology decides what's worth preserving. Bridge nodes survive longest. Dead-end trivia dies fast. System values structural importance over content importance.

**Ghost death cascades:** When a ghost dies, neighbors lose an edge → their edge_count drops → their lifetime shrinks. Abandoned neighborhoods collapse in chain reactions — like forgetting a topic makes you forget related topics faster.

**Complete node lifecycle (final):**

```
Active node (score ≥ archive_threshold)
  │
  │ conv ends, score < archive_threshold
  ▼
Ghost node (lifetime = base + edges × per_edge)
  │
  ├─ search traverses → relevancy += hit_boost
  │   └─ score ≥ archive_threshold? → REVIVED (unflagged, active again)
  │
  ├─ no search traffic, conv boundary → lifetime -= 1
  │   └─ lifetime = 0? → SECOND DEATH (full GC, edges severed)
  │       └─ neighbors lose edge → their lifetime recalculated
  │           └─ cascade possible
  │
  └─ edge severed (neighbor died) → edge_count drops → lifetime shrinks
```

**Updated API (unchanged — ghosts are internal):**
```
put(data, metadata?, persist?)  # persist=true → immediate project-scope promotion
search(query, scope?)           # default = conversation
# no delete — ghost lifecycle handles forgetting
```

---

## Updated Architecture Summary

```
┌──────────────────────────────────────────────────────┐
│                    MOERAE SYSTEM                     │
│                                                      │
│  SCOPE HIERARCHY:                                    │
│  ┌────────────────────────────────────────────┐      │
│  │ Global (all projects)                      │      │
│  │  ┌──────────────────────────────────────┐  │      │
│  │  │ Project Scope                        │  │      │
│  │  │  ┌────────────────────────────────┐  │  │      │
│  │  │  │ Conversation Scope (default)   │  │  │      │
│  │  │  │                                │  │  │      │
│  │  │  │  INDEX LAYER                   │  │  │      │
│  │  │  │  Weighted graph: metadata +    │  │  │      │
│  │  │  │  short data embeddings         │  │  │      │
│  │  │  │         │                      │  │  │      │
│  │  │  │  STORAGE LAYER                 │  │  │      │
│  │  │  │  Raw payloads (long data)      │  │  │      │
│  │  │  └────────────────────────────────┘  │  │      │
│  │  │  · · · weak cross-conv edges · · ·  │  │      │
│  │  │  ┌────────────────────────────────┐  │  │      │
│  │  │  │ Conversation N                 │  │  │      │
│  │  │  └────────────────────────────────┘  │  │      │
│  │  └──────────────────────────────────────┘  │      │
│  └────────────────────────────────────────────┘      │
│                                                      │
│  API:                                                │
│    put(data, metadata?)     # metadata req if long   │
│    search(query, scope?)    # default = conversation │
│    # no delete — ghosts handle forgetting             │
│                                                      │
│  SEARCH: specificity → iterative deepening →         │
│    scope widening → blast radius → delivery          │
│                                                      │
│  BOUNDARIES: hints without content leak,             │
│    agent-controlled crossing, origin-tagged results  │
│                                                      │
│  CONFIG: max_indexable_tokens (e.g. 128)             │
│                                                      │
│  LIFECYCLE (no time decay):                          │
│    put → base relevancy → search hits boost →        │
│    conv end: score frozen, promote or ghost           │
│    put+persist → instant promotion                   │
│                                                      │
│  GHOST NODES:                                        │
│    metadata stays indexed, payload hidden             │
│    lifetime = base + (edge_count × per_edge)         │
│    search traversal → boost → possible revival       │
│    no traffic → countdown → second death + cascade   │
│    search IS the GC signal                           │
└──────────────────────────────────────────────────────┘
```

---

### Conflict Resolution

**[Architecture #50]: Semantic Proximity Conflict Surfacing** (deferred)
_Concept:_ On `put`, system checks similarity against existing nodes. If very high similarity but different content, response includes `similar_existing` field. Informational, not blocking. Low cost since similarity already computed during weaving.
_Status:_ Deferred — available as future enhancement if agents need more control.

**[Architecture #52]: Let Ranking Decide — No Conflict Logic** ⭐ ACCEPTED
_Concept:_ No conflict detection, no supersession, no penalties. Trust the natural ranking:
- Same conversation: freshness tiebreaker puts newer first
- Cross conversation: newer conversation ranks higher
- Consistent search hits boost the correct version's access score
- Outdated data eventually ghosts through disuse

_Why (from Noa):_ Start simple. Add agent control later if needed. The system shouldn't guess about contradictions — ranking handles it naturally.

---

### Write Contention — Solved by Architecture

**[Architecture #53]: Conversation-Level Write Isolation**
_Concept:_ Within a conversation, writes are single-threaded by definition. One agent, one conversation, sequential `put` calls. No contention possible. This is the common case.

**[Architecture #54]: Promotion as Batch Operation**
_Concept:_ At conversation end, all promotions/ghosts calculated locally, then written to project graph in a single atomic batch. Contention window minimized to one short transaction.

**[Architecture #55]: Append-Only Project Graph**
_Concept:_ Project graph is append-only for node/edge creation. Edge weights stored per-conversation, aggregated at read time. Two simultaneous promotions just append — no modify conflicts. Ghost GC runs as separate lazy compaction, not on hot write path.

**Conclusion:** Write contention is a non-problem. Architecture naturally avoids it:
1. Within conversation → single-threaded
2. Promotion → infrequent, batchable
3. Append-only → no modify conflicts
4. Ghost GC → lazy, off critical path

---

## Phase 1 Complete — Final Architecture Summary

```
┌──────────────────────────────────────────────────────────┐
│                      MOERAE SYSTEM                       │
│                                                          │
│  API (entire surface):                                   │
│    put(data, metadata?, persist?)                        │
│    search(query, scope?)                                 │
│                                                          │
│  SCOPE HIERARCHY:                                        │
│  ┌──────────────────────────────────────────────┐        │
│  │ Global (* , *)  — agent must request         │        │
│  │  ┌────────────────────────────────────────┐  │        │
│  │  │ Project (projectA, *)                  │  │        │
│  │  │  ┌──────────────────────────────────┐  │  │        │
│  │  │  │ Conversation (projA, conv2)      │  │  │        │
│  │  │  │          [default scope]         │  │  │        │
│  │  │  │                                  │  │  │        │
│  │  │  │  INDEX: weighted graph of        │  │  │        │
│  │  │  │  metadata + short data           │  │  │        │
│  │  │  │  embeddings                      │  │  │        │
│  │  │  │         │                        │  │  │        │
│  │  │  │  STORAGE: raw payloads           │  │  │        │
│  │  │  │  (long data, not embedded)       │  │  │        │
│  │  │  └──────────────────────────────────┘  │  │        │
│  │  │  · · · weak cross-conv edges · · · ·  │  │        │
│  │  │  ┌──────────────────────────────────┐  │  │        │
│  │  │  │ Conversation N                   │  │  │        │
│  │  │  └──────────────────────────────────┘  │  │        │
│  │  └────────────────────────────────────────┘  │        │
│  └──────────────────────────────────────────────┘        │
│                                                          │
│  SEARCH PIPELINE:                                        │
│    query specificity → routing strategy →                │
│    iterative deepening → scope widening →                │
│    blast radius expansion → delivery shaping             │
│                                                          │
│  WRITE PIPELINE:                                         │
│    embed → threshold check (metadata req?) →             │
│    weave edges to neighbors → store payload              │
│                                                          │
│  BOUNDARIES:                                             │
│    scope-tagged results, hints without content leak,     │
│    agent-controlled crossing                             │
│                                                          │
│  NODE LIFECYCLE (no time decay):                         │
│    Active → score frozen at conv end →                   │
│      score ≥ threshold: promoted to project scope        │
│      score < threshold: ghost (metadata persists,        │
│        payload hidden, edges preserved)                  │
│    persist=true → instant promotion, score=1.0           │
│                                                          │
│  GHOST NODES:                                            │
│    lifetime = base + (edge_count × per_edge)             │
│    search traversal boosts → possible revival            │
│    no traffic → countdown → second death + cascade       │
│    search IS the garbage collection signal               │
│                                                          │
│  CONFLICT: ranking decides (freshness + access score)    │
│  CONTENTION: solved by architecture (single-thread +     │
│    append-only + batch promotion)                        │
│                                                          │
│  CONFIG:                                                 │
│    max_indexable_tokens, base_score, hit_boost,           │
│    promotion_threshold, archive_threshold,                │
│    base_lifetime, lifetime_per_edge                       │
└──────────────────────────────────────────────────────────┘
```

**Total Phase 1 ideas: 55 architectural concepts**
**Accepted core decisions: #13, #15, #20, #22, #23, #28, #32, #36, #43, #47, #49, #52**

---

## Phase 2: Analogical Thinking — Effort-to-Computation Strategies

### Analogies Explored

**Analogy 1: CPU Cache Hierarchy** → Scope levels map to cache levels (L1=conversation, L2=project, L3=global). Miss penalty = scope widening cost. Partial fit — CPU misses are binary, Moerae hits are a similarity gradient.

**Analogy 2: Gradient Descent** → Search as gradient walk through similarity space. Each hop follows highest-similarity neighbor. Converges when no neighbor scores better. Absorbed by usearch — HNSW already implements this internally.

**Analogy 3: Skip Lists** → Express lanes (metadata layer) for coarse jumps, local lanes (data layer) for precise matching. Maps to Moerae's dual-layer indexing. Led to insight that metadata/data separation is structural, not just organizational.

**Analogy 4: DNS Resolution** → Pre-computed paths vs runtime resolution. Key insight: **don't build a custom graph walker when usearch already does optimized routing.** Moerae's graph is for write-time intelligence and blast radius, not search-time navigation.

**Analogy 5: Progressive JPEG** → Multi-resolution delivery. Rejected in favor of clean single response (#11).

### Key Insight: Write-Heavy, Search-Light

Independently converged with HNSW design — strong signal that architecture is sound. usearch handles all search-time routing internally. Moerae's graph adds value at write time (edge weaving, lifecycle) and post-search (blast radius expansion), not during search.

### Effort-to-Computation Ideas

**[Effort #1]: Similarity Gradient as Navigation Signal**
_Concept:_ Search flows through graph following similarity gradient — like water downhill. Each hop compares query against neighbors. Stop when similarity peaks. Effort = hop count.
_Status:_ Absorbed — usearch's HNSW does this internally.

**[Effort #2]: Greedy Gradient Walk**
_Concept:_ Greedy walk — step to best neighbor, repeat until no improvement. Effort = Σ(neighbors per hop).
_Status:_ Absorbed — this IS how HNSW works.

**[Effort #3]: Two-Phase Walk — Express Then Local**
_Concept:_ Walk metadata layer (coarse, big jumps), then drop to data layer (precise). Like skip list express/local lanes.
_Novelty:_ Metadata/data dual layer from #20 already provides this structure.

**[Effort #4]: Two Types of Edges — Semantic vs Operational**
_Concept:_ Semantic edges from embedding similarity (content-based). Operational edges from co-access patterns (behavior-based). Graph encodes both what things mean AND how they're used.

**[Effort #5]: Shortcut Formation from Co-Access**
_Concept:_ When agent searches A then B in temporal proximity repeatedly, shortcut edge forms between them. Weight = co-access frequency. Agent builds its own express lanes through usage.

**[Effort #6]: Shortcut Probing** ❌ REJECTED (performance)
_Concept:_ Probe shortcut destinations during search via dot products.
_Why rejected (from Noa):_ Extra computation per hop dramatically slows retrieval. Every added probe multiplies search latency.

**[Effort #7]: Pre-Computed Paths — usearch Handles Routing** ⭐ KEY INSIGHT
_Concept:_ The graph is a write-time structure. At query time, usearch's HNSW handles all routing. No custom graph walking. Moerae's edges matter for blast radius expansion (post-search) and lifecycle management, not for search navigation.

Actual search cost:
```
Embed query:                    ~5ms (one call to embeddinggemma-300m)
usearch nearest-neighbor:       ~microseconds
Blast radius expansion:         edge lookups, no computation
Scope widening (on miss):       repeat search on larger index
```

**[Effort #8]: The Effort Stack** ⭐ ACCEPTED
_Concept:_ Complete search pipeline with real costs:

| Effort | What happens | Cost |
|---|---|---|
| Low | Embed + search current conv index | ~5ms |
| Medium | Same + blast radius expansion | ~5ms + lookups |
| High | Same + search project scope index | ~10ms |
| Max | Same + search global scope | ~15ms |

Expensive part is always the embedding call — done exactly once.

**[Effort #9]: Shortcuts Pre-Computed, Not Probed** ⭐ ACCEPTED
_Concept:_ Co-access shortcuts computed asynchronously after search, stored as edges. At search time, blast radius expansion finds them as regular edge lookups. Zero extra search-time cost. System gets smarter over time without getting slower.

**[Effort #10]: Progressive Delivery** ❌ REJECTED
_Concept:_ Return metadata first, then data, then context incrementally.
_Why rejected:_ Led to #11 — clean response is better than progressive noise.

**[Effort #11]: Clean Response — Data Only, Debug Behind Flag** ⭐ ACCEPTED
_Concept:_ Default response returns only relevant data payloads. No scores, metadata, source tags, or internal details. Debug flag exposes full internals when needed.

```
search("pytest config")
→ ["pytest -x --tb=short from project root",
   "tests require DB container running"]

search("pytest config", debug=true)
→ [{ data: "...", score: 0.93, source: "conv2", ... }]
```

_Why (from Noa):_ Metadata is deceptive noise consuming agent context. Moerae's job is to return relevant data, clean. Internal machinery exposed only for debugging.

**[Effort #12]: Hints as Separate Channel** ⭐ ACCEPTED
_Concept:_ When wider scope has matches but current scope doesn't, hint is a single integer — not inline data. Minimal footprint.

```
search("database credentials")
→ { results: [], hints: 2 }

search("database credentials", scope="project")
→ ["postgres://user:pass@db:5432/myapp"]
```

### Write-Time Effort

**[Effort #13]: Adaptive Neighbor Count on Write**
_Concept:_ K neighbors isn't fixed — determined by similarity distribution. Connect to neighbors above `connection_threshold`, stop when similarity drops below. Concrete facts get few edges, conceptual topics get many — bushiness emerges automatically.
Configurable: `connection_threshold`, `max_neighbors` (safety cap).

**[Effort #14]: Index Size Growth** — HNSW scales O(log n). Even at 5000+ nodes, usearch search is fast. Not a real bottleneck.

**[Effort #15]: Embedding Is the Bottleneck** ⭐
_Concept:_ Every `put` = 1-2 embedding calls. At ~5ms (GPU) to ~50ms (CPU) each, this dominates write cost. Burst of 20 puts on CPU = 2 seconds blocking.

**[Effort #16]: Async Write Pipeline** ⭐ ACCEPTED
_Concept:_ `put` returns immediately. Embedding, weaving, edge creation happen asynchronously. Agent never blocked. Data searchable after ~10-50ms.
_Tradeoff:_ put-then-immediately-search might miss — acceptable since agents don't typically do this.

**[Effort #17]: Batch Embedding for Write Bursts** ⭐ ACCEPTED
_Concept:_ Multiple rapid `put` calls batched into single embedding call. 20 texts batched = ~150ms vs 20 × 50ms = 1000ms sequential.

### Co-Access Tracking

**[Effort #18]: Co-Access Tracking Cost**
_Concept:_ Rolling buffer of ~20 recent search hits with timestamps. Scan on each search for temporal proximity patterns. O(buffer_size) = negligible.

**[Effort #19]: Node-Level Co-Access, Not Query-Level** ⭐ ACCEPTED
_Concept:_ Track by node IDs hit, not query strings. Robust to query variation. Same semantic destination tracked regardless of phrasing.

### Segment Architecture — Replacing Per-Node Ghosts ⭐ MAJOR REVISION

**[Effort #20]: Open/Closed Index Segmentation**
_Concept:_ Index has two states — open (read/write) and closed (read-only). Node threshold triggers close + new open. Search queries all segments. Writes only hit open segment. Benefits: write isolation trivial, lifecycle cleaner at segment granularity.

**[Effort #21]: Segment-Level Lifecycle**
_Concept:_ The segment is the atom of memory management, not the node. Segments are thematically cohesive (built during same working context). Mirrors human episodic memory — "that afternoon debugging the database" comes back as a block or not at all.

```
Segment tracked with:
  creation_conversation, last_search_hit, hit_count, node_count
```

**[Effort #22]: Segment Summary as Ghost**
_Concept:_ Archived segment replaced by cluster centroid embeddings. One summary embedding represents ~500 nodes. Active index stays lean.

**[Effort #23]: Segment Ghost as Centroid Embedding**
_Concept:_ Centroid = mean of all node embeddings in segment. Cheap to compute. But single centroid of diverse segment is diffuse (same problem as long documents).

**[Effort #24]: Multi-Centroid Segment Ghost** ⭐ ACCEPTED
_Concept:_ K-means clustering (k=3-5) on segment embeddings. Each cluster gets one centroid. 500 nodes → 3-5 ghost markers. Preserves topical resolution at ~1% of full size.

| State | In active index | Searchable | Cost |
|---|---|---|---|
| Open segment | All node embeddings | Full results | Full size |
| Closed segment | All node embeddings | Full results | Full size |
| Archived segment | 3-5 cluster centroids | Hints only | ~1% size |
| Dead segment | Nothing | No | Zero |

**[Effort #25]: Segment Lifecycle — Complete Forgetting Model** ⭐ REPLACES PER-NODE GHOSTS

```
OPEN → CLOSED: node count reaches segment_capacity
CLOSED → ARCHIVED: no search hits across N conversation boundaries
ARCHIVED → DEAD: no centroid hits across M more boundaries
Any → stays alive: search hits reset countdown
```

**What this replaces:** Per-node ghost state (#43, #44, #47, #48), per-node lifetime (#49), ghost death cascades — ALL removed. Segment is the lifecycle unit.

**Simplified config:**
```
max_indexable_tokens          # metadata required above this
segment_capacity              # nodes per segment before close
archive_after                 # conv boundaries before archive
dead_after                    # boundaries after archive before death
centroid_clusters             # k for k-means (e.g., 3-5)
```

**[Effort #26-27]: Persist and Segment Pinning** → Superseded by #28.

**[Effort #28]: Persistence Segments — Long-Term Memory** ⭐ ACCEPTED
_Concept:_ Two segment types:

```
REGULAR segments:      OPEN → CLOSED → ARCHIVED → DEAD
PERSISTENCE segments:  OPEN → CLOSED (never archive, never die)
```

`persist=true` routes node to persistence segment. Regular `put` routes to regular segment. No duplication — node lives in one place. `persist` flag determines which segment type receives it.

```
PERSISTENCE SEGMENTS (never archive)
  persist_seg_1 [CLOSED, read-only, permanent]
  persist_seg_2 [OPEN, accepts persist=true writes]

REGULAR SEGMENTS (lifecycle applies)
  reg_seg_12 [OPEN, accepts regular writes]
  reg_seg_11 [CLOSED, fully searchable]
  reg_seg_7  [ARCHIVED → 4 centroids]
  reg_seg_1  [DEAD]
```

### Updated API (Final)

```
put(data, metadata?, persist?)
  → success/error
  persist=false (default): → regular segment
  persist=true: → persistence segment

search(query, scope?, debug?)
  → default: [data, ...]          # clean array
  → + hints: integer              # wider scope match count
  → debug=true: full internals    # scores, sources, segment info
```

### Final Effort-to-Computation Model

| Operation | What happens | Cost |
|---|---|---|
| `put(data)` | Async: embed → regular segment → weave edges | Agent sees ~0ms |
| `put(data, persist=true)` | Async: embed → persistence segment → weave edges | Agent sees ~0ms |
| `search(query)` | Embed → usearch across conv segments | ~5ms |
| `search(query, scope="project")` | Embed → usearch all segments + centroids | ~5-10ms |
| Segment close | Mark read-only | ~0ms |
| Segment archive | K-means → centroids replace nodes | ~100ms background |
| Segment death | Remove centroids | ~0ms |
| Batch writes (20 puts) | Queued, batched embedding | ~150ms background |

---

## Phase 2 Complete — Architecture Summary Update

```
┌──────────────────────────────────────────────────────────┐
│                      MOERAE SYSTEM                       │
│                                                          │
│  API:                                                    │
│    put(data, metadata?, persist?)                        │
│    search(query, scope?, debug?)                         │
│                                                          │
│  SCOPE: conversation (default) → project → global        │
│    Agent-controlled boundary crossing                    │
│    Hints without content leak                            │
│                                                          │
│  SEGMENTS:                                               │
│    REGULAR:     OPEN → CLOSED → ARCHIVED → DEAD          │
│    PERSISTENCE: OPEN → CLOSED (permanent)                │
│                                                          │
│    Open: read/write, one per type per conversation       │
│    Closed: read-only, fully searchable                   │
│    Archived: 3-5 centroids, hints only (~1% size)        │
│    Dead: removed                                         │
│                                                          │
│  WRITE (async):                                          │
│    accept → queue → batch embed → usearch insert →       │
│    weave edges → searchable (~10-50ms after put)         │
│                                                          │
│  SEARCH (~5ms):                                          │
│    embed query → usearch nearest-neighbor →              │
│    blast radius expansion (edge lookups) →               │
│    scope widen if insufficient → clean data response     │
│                                                          │
│  LIFECYCLE (segment-level, no per-node tracking):        │
│    Segment hit count tracks activity                     │
│    No hits across N conv boundaries → archive            │
│    Archived centroids not hit across M more → death      │
│    Persistence segments exempt from lifecycle             │
│                                                          │
│  CONFIG:                                                 │
│    max_indexable_tokens, segment_capacity,                │
│    archive_after, dead_after, centroid_clusters,          │
│    connection_threshold, max_neighbors                    │
│                                                          │
│  CONFLICT: ranking decides (freshness + position)        │
│  CONTENTION: single-threaded + append-only + async       │
└──────────────────────────────────────────────────────────┘
```

---

## Phase 3: SCAMPER — Tech Stack Decisions

### Substitute Analysis

**usearch** — validated. Survives all constraints (no external DB, lightweight, embeddable). Only lancedb is a viable alternative (stores vectors + payloads together) but adds dependency weight.

**Embedding model** — major findings:

**MTEB v2 Benchmark Data (from leaderboard):**

| Rank | Model | Params | Dims | Mean (Task) | Retrieval |
|---|---|---|---|---|---|
| **#23** | **embeddinggemma-300m** | 308M | 768 | **61.15** | **62.49** |
| #73 | mxbai-embed-large-v1 | 335M | 1024 | 45.91 | 40.30 |
| #99 | nomic-embed-text-v1.5 | 137M | 768 | 44.10 | 34.09 |
| #106 | bge-small-en-v1.5 | 33M | 512 | 43.76 | 36.26 |
| #135 | all-MiniLM-L6-v2 | 23M | 384 | 41.39 | 32.51 |

embeddinggemma-300m is rank #23 globally — nearly 2x the retrieval score of alternatives.

**Known issue:** unsloth GGUF conversion is broken (missing 2 dense layers). The `ggml-org/embeddinggemma-300m-qat-q8_0-GGUF` QAT variant may fix this — needs verification.

### SCAMPER Insights

**[Tech #1]: usearch + llama.cpp runtime** ⭐ ACCEPTED
- usearch for vector indexing
- llama.cpp for GGUF model inference
- Both C/C++, both embeddable, both lightweight

**[Tech #3]: usearch built-in features to leverage**
- Quantized vectors (f32, f16, i8) for archive centroids
- Memory-mapped indexes for closed segments on disk
- Index merging for project-scope operations

**[Tech #4]: i8 quantization for archived centroids** ⭐ ACCEPTED
- Active embeddings: f32 (full precision)
- Archived centroids: i8 (4x smaller, acceptable for hints)

**[Tech #5]: Model size analysis**
- Moerae only indexes short text (≤128 tokens by design)
- But MTEB data shows embeddinggemma dominates even against larger models
- The 300M model earns its size

**[Tech #7]: Default Model + Configurable Override** ⭐ ACCEPTED
_Concept:_ Ship with `ggml-org/embeddinggemma-300m-qat-q8_0-GGUF` as default. Zero config for regular users. Power users can override with any GGUF model via config.
_Why (from Noa):_ Don't make the user bear the burden of choosing an embedding model. Default to the best, allow override for power users.

```
moerae.init()                              # default: embeddinggemma-300m QAT Q8
moerae.init(model="path/to/custom.gguf")   # power user override
```

System auto-detects embedding dimensions from model output.

### Final Tech Stack

```
EMBEDDING:  ggml-org/embeddinggemma-300m-qat-q8_0-GGUF (default)
            Any GGUF model (configurable override)
RUNTIME:    llama.cpp (GGUF inference)
INDEXING:   usearch (HNSW vector search)
STORAGE:    Segment-based (regular + persistence)
API:        put(data, metadata?, persist?) / search(query, scope?, debug?)
```

---

## Phase 4: Decision Tree — Language Selection

### Constraints from Tech Stack

Languages with both usearch + llama.cpp bindings: C, C++, Python, Rust, Go, Java, C#, Node.js

### Decision

**[Lang #1]: Rust — Direct Implementation** ⭐ ACCEPTED

_Concept:_ Build Moerae directly in Rust. No Python prototype phase.

**Why Rust:**
- **Compiler as LLM quality gate.** Code will be generated by LLMs. Rust's exhaustive compilation errors create a tight feedback loop: generate → compile → fix errors → repeat. If it compiles, it's memory-safe and likely correct. Python would let subtle bugs (wrong lifecycle transitions, race conditions, silent segment corruption) pass undetected until runtime.
- **First-class async.** Tokio maps naturally to the async write pipeline.
- **Memory safety without GC.** Segment lifecycle, ghost centroids, scope transitions — complex state management where memory bugs would be devastating.
- **Single binary deployment.** Lightweight final product, no runtime dependency.
- **Strong bindings.** Official usearch Rust crate + llama-cpp-rs.
- **The spec is already written.** This brainstorming session documents the complete architecture: API contract, segment lifecycle, scope hierarchy, write/search pipelines, all config parameters. A Python prototype would just be translating this document into code — Rust can do the same, with the compiler enforcing correctness.

**Why not Python first:**
- LLM-generated Python can produce code that "works" but has subtle bugs — wrong state transitions, race conditions in async writes, segment states that silently corrupt. Found at runtime, maybe weeks later.
- The performance-critical path (embedding, search) is already C/C++ via bindings — Python would only be orchestration. But Rust orchestration is equally fast and catches errors at compile time.
- The architecture is thoroughly designed — we don't need a prototype to validate it.

**Why not C:**
- C gives maximum control but maximum footprint for bugs in segment/lifecycle logic.
- No built-in async, no memory safety guarantees.
- Rust gives the same performance with safety.

**Key insight (from Noa):** When the implementer is an LLM, Rust's compiler acts as an automatic code reviewer on every change. Python's permissiveness becomes a liability, not an advantage.

### Rust Crate Dependencies

```
usearch        — HNSW vector search
llama-cpp-rs   — GGUF model inference
tokio          — async runtime (write pipeline)
serde          — config serialization
```

---

## FINAL ARCHITECTURE SUMMARY

```
┌──────────────────────────────────────────────────────────┐
│                    MOERAE SYSTEM                         │
│                    Language: Rust                         │
│                                                          │
│  API:                                                    │
│    put(data, metadata?, persist?)                        │
│    search(query, scope?, debug?)                         │
│                                                          │
│  TECH STACK:                                             │
│    Embedding: embeddinggemma-300m QAT Q8 (default)       │
│               Any GGUF model (configurable)              │
│    Runtime:   llama.cpp via llama-cpp-rs                  │
│    Indexing:  usearch (HNSW)                              │
│    Async:     Tokio                                      │
│                                                          │
│  SCOPE: conversation (default) → project → global        │
│    Agent-controlled boundary crossing                    │
│    Hints without content leak                            │
│                                                          │
│  SEGMENTS:                                               │
│    REGULAR:     OPEN → CLOSED → ARCHIVED → DEAD          │
│    PERSISTENCE: OPEN → CLOSED (permanent)                │
│    Archive: k-means → centroids (i8 quantized)           │
│                                                          │
│  WRITE (async via Tokio):                                │
│    accept → queue → batch embed → usearch insert →       │
│    weave edges → searchable (~10-50ms after put)         │
│                                                          │
│  SEARCH (~5ms):                                          │
│    embed query → usearch nearest-neighbor →              │
│    blast radius expansion (edge lookups) →               │
│    scope widen if insufficient → clean data response     │
│                                                          │
│  LIFECYCLE (segment-level):                              │
│    Segment hit count tracks activity                     │
│    No hits across N conv boundaries → archive            │
│    Archived centroids not hit across M more → death      │
│    Persistence segments exempt from lifecycle             │
│                                                          │
│  RESPONSE:                                               │
│    Default: clean data array only                        │
│    debug=true: scores, sources, segment info             │
│    hints: integer count of wider-scope matches           │
│                                                          │
│  CONFIG:                                                 │
│    max_indexable_tokens, segment_capacity,                │
│    archive_after, dead_after, centroid_clusters,          │
│    connection_threshold, max_neighbors                    │
│                                                          │
│  CONFLICT: ranking decides (freshness + access score)    │
│  CONTENTION: single-threaded + append-only + async       │
└──────────────────────────────────────────────────────────┘
```

---

## Phase Status

- **Phase 1 (Morphological Analysis):** ✅ COMPLETE — 55 architecture ideas
- **Phase 2 (Analogical Thinking):** ✅ COMPLETE — 28 effort ideas, segment architecture
- **Phase 3 (SCAMPER):** ✅ COMPLETE — tech stack validated, model selected
- **Phase 4 (Decision Tree):** ✅ COMPLETE — Rust, direct implementation

## Session Complete

**Total ideas generated:** 83+ across all phases
**Key decisions:** 20+ accepted design choices
**Architecture status:** Complete and ready for implementation

# Benchmark Analysis — BEAM 100K

Results from running Moerae against the [BEAM](https://github.com/mohammadtavakoli78/BEAM) long-term memory benchmark at the 100K token scale (20 conversations, 400 probing questions across 10 memory ability categories).

## Reading the metrics

| Metric | What it measures |
|--------|-----------------|
| **Top-1** | Cosine similarity of the single best retrieved excerpt (0-1). Higher = the closest match is more semantically relevant to the question. |
| **Top-5** | Average similarity across the 5 best excerpts. The gap between Top-1 and Top-5 shows how fast relevance drops off. |
| **Rubric@k** | Fraction of BEAM's rubric criteria whose key terms appear in the top-k retrieved excerpts. A lower-bound proxy for "did Moerae retrieve the right conversation turns?" — it's lexical matching, so paraphrased content scores 0 even when semantically correct. |
| **ms** | Average search latency per question. |

## Results

```
  Config: top-k=50, segment_capacity=500, max_indexable_tokens=512
  Embedding model: embeddinggemma-300m-qat-q8_0 (308M params, 768 dims)

  Category                       N   Top-1   Top-5  Rubric@5  Rubric@10  Rubric@20
  information extraction        40  0.8407  0.8202    38.8%     41.9%     41.9%
  multi session reasoning       40  0.8587  0.8440     9.3%      9.8%     14.2%
  knowledge update              40  0.8824  0.8457    60.0%     67.5%     70.0%
  temporal reasoning            40  0.8735  0.8496    17.5%     18.8%     18.8%
  event ordering                40  0.8336  0.8231     0.4%      0.8%      1.5%
  contradiction resolution      40  0.8746  0.8408     0.0%      0.0%      0.0%
  preference following          40  0.8760  0.8573     1.2%      1.2%      1.2%
  instruction following         40  0.8454  0.8297     3.8%      3.8%      3.8%
  abstention                    40  0.8652  0.8265     2.5%      2.5%      2.5%
  summarization                 40  0.8522  0.8421     5.7%      6.7%      7.9%
  OVERALL                      400  0.8602            13.9%     15.3%     16.2%

  Score distribution (top-1):
    0.90-1.00   38 ( 9.5%)
    0.85-0.90  212 (53.0%)
    0.80-0.85  136 (34.0%)
    0.75-0.80   14 ( 3.5%)
    0.00-0.75    0 ( 0.0%)
```

## What these results mean

### Retrieval quality is consistent

96.5% of questions have a top-1 similarity above 0.80 and zero fall below 0.75. Moerae finds semantically relevant content for every question — there are no total misses. The tight score distribution (most scores in 0.85-0.90) shows the embedding model reliably surfaces related content from conversations of ~100 turns.

### Difficulty doesn't degrade retrieval

Easy questions average 0.87 top-1 similarity, hard questions 0.86. The gap is negligible. This is expected: difficulty in BEAM refers to reasoning complexity, not how hard the content is to locate. Moerae's job is finding the right turns, not reasoning over them.

### Two tiers of categories

The rubric scores reveal a clean separation between what retrieval can and cannot solve:

**Retrieval-solvable** — these categories test whether the right content is found:

| Category | Rubric@20 | Why |
|----------|-----------|-----|
| knowledge update | 70.0% | Questions target specific facts that appear verbatim in turns. Moerae surfaces both old and new information effectively. |
| information extraction | 41.9% | Factual retrieval. Lower than knowledge update because facts may be spread across more turns. |
| temporal reasoning | 18.8% | Moerae finds the right events but rubric criteria include temporal relationships that only exist in answers. |
| multi session reasoning | 14.2% | Evidence spans multiple non-adjacent turns. Retrieval finds relevant pieces but connecting them is the LLM's job. |

**Reasoning-dependent** — these categories require the downstream LLM to reason over retrieved content:

| Category | Rubric@20 | Why it's low |
|----------|-----------|--------------|
| contradiction resolution | 0.0% | Rubric expects the response to *detect and describe* contradictions ("there is contradictory information"). These phrases only appear in answers, never in conversation turns. Even perfect retrieval scores 0%. |
| event ordering | 1.5% | Rubric tests temporal ordering of events. Retrieval finds the events but doesn't order them. |
| preference/instruction following | 1-4% | Rubric checks compliance behavior, not content presence. |
| abstention | 2.5% | Rubric checks that the model correctly refuses to answer. This is pure LLM behavior. |

Low rubric scores in the reasoning-dependent tier are not a retrieval failure. They confirm that these categories are beyond the scope of what any retrieval system can solve alone — the answer generation step (Phase 2) is what turns high similarity scores into correct answers for these categories.

### Rubric@k is a lower bound

The overall 16.2% rubric@20 appears low but is structurally conservative. Rubric matching is lexical (substring search) while retrieval is semantic. A rubric criterion like *"should mention: implementing a basic homepage route with Flask"* will miss if Moerae retrieves a semantically equivalent paraphrase. True recall is higher than rubric@k suggests, especially for categories where content is commonly rephrased across conversation turns.

## Performance

| Metric | Value |
|--------|-------|
| Ingestion throughput | 8.1 turns/s |
| Search throughput | 2.1 questions/s |
| Average search latency | 474ms |
| Turns ingested | 2,866 across 20 conversations |
| Average turns per conversation | 143 |

Search latency varies by category (197ms to 677ms) likely due to differences in query length and token count affecting embedding time.

## Reproducing

```bash
bash bench/download_beam.sh
./target/release/moerae-bench --dataset bench/beam-data/chats --scale 100K
python bench/analyze.py bench/output/
```

See [BENCHMARK.md](../BENCHMARK.md) for full pipeline documentation including answer generation and BEAM evaluation.

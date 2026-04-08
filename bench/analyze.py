"""
Analyze Moerae retrieval quality from moerae-bench output.

Measures retrieval performance without requiring LLM-as-judge evaluation.
Works directly on the intermediate JSON from Phase 1.

Usage:
    python bench/analyze.py bench/output/              # analyze all files in directory
    python bench/analyze.py bench/output/100K_1.json   # analyze single file
"""

import json
import sys
from pathlib import Path
from collections import defaultdict


def load_files(path: Path) -> list[dict]:
    """Load one or more intermediate JSON files."""
    if path.is_file():
        with open(path) as f:
            return [json.load(f)]
    elif path.is_dir():
        results = []
        for p in sorted(path.glob("*.json")):
            with open(p) as f:
                results.append(json.load(f))
        return results
    else:
        print(f"error: {path} not found", file=sys.stderr)
        sys.exit(1)


def rubric_hit_rate(item: dict, context_items: list[dict], top_k: int) -> float | None:
    """Check what fraction of rubric items are found in top-k retrieved context."""
    rubric = item.get("rubric")
    if not rubric or not isinstance(rubric, list):
        return None

    top_data = " ".join(ctx["data"].lower() for ctx in context_items[:top_k])

    hits = 0
    for criterion in rubric:
        if not isinstance(criterion, str):
            continue
        # Extract key terms from rubric (after "should state:", "should mention:", etc.)
        text = criterion.lower()
        for prefix in ("should state:", "should mention:", "should include:"):
            if prefix in text:
                text = text.split(prefix, 1)[1]
                break

        # Check if key terms appear in retrieved context
        terms = [t.strip().strip("'\"") for t in text.split(",") if len(t.strip()) > 2]
        if not terms:
            terms = [text.strip()]

        if any(term in top_data for term in terms if term):
            hits += 1

    return hits / len(rubric) if rubric else None


def analyze(files: list[dict]):
    """Run analysis across all loaded files."""
    # Collect per-type metrics
    by_type = defaultdict(lambda: {
        "scores": [],
        "top1_scores": [],
        "top5_scores": [],
        "search_ms": [],
        "rubric_hits_5": [],
        "rubric_hits_10": [],
        "rubric_hits_20": [],
        "by_difficulty": defaultdict(list),
        "count": 0,
    })

    all_scores = []
    total_questions = 0

    for data in files:
        for qtype, items in data["questions"].items():
            bucket = by_type[qtype]

            for item in items:
                ctx = item["retrieved_context"]
                if not ctx:
                    continue

                scores = [c["score"] for c in ctx]
                top1 = scores[0] if scores else 0
                top5 = scores[:5]
                bucket["scores"].extend(scores)
                bucket["top1_scores"].append(top1)
                bucket["top5_scores"].append(sum(top5) / len(top5) if top5 else 0)
                bucket["search_ms"].append(item.get("search_ms", 0))
                bucket["count"] += 1
                total_questions += 1
                all_scores.append(top1)

                difficulty = item.get("difficulty", "unknown")
                bucket["by_difficulty"][difficulty].append(top1)

                # Rubric coverage at different k values
                for k, key in [(5, "rubric_hits_5"), (10, "rubric_hits_10"), (20, "rubric_hits_20")]:
                    hit = rubric_hit_rate(item, ctx, k)
                    if hit is not None:
                        bucket[key].append(hit)

    if not all_scores:
        print("No data to analyze.")
        return

    # --- Print report ---
    scale = files[0]["scale"]
    config = files[0]["config"]

    print(f"{'=' * 72}")
    print(f"  Moerae Retrieval Analysis — {scale} scale, {len(files)} conversations")
    print(f"  Config: top-k={config['search_limit']}, "
          f"segment_capacity={config['segment_capacity']}, "
          f"max_indexable_tokens={config['max_indexable_tokens']}")
    print(f"{'=' * 72}")
    print()

    # Ingest stats
    total_turns = sum(d["stats"]["turns_ingested"] for d in files)
    total_ingest = sum(d["stats"]["ingest_secs"] for d in files)
    total_search = sum(d["stats"]["search_secs"] for d in files)
    print(f"  Ingested {total_turns} turns in {total_ingest:.1f}s "
          f"({total_turns / total_ingest:.1f} turns/s)")
    print(f"  Searched {total_questions} questions in {total_search:.1f}s "
          f"({total_questions / total_search:.1f} q/s)")
    print()

    # Per-type table
    print(f"  {'Category':<28} {'N':>3}  {'Top-1':>6}  {'Top-5':>6}  "
          f"{'Rubric@5':>8}  {'Rubric@10':>9}  {'Rubric@20':>9}  {'ms':>5}")
    print(f"  {'—' * 28} {'—' * 3}  {'—' * 6}  {'—' * 6}  "
          f"{'—' * 8}  {'—' * 9}  {'—' * 9}  {'—' * 5}")

    type_order = [
        "information_extraction", "multi_session_reasoning", "knowledge_update",
        "temporal_reasoning", "event_ordering", "contradiction_resolution",
        "preference_following", "instruction_following", "abstention",
        "summarization",
    ]
    sorted_types = [t for t in type_order if t in by_type]
    sorted_types += [t for t in by_type if t not in sorted_types]

    for qtype in sorted_types:
        b = by_type[qtype]
        n = b["count"]
        top1 = sum(b["top1_scores"]) / len(b["top1_scores"]) if b["top1_scores"] else 0
        top5 = sum(b["top5_scores"]) / len(b["top5_scores"]) if b["top5_scores"] else 0
        ms = sum(b["search_ms"]) / len(b["search_ms"]) if b["search_ms"] else 0

        rub5 = sum(b["rubric_hits_5"]) / len(b["rubric_hits_5"]) if b["rubric_hits_5"] else float("nan")
        rub10 = sum(b["rubric_hits_10"]) / len(b["rubric_hits_10"]) if b["rubric_hits_10"] else float("nan")
        rub20 = sum(b["rubric_hits_20"]) / len(b["rubric_hits_20"]) if b["rubric_hits_20"] else float("nan")

        r5 = f"{rub5:7.1%}" if rub5 == rub5 else "     n/a"
        r10 = f"{rub10:8.1%}" if rub10 == rub10 else "      n/a"
        r20 = f"{rub20:8.1%}" if rub20 == rub20 else "      n/a"

        label = qtype.replace("_", " ")
        print(f"  {label:<28} {n:>3}  {top1:>6.4f}  {top5:>6.4f}  {r5}  {r10}  {r20}  {ms:>5.0f}")

    # Overall
    overall_top1 = sum(all_scores) / len(all_scores)
    all_rubric5 = [h for b in by_type.values() for h in b["rubric_hits_5"]]
    all_rubric10 = [h for b in by_type.values() for h in b["rubric_hits_10"]]
    all_rubric20 = [h for b in by_type.values() for h in b["rubric_hits_20"]]
    avg_ms = sum(b_ms for b in by_type.values() for b_ms in b["search_ms"]) / total_questions

    or5 = f"{sum(all_rubric5) / len(all_rubric5):7.1%}" if all_rubric5 else "     n/a"
    or10 = f"{sum(all_rubric10) / len(all_rubric10):8.1%}" if all_rubric10 else "      n/a"
    or20 = f"{sum(all_rubric20) / len(all_rubric20):8.1%}" if all_rubric20 else "      n/a"

    print(f"  {'—' * 28} {'—' * 3}  {'—' * 6}  {'—' * 6}  "
          f"{'—' * 8}  {'—' * 9}  {'—' * 9}  {'—' * 5}")
    print(f"  {'OVERALL':<28} {total_questions:>3}  {overall_top1:>6.4f}  "
          f"{'':>6}  {or5}  {or10}  {or20}  {avg_ms:>5.0f}")

    # Difficulty breakdown
    print()
    print(f"  By difficulty:")
    diff_scores = defaultdict(list)
    for b in by_type.values():
        for diff, scores in b["by_difficulty"].items():
            diff_scores[diff].extend(scores)
    for diff in sorted(diff_scores.keys()):
        scores = diff_scores[diff]
        avg = sum(scores) / len(scores)
        print(f"    {diff:<12} {len(scores):>3} questions, avg top-1: {avg:.4f}")

    # Score distribution
    print()
    print(f"  Score distribution (top-1):")
    brackets = [(0.9, 1.0), (0.85, 0.9), (0.8, 0.85), (0.75, 0.8), (0.0, 0.75)]
    for lo, hi in brackets:
        count = sum(1 for s in all_scores if lo <= s < hi or (hi == 1.0 and s == 1.0))
        bar = "#" * int(count / len(all_scores) * 40)
        print(f"    {lo:.2f}-{hi:.2f}  {count:>3} ({count / len(all_scores):5.1%})  {bar}")

    print()


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python bench/analyze.py <path-to-output-dir-or-file>")
        sys.exit(1)

    path = Path(sys.argv[1])
    files = load_files(path)
    analyze(files)

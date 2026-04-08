"""
Phase 2: Generate LLM answers from Moerae retrieval results.

Reads intermediate JSON from moerae-bench (Phase 1), calls `claude --print`
to answer each probing question using retrieved context, and outputs
BEAM-compatible results JSON.

Usage:
    python bench/generate_answers.py \
        --input bench/output/100K_0.json \
        --output bench/results/100K/0/
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path


SYSTEM_PROMPT = (
    "You are answering questions about a conversation based on retrieved memory excerpts. "
    "Answer accurately and concisely using only the provided context. "
    "If the context does not contain enough information, say so."
)


def build_prompt(question: str, context_items: list[dict], max_context: int) -> str:
    """Build the prompt with retrieved context and question."""
    excerpts = context_items[:max_context]

    context_block = "\n\n".join(
        f"[Excerpt {item['rank']}, relevance: {item['score']:.3f}]\n{item['data']}"
        for item in excerpts
    )

    return (
        f"Retrieved conversation excerpts (most relevant first):\n"
        f"---\n{context_block}\n---\n\n"
        f"Question: {question}"
    )


def generate_answer(prompt: str, model: str) -> str:
    """Call claude --print to generate an answer."""
    result = subprocess.run(
        ["claude", "--print", "--model", model, "--system-prompt", SYSTEM_PROMPT, prompt],
        capture_output=True,
        text=True,
        timeout=120,
    )
    if result.returncode != 0:
        return f"[claude error: {result.stderr.strip()}]"
    return result.stdout.strip()


def process_file(input_path: Path, output_path: Path, max_context: int, model: str):
    """Process one intermediate JSON file and produce BEAM-compatible output."""
    with open(input_path) as f:
        data = json.load(f)

    scale = data["scale"]
    chat_index = data["chat_index"]
    plan = data.get("plan")
    tag = f"[{scale}/{chat_index}]" if plan is None else f"[{scale}/{chat_index}:plan-{plan}]"

    questions = data["questions"]

    beam_output = {}
    total = sum(len(items) for items in questions.values())
    processed = 0

    for qtype, items in questions.items():
        beam_items = []

        for item in items:
            prompt = build_prompt(item["question"], item["retrieved_context"], max_context)
            llm_response = generate_answer(prompt, model)

            # Preserve all original BEAM fields, add llm_response
            beam_item = {
                k: v for k, v in item.items()
                if k not in ("retrieved_context", "search_ms")
            }
            beam_item["llm_response"] = llm_response

            beam_items.append(beam_item)
            processed += 1

            print(f"{tag} [{processed}/{total}] {qtype}", flush=True)

        beam_output[qtype] = beam_items

    # Write output
    output_path.mkdir(parents=True, exist_ok=True)
    result_file = output_path / "moerae_answers.json"

    with open(result_file, "w", encoding="utf-8") as f:
        json.dump(beam_output, f, indent=4, ensure_ascii=False)

    print(f"{tag} Done -> {result_file} ({processed} answers)")


def main():
    parser = argparse.ArgumentParser(description="Generate LLM answers from Moerae retrieval results")
    parser.add_argument("--input", required=True, help="Intermediate JSON from moerae-bench")
    parser.add_argument("--output", required=True, help="Output directory for BEAM-compatible results")
    parser.add_argument("--model", default="sonnet", help="Claude model (default: sonnet)")
    parser.add_argument("--max-context", type=int, default=20, help="Max retrieved excerpts to include")

    args = parser.parse_args()

    process_file(
        input_path=Path(args.input),
        output_path=Path(args.output),
        max_context=args.max_context,
        model=args.model,
    )


if __name__ == "__main__":
    main()

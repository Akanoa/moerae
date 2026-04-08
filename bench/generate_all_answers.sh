#!/bin/bash
set -euo pipefail

SCALE="${1:?Usage: $0 <scale> [input-dir] [output-dir] [model]}"
INPUT_DIR="${2:-bench/output}"
OUTPUT_DIR="${3:-bench/results}"
MODEL="${4:-sonnet}"

files=("${INPUT_DIR}/${SCALE}"_*.json)
if [ ! -f "${files[0]}" ]; then
    echo "error: no files matching ${INPUT_DIR}/${SCALE}_*.json"
    exit 1
fi

echo "Generating answers for ${#files[@]} ${SCALE} conversations (model: ${MODEL})"

for f in "${files[@]}"; do
    idx=$(echo "$f" | grep -oP '\d+(?=\.json)')
    python bench/generate_answers.py \
        --input "$f" \
        --output "${OUTPUT_DIR}/${SCALE}/${idx}/" \
        --model "$MODEL"
done

echo "All done. Results in ${OUTPUT_DIR}/${SCALE}/"

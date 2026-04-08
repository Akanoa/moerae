#!/bin/bash
set -euo pipefail

TARGET="${1:-bench/beam-data}"

echo "Downloading BEAM dataset to ${TARGET}..."

if [ -d "${TARGET}/chats" ]; then
    echo "Dataset already exists at ${TARGET}/chats — skipping."
    echo "Delete ${TARGET} and re-run to re-download."
    exit 0
fi

mkdir -p "${TARGET}"

# Clone BEAM repo (sparse checkout for chats/ only)
TEMP_DIR=$(mktemp -d)
trap 'rm -rf "${TEMP_DIR}"' EXIT

git clone --depth 1 --filter=blob:none --sparse \
    https://github.com/mohammadtavakoli78/BEAM.git \
    "${TEMP_DIR}/BEAM"

cd "${TEMP_DIR}/BEAM"
git sparse-checkout set chats
cd -

# Move chats directory to target
mv "${TEMP_DIR}/BEAM/chats" "${TARGET}/chats"

echo "Done. Dataset available at ${TARGET}/chats/"
echo ""
echo "Scales available:"
ls -1 "${TARGET}/chats/" | while read -r scale; do
    count=$(find "${TARGET}/chats/${scale}" -maxdepth 1 -mindepth 1 -type d | wc -l)
    echo "  ${scale}: ${count} conversations"
done

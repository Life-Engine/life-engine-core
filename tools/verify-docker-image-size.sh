#!/usr/bin/env bash
# Builds the Life Engine Core Docker image and verifies its size is under 50 MB.
#
# Usage (from any directory):
#   bash tools/verify-docker-image-size.sh
#
# Exit codes:
#   0 = image is under the size limit
#   1 = image exceeds the limit, build failure, or Docker not available

set -euo pipefail

# Resolve the repository root so the script works from any working directory.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

IMAGE_TAG="life-engine-core:size-check"
MAX_SIZE_MB=50
MAX_SIZE_BYTES=$((MAX_SIZE_MB * 1024 * 1024))

# Ensure Docker is available.
if ! command -v docker &> /dev/null; then
  echo "FAIL: docker is not installed or not in PATH"
  exit 1
fi

echo "Building Docker image from $(pwd)/apps/core/Dockerfile ..."
docker build \
  -f apps/core/Dockerfile \
  -t "$IMAGE_TAG" \
  . \
  --quiet

# Get image size in bytes.
SIZE_BYTES=$(docker image inspect "$IMAGE_TAG" --format='{{.Size}}')

# Convert to MB using awk (avoids dependency on bc).
SIZE_MB=$(awk "BEGIN { printf \"%.2f\", $SIZE_BYTES / 1048576 }")

echo "Image size: ${SIZE_MB} MB (limit: ${MAX_SIZE_MB} MB)"

# Clean up the test image regardless of pass/fail.
docker rmi "$IMAGE_TAG" > /dev/null 2>&1 || true

# Compare using integer arithmetic (no bc needed).
if [ "$SIZE_BYTES" -gt "$MAX_SIZE_BYTES" ]; then
  echo "FAIL: Image exceeds ${MAX_SIZE_MB} MB limit"
  exit 1
fi

echo "PASS: Image is under ${MAX_SIZE_MB} MB"

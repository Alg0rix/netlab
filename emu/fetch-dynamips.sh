#!/usr/bin/env bash
# Fetch upstream Dynamips at the pinned revision and apply the netlab WASM
# port patch. Idempotent: an already-patched checkout is left alone.
set -euo pipefail
cd "$(dirname "$0")"
REV=41e1b4484e2a581ca29d8b67dd536f8303d21a0c
if [ ! -d dynamips/.git ]; then
  git clone -q https://github.com/GNS3/dynamips.git dynamips
fi
if git -C dynamips diff --quiet; then
  git -C dynamips checkout -q "$REV"
  git -C dynamips apply ../dynamips.patch
  echo "dynamips @ ${REV:0:7} patched"
else
  echo "dynamips already has local changes — not touching it"
fi

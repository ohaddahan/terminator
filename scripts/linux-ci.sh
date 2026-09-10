#!/usr/bin/env bash
# Compatibility entrypoint; implementation lives in the Rust xtask crate.
set -euo pipefail
exec cargo xtask linux-check "$@"

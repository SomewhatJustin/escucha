#!/usr/bin/env bash
set -uo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

OUT_DIR="${OUT_DIR:-$ROOT_DIR/target/diagnostics}"
mkdir -p "$OUT_DIR"

echo "==> Running structured diagnostics"
diag_exit=0
cargo run --quiet -- --diagnose | tee "$OUT_DIR/diagnose.json"
diag_exit=${PIPESTATUS[0]}

echo
echo "==> Running headless smoke test"
smoke_exit=0
cargo run --quiet -- --smoke-test | tee "$OUT_DIR/smoke-test.json"
smoke_exit=${PIPESTATUS[0]}

echo
echo "Reports written to:"
echo "  $OUT_DIR/diagnose.json"
echo "  $OUT_DIR/smoke-test.json"

echo
echo "Exit summary: diagnose=$diag_exit smoke_test=$smoke_exit"
if [ "$diag_exit" -ne 0 ] || [ "$smoke_exit" -ne 0 ]; then
  exit 1
fi

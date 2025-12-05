#!/bin/bash
set -e

echo "Running code tests..."
# Run userland tests with host target and std features, serialized to avoid global runtime race conditions
cargo test -p userland --target x86_64-unknown-linux-gnu --features std -- --test-threads=1

echo "Running host smoke test..."
./scripts/host_smoke.sh

echo "Running QEMU smoke test..."
./scripts/qemu_smoke.sh

echo "All tests passed!"

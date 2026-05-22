#!/bin/bash

# LLM Integration Test Script
# Usage: ./test_llm.sh <API_KEY>

set -e

if [ -z "$1" ]; then
    echo "Usage: ./test_llm.sh <OPENAI_API_KEY>"
    echo "Example: ./test_llm.sh sk-..."
    exit 1
fi

export OPENAI_API_KEY="$1"

echo "=== Testing LLM Integration ==="
echo ""

# Build first
echo "Building..."
cargo build --release --manifest-path crates/nexacode-core/Cargo.toml

echo ""
echo "Running tests..."
echo ""

# Run tests
cargo test --manifest-path crates/nexacode-core/Cargo.toml -- --nocapture

echo ""
echo "=== All tests passed! ==="

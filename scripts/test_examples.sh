#!/bin/sh

# Script to test all examples in the repository
# Usage: ./scripts/test_examples.sh

set -e

echo "Building all examples..."
cargo build --examples

examples=$(cargo metadata --no-deps --format-version=1 | 
            grep -o '"name":"[^"]*","kind":\["example"\]' | 
            sed 's/"name":"//;s/","kind":\["example"\]//')

# Run each example
echo "Running examples:"
for example in $examples; do
  echo "====================================="
  echo "Running example: $example"
  echo "====================================="
  cargo run --example $example
  
  # Check if the example ran successfully
  if [ $? -eq 0 ]; then
    echo "✅ Example $example ran successfully"
  else
    echo "❌ Example $example failed"
    exit 1
  fi
done

echo "All examples passed!" 

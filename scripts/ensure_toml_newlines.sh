#!/bin/sh

# Script to ensure all TOML files in the repository end with a newline
# Usage: ./scripts/ensure_toml_newlines.sh

set -e

# Find all TOML files in the repository
toml_files=$(find . -name "*.toml" -type f | grep -v "target/")

echo "Checking TOML files for final newlines..."

count=0
for file in $toml_files; do
  # Check if file ends with a newline
  if [ -f "$file" ] && [ "$(tail -c 1 "$file" | wc -l)" -eq 0 ]; then
    echo "Adding newline to $file"
    echo "" >> "$file"
    count=$((count + 1))
  fi
done

if [ $count -eq 0 ]; then
  echo "All TOML files already have final newlines."
else
  echo "Added newlines to $count TOML files."
fi

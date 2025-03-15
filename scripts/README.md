# Oxigraph Scripts

This directory contains utility scripts for the Oxigraph project.

## Available Scripts

### `ensure_toml_newlines.sh`

This script ensures that all TOML files in the repository have a final newline.

Usage:
```bash
./scripts/ensure_toml_newlines.sh
```

Why this script exists: 
- Consistent newlines at the end of files are a common best practice
- This helps avoid issues with Git showing files as modified when only the final newline is different
- Some tools expect files to end with a newline, following the POSIX standard

The script is also integrated into the CI pipeline to ensure all TOML files have a proper final newline.

## CI Checks

The Oxigraph project includes several CI checks to ensure code quality and correctness:

### TOML Newlines Check

The CI pipeline checks that all TOML files have proper final newlines, using the `ensure_toml_newlines.sh` script.

### Examples Check

The CI pipeline automatically builds and runs all examples to ensure they compile and execute without errors. This helps catch any regressions that might break the examples.

To add a new example that will be automatically tested by CI:

1. Create a new Rust file in the `examples` directory
2. Add an entry to the `examples/Cargo.toml` file
3. The CI will automatically detect and run the new example 
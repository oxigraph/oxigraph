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
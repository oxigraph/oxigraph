#!/usr/bin/env python3
"""Build script for Oxigraph .NET bindings.

Usage:
    python build_package.py [--release] [--features rocksdb]

Steps:
    1. cargo build (cdylib -> oxigraph_dotnet.dll/.so/.dylib)
    2. dotnet build (C# library + tests)
    3. dotnet test (run xUnit tests)
    4. dotnet pack (optional, creates NuGet package)
"""

import argparse
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOTNET_DIR = ROOT / "dotnet"
RUST_DIR = DOTNET_DIR / "src" / "oxigraph-dotnet"
TEST_DIR = DOTNET_DIR / "tests" / "Oxigraph.Tests"
SRC_DIR = DOTNET_DIR / "src" / "Oxigraph"


def run(cmd, **kwargs):
    print(f"\n> {' '.join(cmd)}")
    result = subprocess.run(cmd, cwd=ROOT, **kwargs)
    if result.returncode != 0:
        sys.exit(result.returncode)


def main():
    parser = argparse.ArgumentParser(description="Build Oxigraph .NET bindings")
    parser.add_argument("--release", action="store_true", help="Build in release mode")
    parser.add_argument("--features", default="", help="Extra cargo features (e.g., rocksdb)")
    args = parser.parse_args()

    profile = "release" if args.release else "debug"
    build_flag = ["--release"] if args.release else []
    features = ["--features", args.features] if args.features else []

    # Step 1: Build Rust cdylib
    print("=== Building Rust cdylib ===")
    run(["cargo", "build", *build_flag, "-p", "oxigraph-dotnet", *features])

    # Step 2: Build C# projects
    print("=== Building C# projects ===")
    config = "Release" if args.release else "Debug"
    run(["dotnet", "build", str(DOTNET_DIR), "-c", config])

    # Step 3: Copy native library to test output
    print("=== Copying native library ===")
    target_dir = ROOT / "target" / profile
    test_out = TEST_DIR / "bin" / config / "net10.0"

    if sys.platform == "win32":
        dll = target_dir / "oxigraph_dotnet.dll"
    elif sys.platform == "darwin":
        dll = target_dir / "liboxigraph_dotnet.dylib"
    else:
        dll = target_dir / "liboxigraph_dotnet.so"

    if dll.exists():
        shutil.copy2(dll, test_out)
        print(f"  Copied {dll.name} -> {test_out}")
    else:
        print(f"  WARNING: {dll} not found, skipping copy")

    # Step 4: Run tests
    print("=== Running tests ===")
    run(["dotnet", "test", str(TEST_DIR), "-c", config, "--verbosity", "normal"])

    print("\n=== Build complete ===")
    print(f"  C# library: {SRC_DIR / 'bin' / config / 'net10.0' / 'Oxigraph.dll'}")
    print(f"  Tests: {TEST_DIR / 'bin' / config / 'net10.0' / 'Oxigraph.Tests.dll'}")


if __name__ == "__main__":
    main()
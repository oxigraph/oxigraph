import argparse
import json
import os
import platform
import re
import sys
import tarfile
from io import BytesIO
from pathlib import Path
from shutil import copy, which
from subprocess import check_call, check_output
from urllib.request import urlopen

parser = argparse.ArgumentParser()
parser.add_argument("--debug", action="store_true")
parser.add_argument("--offline", action="store_true")
args = parser.parse_args()

crate_dir = Path(__file__).resolve().parent
cargo_flags = ["--locked", *(["--offline"] if args.offline else [])]
pkg_dir = crate_dir / "pkg"
cargo_metadata = json.loads(
    check_output(
        ["cargo", "metadata", "--format-version", "1", *cargo_flags], cwd=crate_dir
    )
)
cargo_metadata_incroxigraph_js = next(
    package
    for package in cargo_metadata["packages"]
    if package["name"] == "incroxigraph-js"
)
cargo_metadata_wasm_bindgen = next(
    package
    for package in cargo_metadata["packages"]
    if package["name"] == "wasm-bindgen"
)

target_dir = Path(cargo_metadata["target_directory"])

# Cargo
cargo_args = [
    "cargo",
    "build",
    "--package",
    "incroxigraph-js",
    "--target",
    "wasm32-unknown-unknown",
    *cargo_flags,
]
if not args.debug:
    cargo_args.append("--release")
check_call(cargo_args, cwd=crate_dir)

# Download wasm-bindgen
wasm_bindgen_version = cargo_metadata_wasm_bindgen["version"]
target_map = {
    ("linux", "aarch64"): "aarch64-unknown-linux-gnu",
    ("linux", "arm64"): "aarch64-unknown-linux-gnu",
    ("linux", "x86_64"): "x86_64-unknown-linux-musl",
    ("darwin", "x86_64"): "x86_64-apple-darwin",
    ("darwin", "aarch64"): "aarch64-apple-darwin",
    ("darwin", "arm64"): "aarch64-apple-darwin",
    ("win32", "amd64"): "x86_64-pc-windows-msvc",
}
override = os.environ.get("WASM_BINDGEN") or which("wasm-bindgen")
if override:
    wasm_bindgen_path = Path(override)
else:
    uname_key = (sys.platform, platform.machine().lower())
    if uname_key not in target_map:
        raise SystemExit(
            f"Platform {uname_key} is not supported for builds; set WASM_BINDGEN"
        )
    wasm_bindgen_dir_name = (
        f"wasm-bindgen-{wasm_bindgen_version}-{target_map[uname_key]}"
    )
    wasm_bindgen_path = (
        target_dir
        / wasm_bindgen_dir_name
        / ("wasm-bindgen.exe" if sys.platform == "win32" else "wasm-bindgen")
    )
if override and not wasm_bindgen_path.is_file():
    raise SystemExit(f"WASM_BINDGEN does not point to a file: {wasm_bindgen_path}")
if wasm_bindgen_path.exists():
    print(f"Using cached wasm-bindgen {wasm_bindgen_path}")
else:
    if args.offline:
        raise SystemExit(
            f"Offline build requires wasm-bindgen {wasm_bindgen_version}; set WASM_BINDGEN"
        )
    with (
        urlopen(
            f"https://github.com/wasm-bindgen/wasm-bindgen/releases/download/{wasm_bindgen_version}/{wasm_bindgen_dir_name}.tar.gz"
        ) as response,
        tarfile.open(fileobj=BytesIO(response.read())) as tar,
    ):
        for member in tar.getmembers():
            if (
                not (target_dir / member.name)
                .resolve()
                .is_relative_to(target_dir.resolve())
            ):
                raise ValueError("Invalid wasm-bindgen archive path")
        tar.extractall(target_dir)
    print(f"Downloaded wasm-bindgen to {wasm_bindgen_path}")

if (
    check_output([wasm_bindgen_path, "--version"], text=True).strip()
    != f"wasm-bindgen {wasm_bindgen_version}"
):
    raise SystemExit("wasm-bindgen must match the version in Cargo.lock")

# Wasm-bindgen
wasm_bindgen_args = [
    wasm_bindgen_path,
    target_dir
    / "wasm32-unknown-unknown"
    / ("debug" if args.debug else "release")
    / "incroxigraph_js.wasm",
    "--out-dir",
    pkg_dir,
]
if args.debug:
    wasm_bindgen_args.append("--debug")
check_call(wasm_bindgen_args)

# Keep opaque parser helpers private. Some wasm-bindgen versions list these in
# the entrypoint even though the background module does not export them.
entrypoint = pkg_dir / "incroxigraph_js.js"
private_helpers = {"AsyncParserIterator", "ParserIterator", "ParserIteratorResult"}
entrypoint.write_text(
    re.sub(
        r"""(export\s*\{)([^}]*)(\}\s*from\s*["']\./incroxigraph_js_bg\.js["'];)""",
        lambda match: (
            match[1]
            + ", ".join(
                name.strip()
                for name in match[2].split(",")
                if name.strip() and name.strip() not in private_helpers
            )
            + match[3]
        ),
        entrypoint.read_text(),
    )
)

# package.json
content = {
    "name": "incroxigraph-js",
    "type": "module",
    "collaborators": cargo_metadata_incroxigraph_js["authors"],
    "description": cargo_metadata_incroxigraph_js["description"],
    "version": cargo_metadata_incroxigraph_js["version"],
    "license": cargo_metadata_incroxigraph_js["license"],
    "repository": {
        "type": "git",
        "url": "git+https://github.com/maartyman/incroxigraph.git",
        "directory": "js",
    },
    "bugs": {"url": "https://github.com/maartyman/incroxigraph/issues"},
    "files": [
        "incroxigraph_js*.wasm",
        "incroxigraph_js*.js",
        "incroxigraph_js*.d.ts",
        "snippets",
        "LICENSE-MIT",
        "LICENSE-APACHE",
    ],
    "main": "incroxigraph_js.js",
    "types": "incroxigraph_js.d.ts",
    "exports": {
        ".": {"types": "./incroxigraph_js.d.ts", "default": "./incroxigraph_js.js"}
    },
    "dependencies": json.loads((crate_dir / "package.json").read_text())[
        "dependencies"
    ],
    "engines": {"node": ">=18"},
    "keywords": cargo_metadata_incroxigraph_js["keywords"],
}

(pkg_dir / "package.json").write_text(json.dumps(content, indent=4))
copy(crate_dir / cargo_metadata_incroxigraph_js["readme"], pkg_dir)

for license_file in ("LICENSE-MIT", "LICENSE-APACHE"):
    copy(crate_dir.parent / license_file, pkg_dir)

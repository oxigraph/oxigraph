import argparse
import json
import os
import tarfile
from io import BytesIO
from pathlib import Path
from subprocess import check_output, check_call
from shutil import copy
from urllib.request import urlopen

parser = argparse.ArgumentParser()
parser.add_argument("--debug", action="store_true")
args = parser.parse_args()

crate_dir = Path(__file__).parent
pkg_dir = crate_dir / "pkg"
target_dir = crate_dir.parent / "target"
cargo_metadata = json.loads(
    check_output(["cargo", "metadata", "--format-version", "1"])
)
cargo_metadata_oxigraph_js = next(
    package
    for package in cargo_metadata["packages"]
    if package["name"] == "oxigraph-js"
)
cargo_metadata_wasm_bindgen = next(
    package
    for package in cargo_metadata["packages"]
    if package["name"] == "wasm-bindgen"
)

# Cargo
cargo_args = ["cargo", "build", "--target", "wasm32-unknown-unknown"]
if not args.debug:
    cargo_args.append("--release")
check_call(cargo_args)

# Download wasm-bindgen
wasm_bindgen_version = cargo_metadata_wasm_bindgen["version"]
uname = os.uname()
target_map = {
    ("linux", "aarch64"): "aarch64-unknown-linux-gnu",
    ("linux", "arm64"): "aarch64-unknown-linux-gnu",
    ("linux", "x86_64"): "x86_64-unknown-linux-musl",
    ("darwin", "x86_64"): "x86_64-apple-darwin",
    ("darwin", "aarch64"): "aarch64-apple-darwin",
    ("darwin", "arm64"): "aarch64-apple-darwin",
    ("win32", "x86_64"): "x86_64-pc-windows-msvc",
}
uname_key = (uname.sysname.lower(), uname.machine)
if uname_key not in target_map:
    print(f"Platform {uname} is not supported for builds")
    exit(1)
wasm_bindgen_dir_name = f"wasm-bindgen-{wasm_bindgen_version}-{target_map[uname_key]}"
wasm_bindgen_path = target_dir / wasm_bindgen_dir_name
if not wasm_bindgen_path.exists():
    with urlopen(
        f"https://github.com/wasm-bindgen/wasm-bindgen/releases/download/{wasm_bindgen_version}/{wasm_bindgen_dir_name}.tar.gz"
    ) as response:
        with tarfile.open(fileobj=BytesIO(response.read())) as tar:
            tar.extractall(target_dir)

# Wasm-bindgen
wasm_bindgen_args = [
    wasm_bindgen_path / "wasm-bindgen",
    target_dir
    / "wasm32-unknown-unknown"
    / ("debug" if args.debug else "release")
    / "oxigraph.wasm",
    "--out-dir",
    pkg_dir,
]
if args.debug:
    wasm_bindgen_args.append("--debug")
check_call(wasm_bindgen_args)

# package.json
content = {
    "name": "oxigraph-js",
    "type": "module",
    "collaborators": cargo_metadata_oxigraph_js["authors"],
    "description": cargo_metadata_oxigraph_js["description"],
    "version": cargo_metadata_oxigraph_js["version"],
    "license": cargo_metadata_oxigraph_js["license"],
    "repository": {
        "type": "git",
        "url": "https://github.com/oxigraph/oxigraph/tree/main/js",
    },
    "files": ["oxigraph_bg.wasm", "oxigraph.js", "oxigraph_bg.js", "oxigraph.d.ts"],
    "main": "oxigraph.js",
    "types": "oxigraph.d.ts",
    "keywords": cargo_metadata_oxigraph_js["keywords"],
}

(pkg_dir / "package.json").write_text(json.dumps(content, indent=4))
copy(crate_dir / cargo_metadata_oxigraph_js["readme"], pkg_dir)

import json
from urllib.request import urlopen

MSRV = "1.70.0"
DEFAULT_BUILD_FLAGS = {
    "-Wtrivial-casts",
    "-Wtrivial-numeric-casts",
    "-Wunsafe-code",
    "-Wunused-lifetimes",
    "-Wunused-qualifications",
}
FLAGS_BLACKLIST = {
    "-Wclippy::alloc-instead-of-core",
    "-Wclippy::arithmetic-side-effects",  # TODO: might be nice
    "-Wclippy::as-conversions",
    "-Wclippy::cargo-common-metadata",  # TODO: might be nice
    "-Wclippy::doc-markdown",  # Too many false positives
    "-Wclippy::default-numeric-fallback",
    "-Wclippy::else-if-without-else",
    "-Wclippy::exhaustive-enums",
    "-Wclippy::exhaustive-structs",
    "-Wclippy::float-arithmetic",
    "-Wclippy::float-cmp",
    "-Wclippy::float-cmp-const",
    "-Wclippy::impl-trait-in-params",
    "-Wclippy::implicit-return",
    "-Wclippy::indexing-slicing",
    "-Wclippy::integer-arithmetic",
    "-Wclippy::integer-division",
    "-Wclippy::map-err-ignore",
    "-Wclippy::missing-docs-in-private-items",
    "-Wclippy::missing-errors-doc",
    "-Wclippy::missing-inline-in-public-items",
    "-Wclippy::missing-panics-doc",
    "-Wclippy::missing-trait-methods",
    "-Wclippy::mixed-read-write-in-expression",
    "-Wclippy::mod-module-files",
    "-Wclippy::module-name-repetitions",
    "-Wclippy::modulo-arithmetic",
    "-Wclippy::multiple-crate-versions",
    "-Wclippy::multiple-unsafe-ops-per-block",
    "-Wclippy::must-use-candidate",  # TODO: might be nice
    "-Wclippy::option-option",
    "-Wclippy::pattern-type-mismatch",
    "-Wclippy::pub-use",
    "-Wclippy::question-mark-used",
    "-Wclippy::self-named-module-files",  # TODO: might be nice
    "-Wclippy::semicolon-if-nothing-returned",  # TODO: might be nice
    "-Wclippy::semicolon-outside-block",
    "-Wclippy::similar-names",
    "-Wclippy::single-char-lifetime-names",
    "-Wclippy::std-instead-of-alloc",
    "-Wclippy::std-instead-of-core",
    "-Wclippy::shadow-reuse",
    "-Wclippy::shadow-unrelated",
    "-Wclippy::string-slice",  # TODO: might be nice
    "-Wclippy::too-many-lines",
    "-Wclippy::separated-literal-suffix",
    "-Wclippy::unreachable",  # TODO: might be nice
    "-Wclippy::unwrap-used",  # TODO: might be nice to use expect instead
    "-Wclippy::wildcard-enum-match-arm",  # TODO: might be nice
    "-Wclippy::wildcard-imports",  # TODO: might be nice
}

build_flags = set(DEFAULT_BUILD_FLAGS)
with urlopen(
    f"https://rust-lang.github.io/rust-clippy/rust-{MSRV}/lints.json"
) as response:
    for lint in json.load(response):
        if lint["level"] == "allow" and lint["group"] != "nursery":
            build_flags.add(f"-Wclippy::{lint['id'].replace('_', '-')}")

for flag in FLAGS_BLACKLIST:
    if flag in build_flags:
        build_flags.remove(flag)
    else:
        print(f"Unused blacklisted flag: {flag}")

with open("./config.toml", "wt") as fp:
    fp.write("[build]\n")
    fp.write("rustflags = [\n")
    for flag in sorted(build_flags):
        fp.write(f'    "{flag}",\n')
    fp.write("]\n")

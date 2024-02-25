import json
from pathlib import Path
from urllib.request import urlopen

import tomlkit

MSRV = "1.76.0"
LINT_BLACKLIST = {
    "absolute_paths",  # TODO: might be nice
    "alloc_instead_of_core",
    "arithmetic_side_effects",  # TODO: might be nice
    "as_conversions",
    "big_endian_bytes",
    "cargo_common_metadata",  # TODO: might be nice
    "doc_markdown",  # Too many false positives
    "default_numeric_fallback",
    "else_if_without_else",
    "exhaustive_enums",
    "exhaustive_structs",
    "float_arithmetic",
    "float_cmp",
    "float_cmp_const",
    "impl_trait_in_params",
    "implicit_return",
    "indexing_slicing",
    "integer_division",
    "iter_over_hash_type",
    "little_endian_bytes",
    "map_err_ignore",
    "min_ident_chars",
    "missing_docs_in_private_items",
    "missing_errors_doc",
    "missing_inline_in_public_items",
    "missing_panics_doc",
    "missing_trait_methods",
    "mixed_read_write_in_expression",
    "mod_module_files",
    "module_name_repetitions",
    "modulo_arithmetic",
    "multiple_crate_versions",
    "multiple_unsafe_ops_per_block",
    "must_use_candidate",  # TODO: might be nice
    "option_option",
    "pattern_type_mismatch",
    "pub_use",
    "pub_with_shorthand",
    "question_mark_used",
    "self_named_module_files",  # TODO: might be nice
    "semicolon_if_nothing_returned",  # TODO: might be nice
    "semicolon_outside_block",
    "similar_names",
    "single_call_fn",
    "single_char_lifetime_names",
    "std_instead_of_alloc",
    "std_instead_of_core",
    "shadow_reuse",
    "shadow_unrelated",
    "string_slice",  # TODO: might be nice
    "too_many_lines",
    "separated_literal_suffix",
    "unreachable",  # TODO: might be nice
    "unwrap_used",  # TODO: might be nice to use expect instead
    "wildcard_enum_match_arm",  # TODO: might be nice
    "wildcard_imports",  # TODO: might be nice
}

lints = set()
with urlopen(
    f"https://rust-lang.github.io/rust-clippy/rust-{MSRV}/lints.json"
) as response:
    for lint in json.load(response):
        if lint["level"] == "allow" and lint["group"] != "nursery":
            lints.add(lint["id"])

for flag in LINT_BLACKLIST:
    if flag in lints:
        lints.remove(flag)
    else:
        print(f"Unused blacklisted flag: {flag}")

cargo_path = Path(__file__).parent.parent / "Cargo.toml"
cargo_toml = tomlkit.parse(cargo_path.read_text())
cargo_toml["workspace"]["lints"]["clippy"] = {lint: "warn" for lint in sorted(lints)}
cargo_path.write_text(tomlkit.dumps(cargo_toml))

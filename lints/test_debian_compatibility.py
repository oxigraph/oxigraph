import json
import subprocess
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import urlopen

TARGET_DEBIAN_VERSIONS = ["sid"]
IGNORE_PACKAGES = {"oxigraph-js", "oxigraph-testsuite", "pyoxigraph", "sparql-smith"}
ALLOWED_MISSING_PACKAGES = {
    "codspeed-criterion-compat",
    "json-event-parser",
    "oxhttp",
    "oxiri",
    "quick-xml",
}

base_path = Path(__file__).parent.parent

cargo_metadata = json.loads(
    subprocess.check_output(["cargo", "metadata", "--format-version", "1"])
)
package_by_id = {package["id"]: package for package in cargo_metadata["packages"]}
debian_cache = {}
errors = set()


def parse_version(version):
    return tuple(int(e) for e in version.split("-")[0].split("."))


for package_id in cargo_metadata["workspace_default_members"]:
    package = package_by_id[package_id]
    if package["name"] in IGNORE_PACKAGES:
        continue
    for dependency in package["dependencies"]:
        if (
            "path" in dependency
            or dependency["name"] in ALLOWED_MISSING_PACKAGES
        ):
            continue
        candidate_debian_name = f"rust-{dependency['name'].replace('_', '-')}"
        if dependency["name"] not in debian_cache:
            url = f"https://sources.debian.org/api/src/{candidate_debian_name}/"
            try:
                with urlopen(url) as response:
                    debian_cache[candidate_debian_name] = json.loads(
                        response.read().decode()
                    )
            except HTTPError as e:
                print(f"Error {e} from {url}, skipping {dependency['name']}")
                continue
        debian_package = debian_cache[candidate_debian_name]
        if "error" in debian_package:
            errors.add(f"No Debian package found for {dependency['name']}")
            continue
        for target_debian_suite in TARGET_DEBIAN_VERSIONS:
            debian_version = next(
                (
                    debian_version
                    for debian_version in debian_package["versions"]
                    if target_debian_suite in debian_version["suites"]
                ),
                None,
            )
            if debian_version is None:
                errors.add(
                    f"The debian package {debian_package['package']} does not support {target_debian_suite}"
                )
                continue

            # We check the debian version is compatible with the req version
            parsed_debian_version = parse_version(debian_version["version"])
            for range_element in dependency["req"].split(","):
                range_element = range_element.strip()
                if range_element.startswith("^"):
                    first_found = False
                    for expected, actual in zip(
                        parse_version(range_element[1:]), parsed_debian_version
                    ):
                        if first_found:
                            if actual > expected:
                                break  # Done
                            if actual < expected:
                                errors.add(
                                    f"The debian package {debian_package['package']} version {debian_version['version']} is not compatible with requirement {range_element}"
                                )
                                break
                        else:
                            if actual != expected:
                                errors.add(
                                    f"The debian package {debian_package['package']} version {debian_version['version']} is not compatible with requirement {range_element}"
                                )
                            if expected != 0:
                                first_found = True
                elif range_element.startswith(">="):
                    if not parsed_debian_version >= parse_version(range_element[2:]):
                        errors.add(
                            f"The debian package {debian_package['package']} version {debian_version['version']} is not compatible with requirement {range_element}"
                        )
                elif range_element.startswith(">"):
                    if not parsed_debian_version > parse_version(range_element[1:]):
                        errors.add(
                            f"The debian package {debian_package['package']} version {debian_version['version']} is not compatible with requirement {range_element}"
                        )
                elif range_element.startswith("<="):
                    if not parsed_debian_version <= parse_version(range_element[2:]):
                        errors.add(
                            f"The debian package {debian_package['package']} version {debian_version['version']} is not compatible with requirement {range_element}"
                        )
                elif range_element.startswith("<"):
                    if not parsed_debian_version < parse_version(range_element[1:]):
                        errors.add(
                            f"The debian package {debian_package['package']} version {debian_version['version']} is not compatible with requirement {range_element}"
                        )
                else:
                    errors.add(
                        f"The requirement {range_element} of {dependency['name']} is not supported by this script"
                    )

for error in sorted(errors):
    print(error)
if errors:
    exit(1)

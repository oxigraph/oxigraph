import json
import subprocess
from pathlib import Path
from time import sleep
from urllib.error import HTTPError
from urllib.request import urlopen

TARGET_DEBIAN_VERSIONS = ["sid"]
IGNORE_PACKAGES = {"oxigraph-js", "oxigraph-testsuite", "pyoxigraph", "sparql-smith"}
ALLOWED_MISSING_PACKAGES = {
    "codspeed-criterion-compat",
    "escargot",
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
workspace_packages = {
    package_id.split(" ")[0]
    for package_id in cargo_metadata["workspace_default_members"]
}
debian_cache = {}
errors = set()


def parse_version(version):
    return tuple(int(e) for e in version.split("-")[0].split("."))


def fetch_debian_package_desc(debian_name):
    url = f"https://sources.debian.org/api/src/{debian_name}/"
    for i in range(0, 10):
        try:
            with urlopen(url) as response:
                return json.loads(response.read().decode())
        except HTTPError as e:
            wait = 2**i
            print(f"Error {e} from {url}, retrying after {wait}s")
            sleep(wait)
    raise Exception(f"Failed to fetch {url}")


for package_id in cargo_metadata["workspace_default_members"]:
    package = package_by_id[package_id]
    if package["name"] in IGNORE_PACKAGES:
        continue
    for dependency in package["dependencies"]:
        if (
            dependency["name"] in workspace_packages
            or dependency["name"] in ALLOWED_MISSING_PACKAGES
        ):
            continue
        candidate_debian_name = f"rust-{dependency['name'].replace('_', '-')}"
        if dependency["name"] not in debian_cache:
            debian_cache[candidate_debian_name] = fetch_debian_package_desc(
                candidate_debian_name
            )
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

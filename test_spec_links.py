import re
import sys
from pathlib import Path
from urllib.error import HTTPError
from urllib.parse import urlsplit, urlunsplit
from urllib.request import urlopen

LINK_REGEXES = {
    r"\[[^]]+]\((https?://(w3c.github.io|www.w3.org)/[^)]+)\)",  # Markdown
    r"<(https?://(w3c.github.io|www.w3.org)/[^>]+)>`_",  # reStructuredText
}

base_path = Path(__file__).parent
spec_cache = {}
errors = set()

for ext in ("md", "rs"):
    for file in Path(__file__).parent.rglob(f"*.{ext}"):
        content = file.read_text()
        for link_regex in LINK_REGEXES:
            for m in re.finditer(link_regex, content):
                url = m.group(1)
                (scheme, host, path, query, fragment) = urlsplit(url)
                if scheme != "https":
                    errors.add(f"HTTP URL used by {url} in {file}")
                if query != "":
                    errors.add(f"URL query used by {url} in {file}")
                if path.endswith(".html/"):
                    errors.add(f".html/ used by {url} in {file}")
                base_url = urlunsplit(("https", host, path.rstrip("/"), "", ""))
                if base_url not in spec_cache:
                    try:
                        with urlopen(base_url) as response:
                            spec_cache[base_url] = response.read().decode()
                    except HTTPError as e:
                        errors.add(
                            f"Fetching {url} used in {file} return HTTP error: {e}"
                        )
                spec_content = spec_cache.get(base_url, "")
                if (
                    fragment != ""
                    and re.search(rf"[iI][dD]\s*=\s*['\"]{fragment}['\"]", spec_content)
                    is None
                ):
                    errors.add(
                        f"Fragment {fragment} of {url} used in {file} does not exist"
                    )

print("Used specs:")
for url in sorted(spec_cache.keys()):
    print(url)

if errors:
    print()
    for error in sorted(errors):
        print(error, file=sys.stderr)
    exit(1)

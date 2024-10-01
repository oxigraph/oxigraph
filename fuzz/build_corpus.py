import hashlib
import random
from pathlib import Path

base = Path(__file__).parent.parent
for target, ext in [
    ("sparql_query", "rq"),
    ("sparql_update", "ru"),
    ("sparql_results_xml", "srx"),
    ("sparql_results_json", "srj"),
    ("sparql_results_tsv", "tsv"),
    ("n3", "n3"),
    ("nquads", "nq"),
    ("trig", "trig"),
    ("rdf_xml", "rdf"),
]:
    target_dir = base / "fuzz" / "corpus" / target
    for f in base.rglob(f"*.{ext}"):
        if "manifest" in str(f):
            continue  # we skip the manifests
        with f.open("rb") as fp:
            data = fp.read()
        pos = random.randint(0, len(data))
        data = data[:pos] + b"\xff" + data[pos:]
        hash = hashlib.sha256()
        hash.update(data)
        target_dir.mkdir(parents=True, exist_ok=True)
        with (target_dir / f"{hash.hexdigest()}").open("wb") as fp:
            fp.write(data)

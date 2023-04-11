"""
Converts a SPARQL query JSON explanation file to a flamegraph.
Usage: python explanation_to_flamegraph.py explanation.json flamegraph.svg
"""
import json
import subprocess
from argparse import ArgumentParser
from pathlib import Path
from shutil import which
from tempfile import NamedTemporaryFile

if which('flamegraph.pl') is None:
    raise Exception(
        'This script requires the flamegraph.pl script from https://github.com/brendangregg/FlameGraph to be installed and be in $PATH.')

parser = ArgumentParser(
    prog='OxigraphFlamegraph',
    description='Builds a flamegraph from the Oxigraph query explanation JSON format',
    epilog='Text at the bottom of help')
parser.add_argument('json_explanation', type=Path)
parser.add_argument('flamegraph_svg', type=Path)
args = parser.parse_args()


def trace_line(label: str, value: float):
    return f"{label} {int(value * 1_000_000)}"


with args.json_explanation.open('rt') as fp:
    explanation = json.load(fp)
trace = []
if "parsing duration in seconds" in explanation:
    trace.append(trace_line("parsing", explanation['parsing duration in seconds']))
if "planning duration in seconds" in explanation:
    trace.append(trace_line("planning", explanation['planning duration in seconds']))
already_used_names = {}


def add_to_trace(node, path):
    path = f"{path};{node['name'].replace(' ', '`')}"
    if path in already_used_names:
        already_used_names[path] += 1
        path = f"{path}`{already_used_names[path]}"
    else:
        already_used_names[path] = 0
    samples = node['duration in seconds'] - sum(child['duration in seconds'] for child in node.get("children", ()))
    trace.append(trace_line(path, samples))
    for i, child in enumerate(node.get("children", ())):
        add_to_trace(child, path)


add_to_trace(explanation["plan"], 'eval')
with NamedTemporaryFile('w+t') as fp:
    fp.write('\n'.join(trace))
    fp.flush()
    args.flamegraph_svg.write_bytes(subprocess.run(['flamegraph.pl', fp.name], stdout=subprocess.PIPE).stdout)

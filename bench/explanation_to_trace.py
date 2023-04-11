"""
Converts a SPARQL query JSON explanation file to a tracing event file compatible with Chrome.
Usage: python explanation_to_trace.py explanation.json trace.json
"""
import json
from argparse import ArgumentParser
from pathlib import Path

parser = ArgumentParser(
    prog='OxigraphTracing',
    description='Builds a Trace Event Format file from the Oxigraph query explanation JSON format')
parser.add_argument('json_explanation', type=Path)
parser.add_argument('json_trace_event', type=Path)
args = parser.parse_args()

with args.json_explanation.open('rt') as fp:
    explanation = json.load(fp)
trace = []


def trace_element(name: str, cat: str, start_s: float, duration_s: float):
    return {
        "name": name,
        "cat": cat,
        "ph": "X",
        "ts": int(start_s * 1_000_000),
        "dur": int(duration_s * 1_000_000),
        "pid": 1
    }


def add_to_trace(node, path, start_time: float):
    path = f"{path};{node['name'].replace(' ', '`')}"
    trace.append(trace_element(node["name"], node["name"].split("(")[0], start_time, node["duration in seconds"]))
    for child in node.get("children", ()):
        add_to_trace(child, path, start_time)
        start_time += child["duration in seconds"]


current_time = 0
if "parsing duration in seconds" in explanation:
    d = explanation["parsing duration in seconds"]
    trace.append(trace_element(f"parsing", "parsing", current_time, d))
    current_time += d
if "planning duration in seconds" in explanation:
    d = explanation["planning duration in seconds"]
    trace.append(trace_element(f"planning", "planning", current_time, d))
    current_time += d
add_to_trace(explanation["plan"], 'eval', current_time)

with args.json_trace_event.open("wt") as fp:
    json.dump(trace, fp)

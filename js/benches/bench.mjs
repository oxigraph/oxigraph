import { existsSync } from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import { withCodSpeed } from "@codspeed/tinybench-plugin";
import * as fzstd from "fzstd";
import { Bench } from "tinybench";
import { Store } from "../pkg/oxigraph.js";

async function readData(file) {
    if (!existsSync(file)) {
        const response = await fetch(
            `https://github.com/Tpt/bsbm-tools/releases/download/v0.2/${file}`,
        );
        await writeFile(file, Buffer.from(await response.arrayBuffer()));
    }
    const compressed = await readFile(file);
    const decompressed = fzstd.decompress(compressed);
    return new TextDecoder().decode(decompressed);
}

const explore_1000_nt = await readData("explore-1000.nt.zst");
const explore_1000_store = new Store();
explore_1000_store.load(explore_1000_nt, "application/n-triples");

const bsbm_1000_operations = (await readData("mix-exploreAndUpdate-1000.tsv.zst"))
    .split("\n")
    .reverse()
    .slice(0, 300)
    .filter((line) => line.trim() !== "")
    .map((line) => line.trim().split("\t"));

const bench = withCodSpeed(new Bench());
bench
    .add("JS: load BSBM explore 1000", () => {
        const store = new Store();
        store.load(explore_1000_nt, "application/n-triples");
    })
    .add("JS: BSBM explore 1000 query", () => {
        for (const [kind, sparql] of bsbm_1000_operations) {
            if (kind === "query") {
                explore_1000_store.query(sparql, { results_format: "xml" });
            }
        }
    })
    .add("JS: BSBM explore 1000 query and update", () => {
        for (const [kind, sparql] of bsbm_1000_operations) {
            if (kind === "query") {
                explore_1000_store.query(sparql, { results_format: "xml" });
            } else if (kind === "update") {
                explore_1000_store.update(sparql);
            }
        }
    });

await bench.run();
console.table(bench.table());

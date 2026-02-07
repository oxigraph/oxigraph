import { bench, describe } from "vitest";
import { Store } from "../pkg/oxigraph.js";
import { readData } from "./bench_data";

const explore_1000_nt = await readData("explore-1000.nt.zst");
const explore_1000_store = new Store();
explore_1000_store.load(explore_1000_nt, { format: "application/n-triples" });

const bsbm_1000_operations = (await readData("mix-exploreAndUpdate-1000.tsv.zst"))
    .split("\n")
    .reverse()
    .slice(0, 300)
    .filter((line) => line.trim() !== "")
    .map((line) => line.trim().split("\t"));

describe("JS: Store.load", () => {
    bench("JS: load BSBM explore 1000", () => {
        const store = new Store();
        store.load(explore_1000_nt, { format: "application/n-triples" });
    });

    bench("JS: load BSBM explore 1000 lenient no_transaction", () => {
        const store = new Store();
        store.load(explore_1000_nt, {
            format: "application/n-triples",
            lenient: true,
            no_transaction: true,
        });
    });
});

describe("JS: Store.query", () => {
    bench("JS: BSBM explore 1000 query", () => {
        for (const [kind, sparql] of bsbm_1000_operations) {
            if (kind === "query") {
                explore_1000_store.query(sparql, { results_format: "xml" });
            }
        }
    });
});

describe("JS: Store.query and update", () => {
    bench("JS: BSBM explore 1000 query and update", () => {
        for (const [kind, sparql] of bsbm_1000_operations) {
            if (kind === "query") {
                explore_1000_store.query(sparql, { results_format: "xml" });
            } else if (kind === "update") {
                explore_1000_store.update(sparql);
            }
        }
    });
});

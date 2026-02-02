import { Buffer } from "node:buffer";
import { bench, describe } from "vitest";
import { parse } from "../pkg/oxigraph.js";
import { readData } from "./bench_data";

const explore_1000_nt = await readData("explore-1000.nt.zst");
const explore_1000_nt_buffer = Buffer.from(explore_1000_nt);
const explore_1000_nt_lines = explore_1000_nt.split("\n").map((line) => `${line}\n`);

describe("JS: parse", () => {
    bench("JS: parse BSBM explore 1000 string", () => {
        parse(explore_1000_nt, { format: "application/n-triples" });
    });

    bench("JS: parse BSBM explore 1000 buffer", () => {
        parse(explore_1000_nt_buffer, { format: "application/n-triples" });
    });

    bench("JS: parse BSBM explore 1000 string iterator", () => {
        for (const _ of parse(explore_1000_nt_lines, { format: "application/n-triples" })) {
        }
    });

    bench("JS: parse BSBM explore 1000 string async iterator", async () => {
        for await (const _ of parse(asyncIterator(explore_1000_nt_lines), {
            format: "application/n-triples",
        })) {
        }
    });
});

async function* asyncIterator<T>(iterable: Iterable<T>): AsyncIterator<T> {
    for (const item of iterable) {
        yield item;
    }
}

import { Buffer } from "node:buffer";
import { existsSync } from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import * as fzstd from "fzstd";

export async function readData(file) {
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

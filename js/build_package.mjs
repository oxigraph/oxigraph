#! /usr/bin/env node

import fs from "node:fs";

const pkg = JSON.parse(fs.readFileSync("./pkg/package.json"));
pkg.name = "oxigraph";
pkg.main = "node.js";
pkg.browser = "web.js";
pkg.files = ["*.{js,wasm,d.ts}"];
pkg.homepage = "https://github.com/oxigraph/oxigraph/tree/main/js";
pkg.bugs = {
    url: "https://github.com/oxigraph/oxigraph/issues",
};
pkg.collaborators = undefined;
pkg.repository = {
    type: "git",
    url: "https://github.com/oxigraph/oxigraph.git",
    directory: "js",
};
fs.writeFileSync("./pkg/package.json", JSON.stringify(pkg, null, 2));

// Add Symbol.iterator support to Store and Dataset classes
for (const file of ["./pkg/web.js", "./pkg/node.js"]) {
    if (fs.existsSync(file)) {
        let content = fs.readFileSync(file, "utf8");

        // Find and patch Store class to add Symbol.iterator
        // Look for the Store class and add the iterator method after the opening brace
        if (content.includes("class Store")) {
            // Find where the Store class definition ends (just before the final })
            // Add Symbol.iterator method by finding the last method in the class
            const storeMatch = content.match(/(class Store\s*{[\s\S]*?)(\n}\n)/);
            if (storeMatch) {
                const classContent = storeMatch[1];
                const classEnd = storeMatch[2];
                const updatedClass = `${classContent}\n    [Symbol.iterator]() { return this.__iterator(); }${classEnd}`;
                content = content.replace(storeMatch[0], updatedClass);
            }
        }

        // Find and patch Dataset class to add Symbol.iterator
        if (content.includes("class Dataset")) {
            const datasetMatch = content.match(/(class Dataset\s*{[\s\S]*?)(\n}\n)/);
            if (datasetMatch) {
                const classContent = datasetMatch[1];
                const classEnd = datasetMatch[2];
                const updatedClass = `${classContent}\n    [Symbol.iterator]() { return this.__iterator(); }${classEnd}`;
                content = content.replace(datasetMatch[0], updatedClass);
            }
        }

        // Add DataFactory export
        // Find the end of the exports section (usually near the end of the file)
        // and add the DataFactory object that groups all factory functions
        if (content.includes("export { namedNode, blankNode")) {
            // DataFactory already uses individual exports, add it as an object
            const dataFactoryExport = `\n// RDF/JS DataFactory interface
export const DataFactory = {
    namedNode,
    blankNode,
    literal,
    variable,
    defaultGraph,
    triple,
    quad,
    fromTerm,
    fromQuad
};\n`;
            // Append at the end of the file
            content += dataFactoryExport;
        }

        fs.writeFileSync(file, content);
    }
}

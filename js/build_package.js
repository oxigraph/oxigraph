#! /usr/bin/env node

const fs = require("fs");
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

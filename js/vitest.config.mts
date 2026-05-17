import codspeedPlugin from "@codspeed/vitest-plugin";
import wasm from "vite-plugin-wasm";
import { defineConfig } from "vitest/config";

export default defineConfig({
    plugins: [codspeedPlugin(), wasm()],
    test: {
        globals: true,
    },
});

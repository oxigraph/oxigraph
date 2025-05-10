import codspeedPlugin from "@codspeed/vitest-plugin";
import { defineConfig } from "vitest/config";

export default defineConfig({
    plugins: [codspeedPlugin()],
    test: {
        globals: true,
        testTimeout: 600_000, // in ms
    },
});

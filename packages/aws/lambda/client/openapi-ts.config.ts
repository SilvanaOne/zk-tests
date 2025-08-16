import { defineConfig, defaultPlugins } from "@hey-api/openapi-ts";

export default defineConfig({
  input: "../api/openapi.yaml",
  output: "src/generated",
  plugins: [
    ...defaultPlugins,
    {
      name: "@hey-api/typescript",
    },
    {
      name: "@hey-api/client-fetch",
    },
  ],
});
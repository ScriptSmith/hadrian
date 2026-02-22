import { defineConfig } from "@hey-api/openapi-ts";

export default defineConfig({
  input: "../../openapi/hadrian.openapi.json",
  output: {
    path: "src/client",
    format: "prettier",
  },
  plugins: ["@hey-api/typescript", "@hey-api/sdk"],
});

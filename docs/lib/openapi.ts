import { createOpenAPI } from "fumadocs-openapi/server";

export const openapi = createOpenAPI({
  // Path to the Hadrian OpenAPI spec
  input: ["../openapi/hadrian.openapi.json"],
});

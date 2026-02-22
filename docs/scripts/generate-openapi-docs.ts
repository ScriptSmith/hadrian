import { generateFiles } from "fumadocs-openapi";
import { openapi } from "../lib/openapi";

void generateFiles({
  input: openapi,
  output: "./content/docs/api",
  // Group operations by tag for better organization
  per: "tag",
  // Include operation descriptions in the generated pages
  includeDescription: true,
});

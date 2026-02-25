import { readFileSync } from "node:fs";
import { createMDX } from "fumadocs-mdx/next";

const withMDX = createMDX();

const cargoToml = readFileSync("../Cargo.toml", "utf-8");
const version = cargoToml.match(/^version\s*=\s*"(.+)"/m)?.[1] ?? "latest";

const basePath = process.env.DOCS_BASE_PATH || "";

/** @type {import('next').NextConfig} */
const config = {
  env: { HADRIAN_VERSION: version, DOCS_BASE_PATH: basePath },
  reactStrictMode: true,
  // Static export for serving from gateway
  output: "export",
  // Output to 'out' directory (default, can be served by gateway)
  distDir: "out",
  // Use trailing slashes for cleaner static URLs
  trailingSlash: true,
  ...(basePath && { basePath }),
};

export default withMDX(config);

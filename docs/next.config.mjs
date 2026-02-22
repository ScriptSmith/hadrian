import { createMDX } from "fumadocs-mdx/next";

const withMDX = createMDX();

/** @type {import('next').NextConfig} */
const config = {
  reactStrictMode: true,
  // Static export for serving from gateway
  output: "export",
  // Output to 'out' directory (default, can be served by gateway)
  distDir: "out",
  // Use trailing slashes for cleaner static URLs
  trailingSlash: true,
};

export default withMDX(config);

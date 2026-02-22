import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./src/tests/auth",
  testMatch: "**/saml*.test.ts",
  timeout: 60_000,
  retries: 1,
  use: {
    headless: true,
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
});

import { defineConfig } from "vitest/config";

const isCI = process.env.CI === "true";
const isSerial = process.env.E2E_SERIAL === "true";
const testTimeout = parseInt(process.env.VITEST_TEST_TIMEOUT || "300000", 10);
const maxConcurrency = isSerial ? 1 : isCI ? 4 : 8;

export default defineConfig({
  test: {
    globals: true,
    testTimeout, // 5 minutes per test (container startup)
    // Serial mode gets longer hook timeouts since containers start one at a time
    // but CI runners are slow. Parallel local runs use shorter timeouts.
    hookTimeout: isSerial ? 900_000 : 300_000,
    reporters: ["verbose", "./src/reporters/api-coverage.ts"],
    // Retry flaky container tests once
    retry: 1,
    // Enable file parallelism - each test file runs in its own fork
    fileParallelism: !isSerial,
    // Reduce max concurrent forks in CI (GitHub runners have 2 vCPUs)
    maxConcurrency,
  },
});

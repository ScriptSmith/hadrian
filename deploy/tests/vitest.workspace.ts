import { defineWorkspace } from "vitest/config";

const isCI = process.env.CI === "true";
const isSerial = process.env.E2E_SERIAL === "true";

// Serial mode: merge all tests into a single project with no parallelism.
// This prevents multiple Docker Compose environments from starting concurrently,
// which causes hook timeouts on resource-constrained CI runners (2 vCPUs).
const serialWorkspace = [
  {
    extends: "./vitest.config.ts",
    test: {
      name: "all",
      include: ["src/tests/**/*.test.ts"],
      pool: "forks" as const,
      poolOptions: { forks: { singleFork: true } },
      fileParallelism: false,
    },
  },
];

// Parallel mode: separate workspace projects for local development.
// Each project runs in its own fork pool with file parallelism enabled.
const parallelWorkspace = [
  {
    extends: "./vitest.config.ts",
    test: {
      name: "basic",
      include: ["src/tests/basic/**/*.test.ts"],
      pool: "forks" as const,
      poolOptions: { forks: { singleFork: isCI } },
      fileParallelism: !isCI,
    },
  },
  {
    extends: "./vitest.config.ts",
    test: {
      name: "infrastructure",
      include: ["src/tests/infrastructure/**/*.test.ts"],
      pool: "forks" as const,
      poolOptions: { forks: { singleFork: isCI } },
      fileParallelism: !isCI,
    },
  },
  {
    extends: "./vitest.config.ts",
    test: {
      name: "auth",
      include: ["src/tests/auth/**/*.test.ts"],
      pool: "forks" as const,
      poolOptions: { forks: { singleFork: isCI } },
      fileParallelism: !isCI,
    },
  },
];

export default defineWorkspace(isSerial ? serialWorkspace : parallelWorkspace);

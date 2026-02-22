import { setProjectAnnotations } from "@storybook/react-vite";
import * as a11yAddonAnnotations from "@storybook/addon-a11y/preview";
import * as projectAnnotations from "./preview";
import { beforeAll, afterAll } from "vitest";

// Apply annotations with dark theme as default
setProjectAnnotations([
  a11yAddonAnnotations,
  projectAnnotations,
  { initialGlobals: { theme: "dark" } },
]);

// Suppress expected console messages in tests
const originalConsoleLog = console.log;
const originalConsoleWarn = console.warn;
const originalConsoleError = console.error;

const suppressedLogPatterns = [
  /^Documentation:/,
  /^Found an issue\?/,
  /^Worker script URL:/,
  /^Worker scope:/,
  /^Client ID:/,
  /^Mocking enabled/,
  /^\[MSW\]/,
];

const suppressedWarnPatterns = [/The input spec uses Vega-Lite/];

const suppressedErrorPatterns = [
  /ErrorBoundary caught an error/,
  /The above error occurred in the/,
];

beforeAll(() => {
  console.log = (...args: unknown[]) => {
    const message = String(args[0] ?? "");
    if (suppressedLogPatterns.some((pattern) => pattern.test(message))) {
      return;
    }
    originalConsoleLog.apply(console, args);
  };

  console.warn = (...args: unknown[]) => {
    const message = args.join(" ");
    if (suppressedWarnPatterns.some((pattern) => pattern.test(message))) {
      return;
    }
    originalConsoleWarn.apply(console, args);
  };

  console.error = (...args: unknown[]) => {
    const message = args.join(" ");
    if (suppressedErrorPatterns.some((pattern) => pattern.test(message))) {
      return;
    }
    originalConsoleError.apply(console, args);
  };
});

afterAll(() => {
  console.log = originalConsoleLog;
  console.warn = originalConsoleWarn;
  console.error = originalConsoleError;
});

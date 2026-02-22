import { setProjectAnnotations } from "@storybook/react-vite";
import * as a11yAddonAnnotations from "@storybook/addon-a11y/preview";
import * as projectAnnotations from "./preview";
import { beforeAll, afterAll } from "vitest";

// This is an important step to apply the right configuration when testing your stories.
// More info at: https://storybook.js.org/docs/api/portable-stories/portable-stories-vitest#setprojectannotations
setProjectAnnotations([a11yAddonAnnotations, projectAnnotations]);

// Suppress expected console messages in tests
// These are expected output from components behaving correctly

const originalConsoleLog = console.log;
const originalConsoleWarn = console.warn;
const originalConsoleError = console.error;

// Patterns for log messages that are expected and should be suppressed
const suppressedLogPatterns = [
  // MSW startup and request/response debug output
  /^Documentation:/,
  /^Found an issue\?/,
  /^Worker script URL:/,
  /^Worker scope:/,
  /^Client ID:/,
  /^Mocking enabled/,
  /^\[MSW\]/,
];

// Patterns for warnings/errors that are expected and should be suppressed
const suppressedWarnPatterns = [
  // Vega-Lite version warnings are informational, not actionable
  /The input spec uses Vega-Lite/,
];

const suppressedErrorPatterns = [
  // React error boundary test intentionally throws to test error handling
  /ErrorBoundary caught an error/,
  // React's internal error logging for caught errors
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

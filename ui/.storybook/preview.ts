import type { Preview } from "@storybook/react";
import React from "react";
import { initialize, mswLoader } from "msw-storybook-addon";
import { http, HttpResponse } from "msw";
import "../src/index.css";
import { PreferencesProvider } from "../src/preferences/PreferencesProvider";
import { ConfigProvider } from "../src/config/ConfigProvider";
import { defaultConfig } from "../src/config/defaults";
import { defaultPreferences } from "../src/preferences/types";

// Initialize MSW
// Use relative URL so it works both standalone and when embedded in docs
//
// The second argument registers *initial* handlers that survive each story's
// `resetHandlers()` (story-level `parameters.msw.handlers` are runtime handlers and
// still take precedence). Every story is wrapped in `ConfigProvider`, which fetches
// `/admin/v1/ui/config` on mount. Without a default handler those requests fall
// through to the Vite dev-server proxy → localhost:8080 (no backend in tests),
// spamming the output with "http proxy error" AggregateErrors. Serve defaults here.
initialize(
  {
    quiet: true, // Suppress MSW startup and request/response logging in tests
    onUnhandledRequest: "bypass", // Don't warn about unhandled requests
    serviceWorker: {
      url: "./mockServiceWorker.js",
    },
  },
  [http.get("*/admin/v1/ui/config", () => HttpResponse.json(defaultConfig))]
);

const preview: Preview = {
  parameters: {
    a11y: {
      test: "error",
    },
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/i,
      },
    },
    backgrounds: {
      default: "light",
      values: [
        { name: "light", value: "#ffffff" },
        { name: "dark", value: "#0f172a" },
      ],
    },
    layout: "centered",
  },
  loaders: [mswLoader],
  decorators: [
    (Story, context) => {
      const theme = context.globals.theme || "light";
      document.documentElement.classList.remove("light", "dark");
      document.documentElement.classList.add(theme);
      // Sync localStorage so PreferencesProvider doesn't override the storybook theme
      try {
        const raw = localStorage.getItem("hadrian-preferences");
        const stored = raw ? JSON.parse(raw) : {};
        localStorage.setItem(
          "hadrian-preferences",
          JSON.stringify({ ...defaultPreferences, ...stored, theme })
        );
      } catch {
        localStorage.setItem(
          "hadrian-preferences",
          JSON.stringify({ ...defaultPreferences, theme })
        );
      }
      return React.createElement(
        ConfigProvider,
        null,
        React.createElement(PreferencesProvider, null, React.createElement(Story))
      );
    },
  ],
  globalTypes: {
    theme: {
      description: "Global theme for components",
      defaultValue: "light",
      toolbar: {
        title: "Theme",
        icon: "circlehollow",
        items: ["light", "dark"],
        dynamicTitle: true,
      },
    },
  },
};

export default preview;

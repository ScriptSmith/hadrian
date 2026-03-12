import type { Preview } from "@storybook/react";
import React from "react";
import { initialize, mswLoader } from "msw-storybook-addon";
import "../src/index.css";
import { PreferencesProvider } from "../src/preferences/PreferencesProvider";
import { defaultPreferences } from "../src/preferences/types";

// Initialize MSW
// Use relative URL so it works both standalone and when embedded in docs
initialize({
  quiet: true, // Suppress MSW startup and request/response logging in tests
  onUnhandledRequest: "bypass", // Don't warn about unhandled requests
  serviceWorker: {
    url: "./mockServiceWorker.js",
  },
});

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
      return React.createElement(PreferencesProvider, null, React.createElement(Story));
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

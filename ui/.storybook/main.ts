import type { StorybookConfig } from "@storybook/react-vite";

const config: StorybookConfig = {
  stories: ["../src/**/*.stories.@(js|jsx|mjs|ts|tsx)"],
  addons: [
    "@storybook/addon-vitest",
    "msw-storybook-addon",
    "@storybook/addon-docs",
    "@storybook/addon-a11y",
  ],
  staticDirs: ["../public"],
  framework: {
    name: "@storybook/react-vite",
    options: {},
  },
  docs: {
    autodocs: true,
  },
  typescript: {
    reactDocgen: "react-docgen-typescript",
  },
  async viteFinal(config) {
    // Remove PWA plugin for Storybook builds
    config.plugins = config.plugins?.filter((plugin) => {
      if (Array.isArray(plugin)) {
        return !plugin.some(
          (p) => p && typeof p === "object" && "name" in p && p.name?.includes("pwa")
        );
      }
      return !(
        plugin &&
        typeof plugin === "object" &&
        "name" in plugin &&
        plugin.name?.includes("pwa")
      );
    });
    return config;
  },
};

export default config;
